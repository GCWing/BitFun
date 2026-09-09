use crate::{
    atomic_write_json, LegacyMigrationError, LegacyMigrationResult, MigrationLayout, MigrationRoots,
};
use openbitfun_product_domains::legacy_migration::{
    MigrationPlan, MigratorHandoffRequest, MigratorProtocolCapabilities, MigratorRequestMode,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const MAX_HANDOFF_REQUEST_BYTES: u64 = 64 * 1024;
const MAX_HANDOFF_LIFETIME_MS: i64 = 15 * 60 * 1000;
const MAX_CLOCK_SKEW_MS: i64 = 60 * 1000;
const NONCE_RECEIPT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandoffDisposition {
    Fresh,
    Recovery,
}

#[derive(Debug, Clone)]
pub struct ValidatedHandoff {
    request: MigratorHandoffRequest,
    layout: MigrationLayout,
    disposition: HandoffDisposition,
}

impl ValidatedHandoff {
    pub fn request(&self) -> &MigratorHandoffRequest {
        &self.request
    }

    pub fn layout(&self) -> &MigrationLayout {
        &self.layout
    }

    pub const fn disposition(&self) -> HandoffDisposition {
        self.disposition
    }
}

#[derive(Debug, Clone)]
pub struct HandoffStore {
    roots: MigrationRoots,
    expected_product_id: String,
    expected_release_channel: String,
}

impl HandoffStore {
    pub fn new(
        roots: MigrationRoots,
        expected_product_id: impl Into<String>,
        expected_release_channel: impl Into<String>,
    ) -> Self {
        Self {
            roots,
            expected_product_id: expected_product_id.into(),
            expected_release_channel: expected_release_channel.into(),
        }
    }

    pub fn write_request(
        &self,
        request: &MigratorHandoffRequest,
        now_ms: i64,
    ) -> LegacyMigrationResult<PathBuf> {
        self.validate_request(request, &request.run_id, now_ms)?;
        let layout = MigrationLayout::new(&self.roots, &request.run_id);
        initialize_private_layout(&layout)?;
        ensure_path_chain_is_plain(layout.root(), &layout.request_path())?;
        write_new_private_json(&layout.request_path(), request)?;
        verify_current_user_owned(&layout.request_path())?;
        Ok(layout.request_path())
    }

    pub fn load_request(
        &self,
        run_id: &str,
        now_ms: i64,
    ) -> LegacyMigrationResult<ValidatedHandoff> {
        validate_uuid("run id", run_id)?;
        let layout = MigrationLayout::new(&self.roots, run_id);
        ensure_path_chain_is_plain(layout.root(), &layout.request_path())?;
        verify_current_user_owned(&layout.request_path())?;
        let request = read_bounded_json::<MigratorHandoffRequest>(
            &layout.request_path(),
            MAX_HANDOFF_REQUEST_BYTES,
        )?;
        self.validate_request(&request, run_id, now_ms)?;

        let disposition = match read_optional_bounded_json::<ConsumedNonceReceipt>(
            &layout.consumed_nonce_path(),
            MAX_HANDOFF_REQUEST_BYTES,
        )? {
            None => HandoffDisposition::Fresh,
            Some(receipt) => {
                validate_nonce_receipt(&receipt, &request)?;
                let persisted_plan = layout
                    .read_json::<MigrationPlan>(&layout.plan_path())?
                    .ok_or_else(|| {
                        LegacyMigrationError::InvalidRequest(
                            "handoff nonce was consumed before an immutable plan was stored"
                                .to_string(),
                        )
                    })?;
                validate_plan_binding(&persisted_plan, &request)?;
                HandoffDisposition::Recovery
            }
        };

        Ok(ValidatedHandoff {
            request,
            layout,
            disposition,
        })
    }

    /// Persist the immutable plan before consuming the one-time nonce.
    ///
    /// This ordering leaves a recoverable plan if the process stops immediately
    /// after the nonce receipt is made durable. Reusing a receipt is accepted
    /// only for that exact plan and request.
    pub fn authorize_plan(
        &self,
        handoff: &ValidatedHandoff,
        plan: &MigrationPlan,
        now_ms: i64,
    ) -> LegacyMigrationResult<HandoffDisposition> {
        self.validate_request(&handoff.request, &handoff.request.run_id, now_ms)?;
        validate_plan_binding(plan, &handoff.request)?;
        if let Some(existing) = handoff
            .layout
            .read_json::<MigrationPlan>(&handoff.layout.plan_path())?
        {
            if existing != *plan {
                return Err(LegacyMigrationError::InvalidPlan(
                    "handoff run already has a different immutable plan".to_string(),
                ));
            }
        } else {
            atomic_write_json(&handoff.layout.plan_path(), plan)?;
        }

        let receipt = ConsumedNonceReceipt::new(&handoff.request, now_ms);
        match write_new_private_json(&handoff.layout.consumed_nonce_path(), &receipt) {
            Ok(()) => Ok(HandoffDisposition::Fresh),
            Err(LegacyMigrationError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                let existing = read_bounded_json::<ConsumedNonceReceipt>(
                    &handoff.layout.consumed_nonce_path(),
                    MAX_HANDOFF_REQUEST_BYTES,
                )?;
                validate_nonce_receipt(&existing, &handoff.request)?;
                Ok(HandoffDisposition::Recovery)
            }
            Err(error) => Err(error),
        }
    }

    pub fn load_authorized_plan(
        &self,
        handoff: &ValidatedHandoff,
    ) -> LegacyMigrationResult<Option<MigrationPlan>> {
        let Some(receipt) = read_optional_bounded_json::<ConsumedNonceReceipt>(
            &handoff.layout.consumed_nonce_path(),
            MAX_HANDOFF_REQUEST_BYTES,
        )?
        else {
            return Ok(None);
        };
        validate_nonce_receipt(&receipt, &handoff.request)?;
        let plan = handoff
            .layout
            .read_json::<MigrationPlan>(&handoff.layout.plan_path())?;
        if let Some(plan) = &plan {
            validate_plan_binding(plan, &handoff.request)?;
        }
        Ok(plan)
    }

    fn validate_request(
        &self,
        request: &MigratorHandoffRequest,
        expected_run_id: &str,
        now_ms: i64,
    ) -> LegacyMigrationResult<()> {
        validate_uuid("run id", &request.run_id)?;
        validate_uuid("nonce", &request.nonce)?;
        if request.run_id != expected_run_id {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff request does not match the derived run path".to_string(),
            ));
        }
        if !MigratorProtocolCapabilities::current().accepts_request(request) {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff protocol or required capabilities are unsupported".to_string(),
            ));
        }
        if request.product_id != self.expected_product_id {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff request belongs to a different product identity".to_string(),
            ));
        }
        if request.release_channel != self.expected_release_channel {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff request belongs to a different release channel".to_string(),
            ));
        }
        if request.caller_process_id == 0 {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff request has no caller process identity".to_string(),
            ));
        }
        if request.created_at_ms > now_ms.saturating_add(MAX_CLOCK_SKEW_MS) {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff request creation time is in the future".to_string(),
            ));
        }
        if request.expires_at_ms <= request.created_at_ms
            || request.expires_at_ms.saturating_sub(request.created_at_ms) > MAX_HANDOFF_LIFETIME_MS
        {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff request lifetime is invalid".to_string(),
            ));
        }
        if request.is_expired_at(now_ms) {
            return Err(LegacyMigrationError::InvalidRequest(
                "handoff request has expired".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ConsumedNonceReceipt {
    format_version: u32,
    run_id: String,
    nonce_sha256: String,
    consumed_at_ms: i64,
}

impl Default for ConsumedNonceReceipt {
    fn default() -> Self {
        Self {
            format_version: NONCE_RECEIPT_FORMAT_VERSION,
            run_id: String::new(),
            nonce_sha256: String::new(),
            consumed_at_ms: 0,
        }
    }
}

impl ConsumedNonceReceipt {
    fn new(request: &MigratorHandoffRequest, consumed_at_ms: i64) -> Self {
        Self {
            format_version: NONCE_RECEIPT_FORMAT_VERSION,
            run_id: request.run_id.clone(),
            nonce_sha256: nonce_digest(&request.nonce),
            consumed_at_ms,
        }
    }
}

fn validate_nonce_receipt(
    receipt: &ConsumedNonceReceipt,
    request: &MigratorHandoffRequest,
) -> LegacyMigrationResult<()> {
    if receipt.format_version != NONCE_RECEIPT_FORMAT_VERSION
        || receipt.run_id != request.run_id
        || receipt.nonce_sha256 != nonce_digest(&request.nonce)
    {
        return Err(LegacyMigrationError::InvalidRequest(
            "handoff nonce has already been consumed by another request".to_string(),
        ));
    }
    Ok(())
}

fn validate_plan_binding(
    plan: &MigrationPlan,
    request: &MigratorHandoffRequest,
) -> LegacyMigrationResult<()> {
    if plan.run_id != request.run_id {
        return Err(LegacyMigrationError::InvalidPlan(
            "migration plan does not belong to the handoff run".to_string(),
        ));
    }
    if let Some(source_fingerprint) = request.source_fingerprint.as_deref() {
        if plan.source_fingerprint != source_fingerprint {
            return Err(LegacyMigrationError::InvalidPlan(
                "migration plan does not match the requested legacy source".to_string(),
            ));
        }
    }
    if request.mode == MigratorRequestMode::Execute && plan.selection != request.selection {
        return Err(LegacyMigrationError::InvalidPlan(
            "migration plan selection differs from the confirmed handoff selection".to_string(),
        ));
    }
    Ok(())
}

fn nonce_digest(nonce: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(nonce.as_bytes())))
}

fn validate_uuid(label: &str, value: &str) -> LegacyMigrationResult<()> {
    uuid::Uuid::parse_str(value).map_err(|_| {
        LegacyMigrationError::InvalidRequest(format!("handoff {label} must be a UUID"))
    })?;
    Ok(())
}

fn initialize_private_layout(layout: &MigrationLayout) -> LegacyMigrationResult<()> {
    layout.initialize()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [
            layout.root(),
            &layout.run_root(),
            &layout.stage_root(),
            &layout.backup_root(),
        ] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|error| LegacyMigrationError::io(path, error))?;
        }
    }
    Ok(())
}

