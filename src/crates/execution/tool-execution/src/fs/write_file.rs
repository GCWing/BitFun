use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteLocalFileStatus {
    Created,
    Overwritten,
    Appended,
    AlreadyExistsSameContent,
}

impl WriteLocalFileStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Overwritten => "overwritten",
            Self::Appended => "appended",
            Self::AlreadyExistsSameContent => "already_exists_same_content",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteLocalFileMode {
    Write,
    Append,
}

impl WriteLocalFileMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Write => "w",
            Self::Append => "a",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteLocalFileRequest {
    pub logical_path: String,
    pub resolved_path: PathBuf,
    pub content: String,
    pub mode: WriteLocalFileMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteLocalFileOutcome {
    pub status: WriteLocalFileStatus,
    pub bytes_written: usize,
    pub lines_written: usize,
    pub assistant_message: String,
}

pub fn parse_write_local_file_mode(mode: Option<&str>) -> Result<WriteLocalFileMode, String> {
    match mode.unwrap_or("w") {
        "w" => Ok(WriteLocalFileMode::Write),
        "a" => Ok(WriteLocalFileMode::Append),
        other => Err(format!(
            "mode must be either 'w' (overwrite) or 'a' (append), got '{}'",
            other
        )),
    }
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
    mode: WriteLocalFileMode,
    file_already_exists: bool,
    content: &str,
) -> WriteLocalFileOutcome {
    let status = match (mode, file_already_exists) {
        (WriteLocalFileMode::Write, true) => WriteLocalFileStatus::Overwritten,
        (WriteLocalFileMode::Write, false) => WriteLocalFileStatus::Created,
        (WriteLocalFileMode::Append, true) => WriteLocalFileStatus::Appended,
        (WriteLocalFileMode::Append, false) => WriteLocalFileStatus::Created,
    };
    let verb = match status {
        WriteLocalFileStatus::Created => "created",
        WriteLocalFileStatus::Overwritten => "overwrote",
        WriteLocalFileStatus::Appended => "appended to",
        WriteLocalFileStatus::AlreadyExistsSameContent => unreachable!(),
    };

    WriteLocalFileOutcome {
        status,
        bytes_written: content.len(),
        lines_written: count_written_lines(content),
        assistant_message: format!(
            "Successfully {} {} ({} bytes).",
            verb,
            logical_path,
            content.len()
        ),
    }
}

pub fn write_local_file(request: WriteLocalFileRequest) -> Result<WriteLocalFileOutcome, String> {
    let file_already_exists = request.resolved_path.exists();
    if request.mode == WriteLocalFileMode::Write && file_already_exists {
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

    match request.mode {
        WriteLocalFileMode::Write => {
            fs::write(&request.resolved_path, &request.content).map_err(|error| {
                format!("Failed to write file {}: {}", request.logical_path, error)
            })?;
        }
        WriteLocalFileMode::Append => {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&request.resolved_path)
                .map_err(|error| {
                    format!(
                        "Failed to open file {} for append: {}",
                        request.logical_path, error
                    )
                })?;
            file.write_all(request.content.as_bytes())
                .map_err(|error| {
                    format!(
                        "Failed to append to file {}: {}",
                        request.logical_path, error
                    )
                })?;
        }
    };

    Ok(write_file_success_outcome(
        &request.logical_path,
        request.mode,
        file_already_exists,
        &request.content,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        count_written_lines, parse_write_local_file_mode, write_file_success_outcome,
        write_same_content_outcome, WriteLocalFileMode, WriteLocalFileStatus,
    };

    #[test]
    fn parses_write_modes_with_default() {
        assert_eq!(
            parse_write_local_file_mode(None).expect("default mode"),
            WriteLocalFileMode::Write
        );
        assert_eq!(
            parse_write_local_file_mode(Some("a")).expect("append mode"),
            WriteLocalFileMode::Append
        );
        assert_eq!(
            parse_write_local_file_mode(Some("x")).expect_err("invalid mode"),
            "mode must be either 'w' (overwrite) or 'a' (append), got 'x'"
        );
    }

    #[test]
    fn counts_empty_and_trailing_newline_writes_like_existing_tool() {
        assert_eq!(count_written_lines(""), 0);
        assert_eq!(count_written_lines("one"), 1);
        assert_eq!(count_written_lines("one\n"), 1);
        assert_eq!(count_written_lines("one\ntwo"), 2);
    }

    #[test]
    fn builds_success_outcome_for_write_and_append_modes() {
        let created =
            write_file_success_outcome("new.txt", WriteLocalFileMode::Write, false, "alpha");
        assert_eq!(created.status, WriteLocalFileStatus::Created);
        assert_eq!(created.bytes_written, 5);
        assert_eq!(created.lines_written, 1);
        assert_eq!(
            created.assistant_message,
            "Successfully created new.txt (5 bytes)."
        );

        let appended =
            write_file_success_outcome("log.txt", WriteLocalFileMode::Append, true, "\nalpha");
        assert_eq!(appended.status, WriteLocalFileStatus::Appended);
        assert_eq!(appended.lines_written, 2);
        assert_eq!(
            appended.assistant_message,
            "Successfully appended to log.txt (6 bytes)."
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
