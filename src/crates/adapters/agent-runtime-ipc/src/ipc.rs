use crate::RuntimeInstanceIdentity;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
#[cfg(windows)]
use std::time::Instant;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

#[cfg(unix)]
const MAX_PORTABLE_UDS_PATH_BYTES: usize = 103;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalIpcEndpoint {
    discovery_value: String,
    #[cfg(unix)]
    path: PathBuf,
}

impl LocalIpcEndpoint {
    pub(crate) fn for_instance(
        runtime_root: &Path,
        identity: &RuntimeInstanceIdentity,
    ) -> Result<Self, RuntimeIpcTransportError> {
        let suffix = &identity.as_str()[..16];
        #[cfg(windows)]
        {
            let _ = runtime_root;
            Ok(Self {
                discovery_value: format!(r"\\.\pipe\openbitfun-agent-runtime-{suffix}"),
            })
        }
        #[cfg(unix)]
        {
            let root = dunce::canonicalize(runtime_root)
                .map_err(RuntimeIpcTransportError::CanonicalizeRuntimeRoot)?;
            let discovery_value = format!("openbitfun-ar-{suffix}.sock");
            let path = root.join(&discovery_value);
            validate_uds_path_length(&path)?;
            Ok(Self {
                discovery_value,
                path,
            })
        }
    }

    pub(crate) fn parse_for_root(
        value: &str,
        runtime_root: &Path,
        identity: &RuntimeInstanceIdentity,
    ) -> Result<Self, RuntimeIpcTransportError> {
        let expected = Self::for_instance(runtime_root, identity)?;
        if value != expected.discovery_value {
            return Err(RuntimeIpcTransportError::InvalidEndpoint);
        }
        Ok(expected)
    }

    pub(crate) fn discovery_value(&self) -> &str {
        &self.discovery_value
    }

    #[cfg(unix)]
    pub(crate) fn as_path(&self) -> &Path {
        &self.path
    }

    #[cfg(windows)]
    fn as_pipe_name(&self) -> &str {
        &self.discovery_value
    }

    pub(crate) async fn connect(
        &self,
        deadline: Duration,
    ) -> Result<LocalIpcStream, RuntimeIpcTransportError> {
        if deadline.is_zero() {
            return Err(RuntimeIpcTransportError::InvalidDeadline);
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;
            let expires_at = Instant::now() + deadline;
            loop {
                match ClientOptions::new().open(self.as_pipe_name()) {
                    Ok(client) => return Ok(LocalIpcStream::WindowsClient(client)),
                    Err(error) if Instant::now() < expires_at => {
                        let remaining = expires_at.saturating_duration_since(Instant::now());
                        tokio::time::sleep(remaining.min(Duration::from_millis(10))).await;
                        if error.kind() != std::io::ErrorKind::NotFound
                            && error.raw_os_error() != Some(231)
                        {
                            return Err(RuntimeIpcTransportError::Connect(error));
                        }
                    }
                    Err(error) => return Err(RuntimeIpcTransportError::Connect(error)),
                }
            }
        }
        #[cfg(unix)]
        {
            let stream =
                tokio::time::timeout(deadline, tokio::net::UnixStream::connect(self.as_path()))
                    .await
                    .map_err(|_| RuntimeIpcTransportError::ConnectTimeout)?
                    .map_err(RuntimeIpcTransportError::Connect)?;
            Ok(LocalIpcStream::Unix(stream))
        }
    }
}

#[cfg(unix)]
fn validate_uds_path_length(path: &Path) -> Result<(), RuntimeIpcTransportError> {
    use std::os::unix::ffi::OsStrExt;
    let observed = path.as_os_str().as_bytes().len();
    if observed > MAX_PORTABLE_UDS_PATH_BYTES {
        return Err(RuntimeIpcTransportError::EndpointTooLong {
            observed,
            maximum: MAX_PORTABLE_UDS_PATH_BYTES,
        });
    }
    Ok(())
}

pub(crate) struct LocalIpcListener {
    endpoint: LocalIpcEndpoint,
    #[cfg(windows)]
    server: Option<tokio::net::windows::named_pipe::NamedPipeServer>,
    #[cfg(unix)]
    listener: tokio::net::UnixListener,
}

impl LocalIpcListener {
    pub(crate) async fn bind(endpoint: LocalIpcEndpoint) -> Result<Self, RuntimeIpcTransportError> {
        #[cfg(windows)]
        {
            let server = create_windows_server(&endpoint, true)?;
            Ok(Self {
                endpoint,
                server: Some(server),
            })
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = endpoint.as_path();
            remove_stale_unix_socket(path)?;
            let listener =
                tokio::net::UnixListener::bind(path).map_err(RuntimeIpcTransportError::Bind)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(RuntimeIpcTransportError::Bind)?;
            Ok(Self { endpoint, listener })
        }
    }

