use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteLocalFileStatus {
    Created,
    Overwritten,
    AlreadyExistsSameContent,
}

impl WriteLocalFileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Overwritten => "overwritten",
            Self::AlreadyExistsSameContent => "already_exists_same_content",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteLocalFileRequest {
    pub logical_path: String,
    pub resolved_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteLocalFileOutcome {
    pub status: WriteLocalFileStatus,
    pub bytes_written: usize,
    pub lines_written: usize,
    pub assistant_message: String,
}

fn count_written_lines(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count().max(1)
    }
}

pub fn write_local_file(request: WriteLocalFileRequest) -> Result<WriteLocalFileOutcome, String> {
    let file_already_exists = request.resolved_path.exists();
    if file_already_exists {
        let existing = fs::read(&request.resolved_path).map_err(|error| {
            format!(
                "Failed to read existing file {}: {}",
                request.logical_path, error
            )
        })?;
        if existing == request.content.as_bytes() {
            return Ok(WriteLocalFileOutcome {
                status: WriteLocalFileStatus::AlreadyExistsSameContent,
                bytes_written: 0,
                lines_written: 0,
                assistant_message: format!(
                    "Write skipped because {} already exists with identical content.",
                    request.logical_path
                ),
            });
        }
    }

    if let Some(parent) = request.resolved_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create directory: {}", error))?;
    }

    fs::write(&request.resolved_path, &request.content)
        .map_err(|error| format!("Failed to write file {}: {}", request.logical_path, error))?;

    let status = if file_already_exists {
        WriteLocalFileStatus::Overwritten
    } else {
        WriteLocalFileStatus::Created
    };
    let verb = match status {
        WriteLocalFileStatus::Created => "created",
        WriteLocalFileStatus::Overwritten => "overwrote",
        WriteLocalFileStatus::AlreadyExistsSameContent => unreachable!(),
    };

    Ok(WriteLocalFileOutcome {
        status,
        bytes_written: request.content.len(),
        lines_written: count_written_lines(&request.content),
        assistant_message: format!(
            "Successfully {} {} ({} bytes).",
            verb,
            request.logical_path,
            request.content.len()
        ),
    })
}