fn write_new_private_json<T: Serialize>(path: &Path, value: &T) -> LegacyMigrationResult<()> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| LegacyMigrationError::json(path, error))?;
    bytes.push(b'\n');
    #[cfg(windows)]
    let mut file = create_current_user_owned_file(path)?;
    #[cfg(not(windows))]
    let mut file = {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(path)
            .map_err(|error| LegacyMigrationError::io(path, error))?
    };
    file.write_all(&bytes)
        .map_err(|error| LegacyMigrationError::io(path, error))?;
    file.sync_all()
        .map_err(|error| LegacyMigrationError::io(path, error))?;
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
    Ok(())
}

fn read_bounded_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
) -> LegacyMigrationResult<T> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| LegacyMigrationError::io(path, error))?;
    if is_link_or_reparse(&metadata) {
        return Err(LegacyMigrationError::LinkedPath(path.to_path_buf()));
    }
    if metadata.len() > max_bytes {
        return Err(LegacyMigrationError::ResourceLimit(format!(
            "handoff file exceeds {max_bytes} bytes"
        )));
    }
    let bytes = fs::read(path).map_err(|error| LegacyMigrationError::io(path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| LegacyMigrationError::json(path, error))
}

fn read_optional_bounded_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
) -> LegacyMigrationResult<Option<T>> {
    match read_bounded_json(path, max_bytes) {
        Ok(value) => Ok(Some(value)),
        Err(LegacyMigrationError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn ensure_path_chain_is_plain(root: &Path, target: &Path) -> LegacyMigrationResult<()> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| LegacyMigrationError::PathEscape(target.to_path_buf()))?;
    let mut current = root.to_path_buf();
    if let Ok(metadata) = fs::symlink_metadata(&current) {
        if is_link_or_reparse(&metadata) {
            return Err(LegacyMigrationError::LinkedPath(current));
        }
    }
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(LegacyMigrationError::PathEscape(target.to_path_buf()));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_link_or_reparse(&metadata) => {
                return Err(LegacyMigrationError::LinkedPath(current));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(LegacyMigrationError::io(&current, error)),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn verify_current_user_owned(path: &Path) -> LegacyMigrationResult<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows::Win32::Security::{
        EqualSid, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    };

    struct LocalSecurityDescriptor(PSECURITY_DESCRIPTOR);
    impl Drop for LocalSecurityDescriptor {
        fn drop(&mut self) {
            // SAFETY: GetNamedSecurityInfoW allocated this descriptor with
            // LocalAlloc-compatible ownership and this wrapper frees it once.
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.0 .0)));
            }
        }
    }

    let canonical_path =
        fs::canonicalize(path).map_err(|error| LegacyMigrationError::io(path, error))?;
    let wide_path = canonical_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut owner = PSID::default();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: every output pointer refers to initialized storage that lives
    // through the call; the UTF-16 path is NUL terminated.
    let status = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(wide_path.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            None,
            None,
            &mut descriptor,
        )
    };
    if status.0 != 0 || descriptor.0.is_null() || owner.0.is_null() {
        return Err(LegacyMigrationError::InvalidRequest(
            "handoff file owner could not be verified".to_string(),
        ));
    }
    let _descriptor = LocalSecurityDescriptor(descriptor);

    with_current_user_sid(|sid| {
        // SAFETY: both SIDs are owned by live security descriptor/token buffers.
        unsafe { EqualSid(owner, sid) }.map_err(|_| {
            LegacyMigrationError::InvalidRequest(
                "handoff file is owned by another OS user".to_string(),
            )
        })
    })
}