    pub(crate) async fn accept(&mut self) -> Result<LocalIpcStream, RuntimeIpcTransportError> {
        #[cfg(windows)]
        {
            self.server
                .as_mut()
                .expect("Windows listener always owns an accept instance")
                .connect()
                .await
                .map_err(RuntimeIpcTransportError::Accept)?;
            let server = self
                .server
                .take()
                .expect("connected Windows listener instance must remain owned");
            self.server = Some(create_windows_server(&self.endpoint, false)?);
            Ok(LocalIpcStream::WindowsServer(server))
        }
        #[cfg(unix)]
        {
            let (stream, _) = self
                .listener
                .accept()
                .await
                .map_err(RuntimeIpcTransportError::Accept)?;
            Ok(LocalIpcStream::Unix(stream))
        }
    }
}

#[cfg(windows)]
fn create_windows_server(
    endpoint: &LocalIpcEndpoint,
    first: bool,
) -> Result<tokio::net::windows::named_pipe::NamedPipeServer, RuntimeIpcTransportError> {
    use tokio::net::windows::named_pipe::ServerOptions;
    ServerOptions::new()
        .first_pipe_instance(first)
        .reject_remote_clients(true)
        .create(endpoint.as_pipe_name())
        .map_err(RuntimeIpcTransportError::Bind)
}

#[cfg(unix)]
fn remove_stale_unix_socket(path: &Path) -> Result<(), RuntimeIpcTransportError> {
    use std::os::unix::fs::FileTypeExt;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => {
            std::fs::remove_file(path).map_err(RuntimeIpcTransportError::Bind)
        }
        Ok(_) => Err(RuntimeIpcTransportError::EndpointOccupied),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(RuntimeIpcTransportError::Bind(error)),
    }
}

impl Drop for LocalIpcListener {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = remove_stale_unix_socket(self.endpoint.as_path());
        }
    }
}

pub(crate) enum LocalIpcStream {
    #[cfg(windows)]
    WindowsClient(tokio::net::windows::named_pipe::NamedPipeClient),
    #[cfg(windows)]
    WindowsServer(tokio::net::windows::named_pipe::NamedPipeServer),
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
}

impl AsyncRead for LocalIpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            #[cfg(windows)]
            Self::WindowsClient(stream) => Pin::new(stream).poll_read(context, buffer),
            #[cfg(windows)]
            Self::WindowsServer(stream) => Pin::new(stream).poll_read(context, buffer),
            #[cfg(unix)]
            Self::Unix(stream) => Pin::new(stream).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for LocalIpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            #[cfg(windows)]
            Self::WindowsClient(stream) => Pin::new(stream).poll_write(context, buffer),
            #[cfg(windows)]
            Self::WindowsServer(stream) => Pin::new(stream).poll_write(context, buffer),
            #[cfg(unix)]
            Self::Unix(stream) => Pin::new(stream).poll_write(context, buffer),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            #[cfg(windows)]
            Self::WindowsClient(stream) => Pin::new(stream).poll_flush(context),
            #[cfg(windows)]
            Self::WindowsServer(stream) => Pin::new(stream).poll_flush(context),
            #[cfg(unix)]
            Self::Unix(stream) => Pin::new(stream).poll_flush(context),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            #[cfg(windows)]
            Self::WindowsClient(stream) => Pin::new(stream).poll_shutdown(context),
            #[cfg(windows)]
            Self::WindowsServer(stream) => Pin::new(stream).poll_shutdown(context),
            #[cfg(unix)]
            Self::Unix(stream) => Pin::new(stream).poll_shutdown(context),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeIpcTransportError {
    #[error("runtime IPC endpoint is invalid")]
    InvalidEndpoint,
    #[cfg(unix)]
    #[error("runtime IPC endpoint path is too long: {observed} bytes exceeds {maximum}")]
    EndpointTooLong { observed: usize, maximum: usize },
    #[cfg(unix)]
    #[error("runtime IPC endpoint path is occupied by a non-socket entry")]
    EndpointOccupied,
    #[error("runtime IPC deadline must be greater than zero")]
    InvalidDeadline,
    #[cfg(unix)]
    #[error("failed to canonicalize runtime IPC directory")]
    CanonicalizeRuntimeRoot(#[source] std::io::Error),
    #[error("failed to bind runtime IPC endpoint")]
    Bind(#[source] std::io::Error),
    #[error("failed to accept runtime IPC connection")]
    Accept(#[source] std::io::Error),
    #[error("failed to connect to runtime IPC endpoint")]
    Connect(#[source] std::io::Error),
    #[cfg(unix)]
    #[error("timed out connecting to runtime IPC endpoint")]
    ConnectTimeout,
}
