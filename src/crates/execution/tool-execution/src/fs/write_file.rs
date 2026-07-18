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

pub fn write_same_content_outcome(logical_path: &str) -> WriteLocalFileOutcome {
    WriteLocalFileOutcome {
        status: WriteLocalFileStatus::AlreadyExistsSameContent,
        bytes_written: 0,
        lines_written: 0,
        assistant_message: format!(
            "Write skipped because {} already exists with identical content.",
            logical_path
        ),
    }
}

pub fn write_file_success_outcome(
    logical_path: &str,
    file_already_exists: bool,
    content: &str,
) -> WriteLocalFileOutcome {
    let (status, verb) = if file_already_exists {
        (WriteLocalFileStatus::Overwritten, "overwrote")
    } else {
        (WriteLocalFileStatus::Created, "created")
    };

    let lines_written = count_written_lines(content);
    WriteLocalFileOutcome {
        status,
        bytes_written: content.len(),
        lines_written,
        assistant_message: format!(
            "Successfully {} {} ({} lines, {} bytes).",
            verb,
            logical_path,
            lines_written,
            content.len()
        ),
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
            return Ok(write_same_content_outcome(&request.logical_path));
        }
    }

    if let Some(parent) = request.resolved_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create directory: {}", error))?;
    }

    fs::write(&request.resolved_path, &request.content)
        .map_err(|error| format!("Failed to write file {}: {}", request.logical_path, error))?;

    Ok(write_file_success_outcome(
        &request.logical_path,
        file_already_exists,
        &request.content,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        count_written_lines, write_file_success_outcome, write_same_content_outcome,
        WriteLocalFileStatus,
    };

    #[test]
    fn counts_empty_and_trailing_newline_writes_like_existing_tool() {
        assert_eq!(count_written_lines(""), 0);
        assert_eq!(count_written_lines("one"), 1);
        assert_eq!(count_written_lines("one\n"), 1);
        assert_eq!(count_written_lines("one\ntwo"), 2);
    }

    #[test]
    fn builds_success_outcome_for_create_and_overwrite() {
        let created = write_file_success_outcome("new.txt", false, "alpha");
        assert_eq!(created.status, WriteLocalFileStatus::Created);
        assert_eq!(created.bytes_written, 5);
        assert_eq!(created.lines_written, 1);
        assert_eq!(
            created.assistant_message,
            "Successfully created new.txt (1 lines, 5 bytes)."
        );

        let overwritten = write_file_success_outcome("existing.txt", true, "alpha");
        assert_eq!(overwritten.status, WriteLocalFileStatus::Overwritten);
        assert_eq!(overwritten.lines_written, 1);
        assert_eq!(
            overwritten.assistant_message,
            "Successfully overwrote existing.txt (1 lines, 5 bytes)."
        );
    }

    #[test]
    fn builds_same_content_outcome() {
        let outcome = write_same_content_outcome("existing.txt");

        assert_eq!(
            outcome.status,
            WriteLocalFileStatus::AlreadyExistsSameContent
        );
        assert_eq!(outcome.bytes_written, 0);
        assert_eq!(outcome.lines_written, 0);
        assert!(outcome.assistant_message.contains("identical content"));
    }
}