#[cfg(windows)]
fn with_current_user_sid<T>(
    action: impl FnOnce(windows::Win32::Security::PSID) -> LegacyMigrationResult<T>,
) -> LegacyMigrationResult<T> {
    use std::ffi::c_void;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct OwnedHandle(HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns the process token handle exactly once.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    let mut raw_token = HANDLE::default();
    // SAFETY: `raw_token` is writable and becomes owned by `OwnedHandle` only
    // after OpenProcessToken succeeds.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw_token) }
        .map_err(|error| LegacyMigrationError::ProcessInspection(error.to_string()))?;
    let token = OwnedHandle(raw_token);
    let mut required = 0u32;
    // SAFETY: the zero-length probe passes no output buffer and a valid size
    // pointer, as required by GetTokenInformation.
    let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut required) };
    if required < std::mem::size_of::<TOKEN_USER>() as u32 {
        return Err(LegacyMigrationError::ProcessInspection(
            "current user token identity is unavailable".to_string(),
        ));
    }
    let word_count = (required as usize).div_ceil(std::mem::size_of::<usize>());
    let mut buffer = vec![0usize; word_count];
    // SAFETY: `buffer` has exactly the size reported by the preceding probe and
    // remains live through both this call and the TOKEN_USER view below.
    unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut c_void),
            required,
            &mut required,
        )
    }
    .map_err(|error| LegacyMigrationError::ProcessInspection(error.to_string()))?;
    // SAFETY: GetTokenInformation successfully initialized a TOKEN_USER at the
    // start of the aligned allocator buffer for the duration of this scope.
    let token_user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
    action(token_user.User.Sid)
}

