//! Clipboard File API

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct ClipboardFilesResponse {
    pub files: Vec<String>,
    pub is_cut: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteFilesRequest {
    pub source_paths: Vec<String>,
    pub target_directory: String,
    pub is_cut: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteFilesResponse {
    pub success_count: usize,
    pub failed_files: Vec<FailedFile>,
}

#[derive(Debug, Serialize)]
pub struct FailedFile {
    pub path: String,
    pub error: String,
}

#[cfg(target_os = "windows")]
mod windows_clipboard {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    const CF_HDROP: u32 = 15;

    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(hwnd: *mut std::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut std::ffi::c_void;
        fn IsClipboardFormatAvailable(format: u32) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn DragQueryFileW(
            hdrop: *mut std::ffi::c_void,
            file_index: u32,
            file_name: *mut u16,
            buffer_size: u32,
        ) -> u32;
    }

    pub fn get_clipboard_files() -> Result<Vec<String>, String> {
        unsafe {
            if IsClipboardFormatAvailable(CF_HDROP) == 0 {
                return Ok(Vec::new());
            }

            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return Err("Failed to open clipboard".to_string());
            }

            struct ClipboardGuard;
            impl Drop for ClipboardGuard {
                fn drop(&mut self) {
                    unsafe {
                        CloseClipboard();
                    }
                }
            }
            let _guard = ClipboardGuard;

            let hdrop = GetClipboardData(CF_HDROP);
            if hdrop.is_null() {
                return Ok(Vec::new());
            }

            let file_count = DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0);
            if file_count == 0 {
                return Ok(Vec::new());
            }

            let mut files = Vec::with_capacity(file_count as usize);

            for i in 0..file_count {
                let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0);
                if len == 0 {
                    continue;
                }

                let mut buffer: Vec<u16> = vec![0; (len + 1) as usize];
                let actual_len = DragQueryFileW(hdrop, i, buffer.as_mut_ptr(), len + 1);

                if actual_len > 0 {
                    let path = OsString::from_wide(&buffer[..actual_len as usize]);
                    if let Some(path_str) = path.to_str() {
                        files.push(path_str.to_string());
                    }
                }
            }

            Ok(files)
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_clipboard {
    pub fn get_clipboard_files() -> Result<Vec<String>, String> {
        use std::process::Command;

        let output = Command::new("osascript")
            .args(&[
                "-e",
                r#"
                set theFiles to {}
                try
                    set theClip to the clipboard as «class furl»
                    set end of theFiles to POSIX path of theClip
                on error
                    try
                        set theClip to the clipboard as list
                        repeat with aFile in theClip
                            try
                                set end of theFiles to POSIX path of (aFile as alias)
                            end try
                        end repeat
                    end try
                end try
                return theFiles as text
                "#,
            ])
            .output()
            .map_err(|e| format!("Failed to execute osascript: {}", e))?;

        if output.status.success() {
            let paths_str = String::from_utf8_lossy(&output.stdout);
            let files: Vec<String> = paths_str
                .lines()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            Ok(files)
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_clipboard {
    pub fn get_clipboard_files() -> Result<Vec<String>, String> {
        use std::process::Command;

        let output = Command::new("xclip")
            .args(&["-selection", "clipboard", "-t", "text/uri-list", "-o"])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let content = String::from_utf8_lossy(&output.stdout);
                let files: Vec<String> = content
                    .lines()
                    .filter(|line| line.starts_with("file://"))
                    .map(|line| {
                        let path = line.trim_start_matches("file://");
                        urlencoding::decode(path)
                            .map(|s| s.into_owned())
                            .unwrap_or_else(|_| path.to_string())
                    })
                    .collect();
                Ok(files)
            }
            _ => Ok(Vec::new()),
        }
    }
}

fn get_clipboard_files_internal() -> Result<Vec<String>, String> {
    #[cfg(target_os = "windows")]
    {
        windows_clipboard::get_clipboard_files()
    }

    #[cfg(target_os = "macos")]
    {
        macos_clipboard::get_clipboard_files()
    }

    #[cfg(target_os = "linux")]
    {
        linux_clipboard::get_clipboard_files()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Reading clipboard files is not supported on this platform".to_string())
    }
}

#[tauri::command]
pub async fn get_clipboard_files() -> Result<ClipboardFilesResponse, String> {
    match get_clipboard_files_internal() {
        Ok(files) => Ok(ClipboardFilesResponse {
            files,
            is_cut: false,
        }),
        Err(e) => {
            log::error!("Failed to read clipboard files: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn paste_files(request: PasteFilesRequest) -> Result<PasteFilesResponse, String> {
    let target_dir = Path::new(&request.target_directory);

    if !target_dir.exists() {
        return Err(format!(
            "Target directory does not exist: {}",
            request.target_directory
        ));
    }

    if !target_dir.is_dir() {
        return Err(format!(
            "Target path is not a directory: {}",
            request.target_directory
        ));
    }

    let mut success_count = 0;
    let mut failed_files = Vec::new();

    for source_path in &request.source_paths {
        let source = Path::new(source_path);

        if !source.exists() {
            failed_files.push(FailedFile {
                path: source_path.clone(),
                error: "Source file does not exist".to_string(),
            });
            continue;
        }

        let file_name = match source.file_name() {
            Some(name) => name,
            None => {
                failed_files.push(FailedFile {
                    path: source_path.clone(),
                    error: "Failed to get file name".to_string(),
                });
                continue;
            }
        };

        let target_path = target_dir.join(file_name);

        let final_target = if target_path.exists() {
            generate_unique_path(&target_path)
        } else {
            target_path
        };

        let result = if source.is_dir() {
            copy_directory_recursive(source, &final_target)
        } else {
            std::fs::copy(source, &final_target)
                .map(|_| ())
                .map_err(|e| e.to_string())
        };

        match result {
            Ok(_) => {
                success_count += 1;

                if request.is_cut {
                    if source.is_dir() {
                        if let Err(e) = std::fs::remove_dir_all(source) {
                            log::warn!("Failed to remove source directory after cut: {}", e);
                        }
                    } else if let Err(e) = std::fs::remove_file(source) {
                        log::warn!("Failed to remove source file after cut: {}", e);
                    }
                }
            }
            Err(e) => {
                failed_files.push(FailedFile {
                    path: source_path.clone(),
                    error: e,
                });
            }
        }
    }

    Ok(PasteFilesResponse {
        success_count,
        failed_files,
    })
}

fn generate_unique_path(path: &Path) -> std::path::PathBuf {
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let extension = path.extension().and_then(|s| s.to_str());

    let mut counter = 1;
    loop {
        let new_name = if let Some(ext) = extension {
            format!("{} ({}). {}", stem, counter, ext)
        } else {
            format!("{} ({})", stem, counter)
        };

        let new_path = parent.join(&new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}

fn copy_directory_recursive(source: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(target).map_err(|e| format!("Failed to create directory: {}", e))?;

    for entry in
        std::fs::read_dir(source).map_err(|e| format!("Failed to read directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());

        if source_path.is_dir() {
            copy_directory_recursive(&source_path, &target_path)?;
        } else {
            std::fs::copy(&source_path, &target_path)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }

    Ok(())
}