#[cfg(windows)]
fn create_current_user_owned_file(path: &Path) -> LegacyMigrationResult<File> {
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows::core::PCWSTR;
    use windows::Win32::Security::{
        InitializeSecurityDescriptor, SetSecurityDescriptorOwner, PSECURITY_DESCRIPTOR,
        SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
    };
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_WRITE, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    with_current_user_sid(|sid| {
        let mut descriptor = SECURITY_DESCRIPTOR::default();
        let descriptor_ptr =
            PSECURITY_DESCRIPTOR((&mut descriptor as *mut SECURITY_DESCRIPTOR).cast());
        // SAFETY: the descriptor is aligned, writable and lives through file creation.
        // The SID remains live inside with_current_user_sid. Leave the DACL absent
        // so Windows retains its normal inherited/default access policy.
        unsafe {
            InitializeSecurityDescriptor(descriptor_ptr, 1)
                .and_then(|()| SetSecurityDescriptorOwner(descriptor_ptr, Some(sid), false))
        }
        .map_err(|error| LegacyMigrationError::ProcessInspection(error.to_string()))?;
        let mut attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor_ptr.0,
            bInheritHandle: false.into(),
        };
        // canonicalize supplies the verbatim Windows prefix for long paths,
        // matching std::fs behavior without requiring the new file to exist.
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = path.file_name().ok_or_else(|| {
            LegacyMigrationError::io(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "request path has no filename",
                ),
            )
        })?;
        let absolute = fs::canonicalize(parent)
            .map_err(|error| LegacyMigrationError::io(path, error))?
            .join(name);
        let mut wide_path = absolute.as_os_str().encode_wide().collect::<Vec<_>>();
        if wide_path.contains(&0) {
            return Err(LegacyMigrationError::io(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "request path contains a NUL character",
                ),
            ));
        }
        wide_path.push(0);
        // SAFETY: the path, attributes, descriptor and SID remain live through
        // CreateFileW. CREATE_NEW never opens or changes an existing file.
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide_path.as_ptr()),
                FILE_GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                Some(&mut attributes),
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|error| {
            // CreateFileW reports HRESULT_FROM_WIN32; preserve the OS code so
            // callers can still distinguish AlreadyExists during nonce recovery.
            LegacyMigrationError::io(
                path,
                std::io::Error::from_raw_os_error(error.code().0 & 0xffff),
            )
        })?;
        // SAFETY: CreateFileW returned a valid, uniquely owned handle; File
        // takes responsibility for closing it, including on write failure.
        Ok(unsafe { File::from_raw_handle(handle.0) })
    })
}

#[cfg(unix)]
fn verify_current_user_owned(path: &Path) -> LegacyMigrationResult<()> {
    use std::os::unix::fs::MetadataExt;
    let metadata =
        fs::symlink_metadata(path).map_err(|error| LegacyMigrationError::io(path, error))?;
    // SAFETY: geteuid takes no pointers and has no caller-side preconditions.
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(LegacyMigrationError::InvalidRequest(
            "handoff file is owned by another OS user".to_string(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterProcess {
    pub process_id: u32,
    pub executable_name: String,
    pub is_handoff_caller: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessEntry {
    process_id: u32,
    executable_name: String,
}

const WRITER_EXECUTABLE_NAMES: &[&str] = &[
    "bitfun.exe",
    "bitfun-desktop.exe",
    "bitfun",
    "bitfun-desktop",
    "openbitfun.exe",
    "openbitfun-desktop.exe",
    "openbitfun-agent-runtime.exe",
    "openbitfun",
    "openbitfun-desktop",
    "openbitfun-agent-runtime",
];

pub fn blocking_writer_processes(
    caller_process_id: u32,
) -> LegacyMigrationResult<Vec<WriterProcess>> {
    blocking_writer_processes_for_product(caller_process_id, &[])
}

/// Find legacy and current writers, including product-defined sibling names.
///
/// The additional names come from the build-time product projection rather
/// than the handoff request, so a request cannot broaden process matching.
pub fn blocking_writer_processes_for_product(
    caller_process_id: u32,
    product_writer_binary_names: &[&str],
) -> LegacyMigrationResult<Vec<WriterProcess>> {
    let entries = platform_process_entries()?;
    Ok(classify_writer_processes(
        &entries,
        caller_process_id,
        std::process::id(),
        product_writer_binary_names,
    ))
}

fn classify_writer_processes(
    entries: &[ProcessEntry],
    caller_process_id: u32,
    current_process_id: u32,
    product_writer_binary_names: &[&str],
) -> Vec<WriterProcess> {
    let mut blockers = entries
        .iter()
        .filter(|entry| entry.process_id != current_process_id)
        .filter_map(|entry| {
            // Zero means an independent launch with no caller. Windows exposes
            // the System Idle Process at PID 0; it must never block migration.
            let is_handoff_caller = caller_process_id != 0 && entry.process_id == caller_process_id;
            let known_writer = WRITER_EXECUTABLE_NAMES
                .iter()
                .chain(product_writer_binary_names.iter())
                .any(|name| process_name_matches_binary(&entry.executable_name, name));
            (is_handoff_caller || known_writer).then(|| WriterProcess {
                process_id: entry.process_id,
                executable_name: entry.executable_name.clone(),
                is_handoff_caller,
            })
        })
        .collect::<Vec<_>>();
    blockers.sort_by_key(|process| process.process_id);
    blockers.dedup_by_key(|process| process.process_id);
    blockers
}

fn process_name_matches_binary(process_name: &str, binary_name: &str) -> bool {
    process_name.eq_ignore_ascii_case(binary_name)
        || process_name.eq_ignore_ascii_case(&platform_binary_filename(binary_name))
}

#[cfg(windows)]
fn platform_process_entries() -> LegacyMigrationResult<Vec<ProcessEntry>> {
    use std::mem::size_of;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    struct Snapshot(HANDLE);
    impl Drop for Snapshot {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns the ToolHelp snapshot handle exactly
            // once and never exposes it beyond this scope.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }

    let snapshot = Snapshot(
        // SAFETY: the system call receives a documented snapshot flag and no
        // caller-owned pointers.
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
            .map_err(|error| LegacyMigrationError::ProcessInspection(error.to_string()))?,
    );
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut entries = Vec::new();
    // SAFETY: the snapshot is live and `entry.dwSize` identifies the complete
    // writable PROCESSENTRY32W buffer.
    if unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_err() {
        return Ok(entries);
    }
    loop {
        let length = entry
            .szExeFile
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(entry.szExeFile.len());
        entries.push(ProcessEntry {
            process_id: entry.th32ProcessID,
            executable_name: String::from_utf16_lossy(&entry.szExeFile[..length]),
        });
        // SAFETY: the same live snapshot and correctly sized entry buffer are
        // reused serially until enumeration completes.
        if unsafe { Process32NextW(snapshot.0, &mut entry) }.is_err() {
            break;
        }
    }
    Ok(entries)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_process_entries() -> LegacyMigrationResult<Vec<ProcessEntry>> {
    let mut entries = Vec::new();
    let directory = fs::read_dir("/proc")
        .map_err(|error| LegacyMigrationError::ProcessInspection(error.to_string()))?;
    for entry in directory.flatten() {
        let Some(process_id) = entry.file_name().to_string_lossy().parse::<u32>().ok() else {
            continue;
        };
        let executable_name = fs::read_to_string(entry.path().join("comm"))
            .unwrap_or_default()
            .trim()
            .to_string();
        entries.push(ProcessEntry {
            process_id,
            executable_name,
        });
    }
    Ok(entries)
}

#[cfg(target_os = "macos")]
fn platform_process_entries() -> LegacyMigrationResult<Vec<ProcessEntry>> {
    // macOS has no /proc. Use the system ps with unlimited output width so
    // application bundle paths (including spaces) cannot truncate writer names.
    // `comm` excludes arguments, which may contain user content or credentials.
    let output = openbitfun_services_core::process_manager::create_command("/bin/ps")
        .args(["-ww", "-axo", "pid=,comm="])
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| {
            LegacyMigrationError::ProcessInspection("could not run system ps".to_string())
        })?;
    if !output.status.success() {
        return Err(LegacyMigrationError::ProcessInspection(
            "system ps failed to enumerate processes".to_string(),
        ));
    }
    parse_macos_process_entries(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_process_entries(output: &str) -> LegacyMigrationResult<Vec<ProcessEntry>> {
    let invalid_inventory = || {
        LegacyMigrationError::ProcessInspection(
            "system ps returned an invalid inventory".to_string(),
        )
    };
    let mut entries = Vec::new();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let (pid, command) = line
            .trim_start()
            .split_once(char::is_whitespace)
            .ok_or_else(invalid_inventory)?;
        let process_id = pid
            .parse::<u32>()
            .ok()
            .filter(|pid| *pid > 0)
            .ok_or_else(invalid_inventory)?;
        let executable_name = command
            .trim_start()
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .ok_or_else(invalid_inventory)?
            .to_string();
        entries.push(ProcessEntry {
            process_id,
            executable_name,
        });
    }
    if entries.is_empty() {
        return Err(invalid_inventory());
    }
    Ok(entries)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedExecutable {
    current_executable: PathBuf,
    target_executable: PathBuf,
}

impl TrustedExecutable {
    pub fn current_executable(&self) -> &Path {
        &self.current_executable
    }

    pub fn target_executable(&self) -> &Path {
        &self.target_executable
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TrustedInstallationResolver;

impl TrustedInstallationResolver {
    pub fn resolve_sibling(
        current_executable: &Path,
        expected_current_binary_name: &str,
        expected_target_binary_name: &str,
    ) -> LegacyMigrationResult<TrustedExecutable> {
        validate_binary_name(expected_current_binary_name)?;
        validate_binary_name(expected_target_binary_name)?;
        let current = fs::canonicalize(current_executable)
            .map_err(|error| LegacyMigrationError::io(current_executable, error))?;
        let current_name = current.file_name().and_then(OsStr::to_str).ok_or_else(|| {
            LegacyMigrationError::TrustedInstallationUnavailable(
                "current executable has no Unicode file name".to_string(),
            )
        })?;
        if !names_equal(
            current_name,
            &platform_binary_filename(expected_current_binary_name),
        ) {
            return Err(LegacyMigrationError::UntrustedExecutable(current));
        }
        reject_linked_executable(&current)?;
        let install_root = current.parent().ok_or_else(|| {
            LegacyMigrationError::TrustedInstallationUnavailable(
                "current executable has no installation directory".to_string(),
            )
        })?;
        let target_path = install_root.join(platform_binary_filename(expected_target_binary_name));
        reject_linked_executable(&target_path)?;
        let target = fs::canonicalize(&target_path)
            .map_err(|error| LegacyMigrationError::io(&target_path, error))?;
        if target.parent() != Some(install_root) {
            return Err(LegacyMigrationError::UntrustedExecutable(target));
        }
        Ok(TrustedExecutable {
            current_executable: current,
            target_executable: target,
        })
    }
}

pub fn launch_trusted_executable(
    executable: &TrustedExecutable,
    arguments: &[&OsStr],
) -> LegacyMigrationResult<u32> {
    let initial_spawn = openbitfun_services_core::process_manager::create_detached_command(
        executable.target_executable(),
    )
    .args(arguments)
    .spawn();

    #[cfg(windows)]
    let child = match initial_spawn {
        Ok(child) => child,
        Err(error) if allow_inherited_job_dev_retry(&error) => {
            openbitfun_services_core::process_manager::create_inherited_job_process_group_command(
                executable.target_executable(),
            )
            .args(arguments)
            .spawn()
            .map_err(|retry_error| {
                LegacyMigrationError::io(executable.target_executable(), retry_error)
            })?
        }
        Err(error) => {
            return Err(LegacyMigrationError::io(
                executable.target_executable(),
                error,
            ));
        }
    };

    #[cfg(not(windows))]
    let child = initial_spawn
        .map_err(|error| LegacyMigrationError::io(executable.target_executable(), error))?;

    Ok(child.id())
}

#[cfg(windows)]
fn allow_inherited_job_dev_retry(error: &std::io::Error) -> bool {
    should_retry_without_job_breakaway(cfg!(debug_assertions), error.kind())
}

#[cfg(any(windows, test))]
fn should_retry_without_job_breakaway(
    debug_assertions: bool,
    error_kind: std::io::ErrorKind,
) -> bool {
    debug_assertions && error_kind == std::io::ErrorKind::PermissionDenied
}

fn validate_binary_name(name: &str) -> LegacyMigrationResult<()> {
    if name.is_empty()
        || Path::new(name).components().count() != 1
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', '\0'])
    {
        return Err(LegacyMigrationError::TrustedInstallationUnavailable(
            "trusted executable name is invalid".to_string(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn platform_binary_filename(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    }
}

#[cfg(not(windows))]
fn platform_binary_filename(name: &str) -> String {
    name.to_string()
}

#[cfg(windows)]
fn names_equal(actual: &str, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}

#[cfg(not(windows))]
fn names_equal(actual: &str, expected: &str) -> bool {
    actual == expected
}

fn reject_linked_executable(path: &Path) -> LegacyMigrationResult<()> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| LegacyMigrationError::io(path, error))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(LegacyMigrationError::UntrustedExecutable(
            path.to_path_buf(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbitfun_product_domains::legacy_migration::{
        MigrationSelection, MigratorProtocolCapability, MigratorRequestOrigin,
        CURRENT_MIGRATION_FORMAT_VERSION, CURRENT_MIGRATOR_PROTOCOL_VERSION,
    };
    use std::collections::BTreeSet;

    #[test]
    fn macos_inventory_preserves_bundle_names_and_writer_classification() {
        let entries = parse_macos_process_entries(
            "   42 /Users/test/Development Projects/target/debug/openbitfun-desktop\n\
              43 /Applications/BitFun.app/Contents/MacOS/BitFun\n\
              44 /Applications/Custom Product.app/Contents/MacOS/Custom Product\n\
              45 /Applications/OpenBitFun.app/Contents/MacOS/openbitfun-data-migrator\n\
              46 /usr/bin/unrelated\n",
        )
        .expect("parse process inventory");
        let blockers = classify_writer_processes(&entries, 42, 45, &["Custom Product"]);
        assert_eq!(
            blockers
                .iter()
                .map(|entry| entry.process_id)
                .collect::<Vec<_>>(),
            vec![42, 43, 44]
        );
        assert!(blockers[0].is_handoff_caller);
        assert_eq!(blockers[1].executable_name, "BitFun");
        assert_eq!(blockers[2].executable_name, "Custom Product");
    }

    #[test]
    fn macos_inventory_rejects_empty_or_malformed_output() {
        for output in [
            "",
            " \n",
            "42",
            "0 /bin/app",
            "pid /bin/app",
            "42 ",
            "42 /",
            "42 /bin/app\ninvalid",
        ] {
            assert!(
                matches!(
                    parse_macos_process_entries(output),
                    Err(LegacyMigrationError::ProcessInspection(_))
                ),
                "accepted invalid process inventory: {output:?}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_process_inventory_includes_current_executable() {
        let entries = platform_process_entries().expect("inspect macOS processes");
        let current = entries
            .iter()
            .find(|entry| entry.process_id == std::process::id())
            .expect("current process is present");
        let executable = std::env::current_exe().expect("current executable");
        assert_eq!(
            current.executable_name,
            executable.file_name().unwrap().to_string_lossy()
        );
    }

    fn roots(root: &Path) -> MigrationRoots {
        MigrationRoots {
            legacy_user_root: root.join("legacy/user"),
            legacy_home_root: root.join("legacy/home"),
            legacy_skills_root: root.join("legacy/skills"),
            legacy_ssh_root: root.join("legacy/ssh"),
            target_user_root: root.join("target/user"),
            target_home_root: root.join("target/home"),
            target_skills_root: root.join("target/skills"),
            target_ssh_root: root.join("target/ssh"),
        }
    }

    fn request(now_ms: i64) -> MigratorHandoffRequest {
        MigratorHandoffRequest {
            protocol_version: CURRENT_MIGRATOR_PROTOCOL_VERSION,
            mode: MigratorRequestMode::Execute,
            origin: MigratorRequestOrigin::Settings,
            run_id: uuid::Uuid::new_v4().to_string(),
            nonce: uuid::Uuid::new_v4().to_string(),
            selection: MigrationSelection::all(),
            caller_process_id: 42,
            product_id: "openbitfun".to_string(),
            release_channel: "stable".to_string(),
            created_at_ms: now_ms,
            expires_at_ms: now_ms + 60_000,
            required_capabilities: BTreeSet::from([
                MigratorProtocolCapability::OfflineExecute,
                MigratorProtocolCapability::JournalRecovery,
            ]),
            ..MigratorHandoffRequest::default()
        }
    }

    fn plan(request: &MigratorHandoffRequest) -> MigrationPlan {
        MigrationPlan {
            format_version: CURRENT_MIGRATION_FORMAT_VERSION,
            run_id: request.run_id.clone(),
            source_fingerprint: "sha256:fixture".to_string(),
            selection: request.selection.clone(),
            plan_hash: "sha256:plan".to_string(),
            ..MigrationPlan::default()
        }
    }

    #[test]
    fn private_request_creation_preserves_owner_and_existing_contents() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let parent = temporary
            .path()
            .join("nested".repeat(20))
            .join("nested".repeat(20));
        fs::create_dir_all(&parent).expect("long request directory");
        let path = parent.join("request.json");
        write_new_private_json(&path, &"original").expect("create owned request");
        verify_current_user_owned(&path).expect("request belongs to current user");
        let original = fs::read(&path).unwrap();
        assert!(matches!(
            write_new_private_json(&path, &"replacement"),
            Err(LegacyMigrationError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists
        ));
        assert_eq!(fs::read(&path).unwrap(), original);
    }

    #[test]
    fn handoff_nonce_can_only_resume_the_same_persisted_plan() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let roots = roots(temporary.path());
        let store = HandoffStore::new(roots, "openbitfun", "stable");
        let request = request(1_000);
        store.write_request(&request, 1_000).expect("write request");
        let handoff = store
            .load_request(&request.run_id, 1_001)
            .expect("load request");
        assert_eq!(handoff.disposition(), HandoffDisposition::Fresh);
        let plan = plan(&request);
        assert_eq!(
            store
                .authorize_plan(&handoff, &plan, 1_002)
                .expect("consume nonce"),
            HandoffDisposition::Fresh
        );

        let recovered = store
            .load_request(&request.run_id, 1_003)
            .expect("load recovery request");
        assert_eq!(recovered.disposition(), HandoffDisposition::Recovery);
        assert_eq!(
            store
                .authorize_plan(&recovered, &plan, 1_004)
                .expect("resume exact plan"),
            HandoffDisposition::Recovery
        );

        let mut different = plan.clone();
        different.plan_hash = "sha256:different".to_string();
        assert!(store.authorize_plan(&recovered, &different, 1_005).is_err());
    }

    #[test]
    fn handoff_rejects_wrong_product_channel_and_expiry() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let store = HandoffStore::new(roots(temporary.path()), "openbitfun", "stable");
        let mut wrong_product = request(1_000);
        wrong_product.product_id = "other".to_string();
        assert!(store.write_request(&wrong_product, 1_000).is_err());

        let mut wrong_channel = request(1_000);
        wrong_channel.release_channel = "nightly".to_string();
        assert!(store.write_request(&wrong_channel, 1_000).is_err());

        let expired = request(1_000);
        assert!(store.write_request(&expired, 100_000).is_err());
    }

    #[test]
    fn inherited_job_retry_requires_debug_and_permission_denied() {
        use std::io::ErrorKind;

        assert!(should_retry_without_job_breakaway(
            true,
            ErrorKind::PermissionDenied
        ));
        assert!(!should_retry_without_job_breakaway(
            false,
            ErrorKind::PermissionDenied
        ));
        assert!(!should_retry_without_job_breakaway(
            true,
            ErrorKind::NotFound
        ));
    }

    #[test]
    fn standalone_process_check_does_not_wait_for_windows_idle_or_itself() {
        let processes = vec![
            ProcessEntry {
                process_id: 0,
                executable_name: "System Idle Process".into(),
            },
            ProcessEntry {
                process_id: 42,
                executable_name: "openbitfun-data-migrator.exe".into(),
            },
            ProcessEntry {
                process_id: 81,
                executable_name: "openbitfun.exe".into(),
            },
        ];
        let blockers = classify_writer_processes(&processes, 0, 42, &[]);
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0].process_id, 81);
        assert!(!blockers[0].is_handoff_caller);
    }

    #[test]
    fn process_classifier_keeps_caller_and_known_writers_only() {
        let processes = vec![
            ProcessEntry {
                process_id: 10,
                executable_name: "renamed-caller.exe".to_string(),
            },
            ProcessEntry {
                process_id: 11,
                executable_name: "bitfun-desktop.exe".to_string(),
            },
            ProcessEntry {
                process_id: 12,
                executable_name: "unrelated.exe".to_string(),
            },
            ProcessEntry {
                process_id: 13,
                executable_name: "openbitfun-data-migrator.exe".to_string(),
            },
        ];
        let blockers = classify_writer_processes(&processes, 10, 13, &[]);
        assert_eq!(
            blockers
                .iter()
                .map(|process| process.process_id)
                .collect::<Vec<_>>(),
            vec![10, 11]
        );
        assert!(blockers[0].is_handoff_caller);
    }

    #[test]
    fn process_classifier_includes_product_projected_writer_names() {
        let processes = vec![ProcessEntry {
            process_id: 21,
            executable_name: platform_binary_filename("acme-desktop"),
        }];

        let blockers = classify_writer_processes(&processes, 99, 100, &["acme-desktop"]);

        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0].process_id, 21);
        assert!(!blockers[0].is_handoff_caller);
    }

    #[test]
    fn trusted_resolver_never_accepts_a_request_supplied_target_path() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let current = temporary
            .path()
            .join(platform_binary_filename("openbitfun-data-migrator"));
        let target = temporary
            .path()
            .join(platform_binary_filename("openbitfun-desktop"));
        fs::write(&current, b"migrator").expect("write current executable");
        fs::write(&target, b"desktop").expect("write target executable");

        let resolved = TrustedInstallationResolver::resolve_sibling(
            &current,
            "openbitfun-data-migrator",
            "openbitfun-desktop",
        )
        .expect("resolve trusted sibling");
        assert_eq!(
            resolved.target_executable(),
            fs::canonicalize(target).unwrap()
        );
        assert!(TrustedInstallationResolver::resolve_sibling(
            &current,
            "openbitfun-data-migrator",
            "../attacker",
        )
        .is_err());
    }
}
