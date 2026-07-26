mod support;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use support::{CliTestEnvironment, MockOpenAiServer, STREAM_RESIZED_MARKER, STREAM_START_MARKER};

const INITIAL_SIZE: PtySize = PtySize {
    rows: 30,
    cols: 100,
    pixel_width: 0,
    pixel_height: 0,
};
const RESIZED_SIZE: PtySize = PtySize {
    rows: 40,
    cols: 120,
    pixel_width: 0,
    pixel_height: 0,
};
const EXEC_STREAM_SIZE: PtySize = PtySize {
    rows: 30,
    cols: 4096,
    pixel_width: 0,
    pixel_height: 0,
};
const STARTUP_INPUT: &[u8] = b"exercise active turn resize Q7Z9";
const STARTUP_INPUT_SENTINEL: &str = "Q7Z9";
const MULTILINE_INPUT_SENTINEL: &str = "M7Q4";
const RECOVERY_INPUT: &[u8] = b"READY_AFTER_CANCEL K4W8";
const RECOVERY_INPUT_SENTINEL: &str = "K4W8";

#[test]
fn interactive_startup_survives_resize_multiline_input_and_emits_cleanup() {
    let environment = CliTestEnvironment::new();
    let mut process = PtyProcess::spawn(environment.pty_command(), INITIAL_SIZE);

    process.expect_output(
        "\x1b[?2004h",
        Duration::from_secs(30),
        "interactive startup did not enable bracketed paste",
    );
    #[cfg(unix)]
    assert!(
        process.output().contains("\x1b[?1049h"),
        "interactive TUI must enter the alternate screen"
    );

    process.resize(RESIZED_SIZE);

    // Ratatui emits only changed cells, so the sentinel must not share characters
    // with the startup placeholder at the same screen positions.
    #[cfg(unix)]
    process.write(b"\x1b[200~M7Q4\r\nbeta\x1b[201~");
    #[cfg(windows)]
    {
        let mut rapid_input = MULTILINE_INPUT_SENTINEL.as_bytes().to_vec();
        rapid_input.extend(std::iter::repeat_n(b'a', 251));
        rapid_input.extend_from_slice(b"\rbeta");
        process.write(&rapid_input);
    }

    process.expect_output(
        MULTILINE_INPUT_SENTINEL,
        Duration::from_secs(15),
        "interactive startup did not render multiline input",
    );
    process.expect_output(
        "beta",
        Duration::from_secs(15),
        "interactive startup did not render the multiline input tail",
    );
    assert!(
        !process.output().contains("Welcome to BitFun CLI!"),
        "multiline input was submitted instead of remaining in the startup editor"
    );

    process.write(&[0x03]);
    let (status, output) = process.finish(Duration::from_secs(15));
    assert!(
        status.success(),
        "unexpected process status {status}:\n{output}"
    );
    assert!(
        output.contains(MULTILINE_INPUT_SENTINEL),
        "paste text was not rendered:\n{output}"
    );
    assert!(
        output.contains("beta"),
        "paste tail was not rendered:\n{output}"
    );
    assert!(
        !output.contains("[200~") && !output.contains("[201~"),
        "bracketed-paste markers leaked into input:\n{output}"
    );
    assert!(
        output.contains("\x1b[?2004l"),
        "bracketed paste was not disabled:\n{output}"
    );
    #[cfg(unix)]
    assert!(
        output.contains("\x1b[?1049l"),
        "alternate screen was not left:\n{output}"
    );
    assert!(
        output.contains("\x1b[?25h"),
        "cursor was not restored:\n{output}"
    );
    assert!(
        output.contains("Goodbye!"),
        "clean exit was not reported:\n{output}"
    );
}

#[test]
fn active_turn_resize_can_be_cancelled_and_returns_to_editable_input() {
    let server = MockOpenAiServer::gated();
    let environment = CliTestEnvironment::new();
    environment.initialize_git_repository();
    environment.configure_mock_model(server.base_url());
    let mut process = PtyProcess::spawn(environment.pty_command(), INITIAL_SIZE);

    process.expect_output(
        "\x1b[?2004h",
        Duration::from_secs(30),
        "interactive startup did not enable bracketed paste",
    );
    process.write(STARTUP_INPUT);
    process.expect_output(
        STARTUP_INPUT_SENTINEL,
        Duration::from_secs(15),
        "startup prompt sentinel was not rendered before submission",
    );
    process.write(b"\r");
    process.expect_output(
        STREAM_START_MARKER,
        Duration::from_secs(30),
        "active model stream was not rendered",
    );

    let active_turn_size = PtySize {
        rows: 24,
        cols: 52,
        pixel_width: 0,
        pixel_height: 0,
    };
    process.resize(active_turn_size);
    server.release();
    process.expect_output(
        STREAM_RESIZED_MARKER,
        Duration::from_secs(15),
        "active model stream did not remain renderable after resize",
    );
    process.write(&[0x03]);
    process.expect_output(
        "Cancelled",
        Duration::from_secs(15),
        "active turn did not reach the cancelled state after resize",
    );
    server.expect_stream_disconnect(Duration::from_secs(5));

    process.write(RECOVERY_INPUT);
    process.expect_output(
        RECOVERY_INPUT_SENTINEL,
        Duration::from_secs(15),
        "recovery input sentinel was not rendered after cancellation",
    );
    process.write(&[0x03]);

    let (status, output) = process.finish(Duration::from_secs(15));
    assert!(
        status.success(),
        "unexpected process status {status}:\n{output}"
    );
    assert!(output.contains(STREAM_START_MARKER), "{output}");
    assert!(output.contains(STREAM_RESIZED_MARKER), "{output}");
    assert!(output.contains("Cancelled"), "{output}");
    assert!(output.contains(RECOVERY_INPUT_SENTINEL), "{output}");
    assert!(output.contains("\x1b[?2004l"), "{output}");
    assert!(output.contains("\x1b[?25h"), "{output}");
    assert!(output.contains("Goodbye!"), "{output}");
}

#[test]
fn exec_stream_json_ctrl_c_emits_one_cancelled_terminal_and_disconnects() {
    assert_exec_stream_json_ctrl_c_contract(false);
}

#[test]
fn legacy_exec_stream_json_ctrl_c_emits_one_cancelled_terminal_and_disconnects() {
    assert_exec_stream_json_ctrl_c_contract(true);
}

fn assert_exec_stream_json_ctrl_c_contract(deprecated_entrypoint: bool) {
    let server = MockOpenAiServer::gated();
    let environment = CliTestEnvironment::new();
    environment.configure_mock_model(server.base_url());
    let mut command = if deprecated_entrypoint {
        environment.deprecated_pty_command()
    } else {
        environment.pty_command()
    };
    command.args([
        "exec",
        "exercise interrupt contract",
        "--output-format",
        "stream-json",
    ]);
    let mut process = PtyProcess::spawn(command, EXEC_STREAM_SIZE);

    process.expect_output(
        STREAM_START_MARKER,
        Duration::from_secs(30),
        "exec model stream did not start",
    );
    process.write(&[0x03]);
    server.expect_stream_disconnect(Duration::from_secs(5));

    let (status, output) = process.finish(Duration::from_secs(15));
    assert_eq!(
        status.exit_code(),
        1,
        "interrupt exit code changed:\n{output}"
    );
    assert!(
        output.contains("BITFUN_EXIT: cancelled:"),
        "missing stable cancellation diagnostic:\n{output}"
    );
    let events = strict_stream_json_events(&output);
    let terminal_events = events
        .iter()
        .filter(|value| {
            matches!(
                value["event"]["type"].as_str(),
                Some(
                    "DialogTurnCompleted"
                        | "DialogTurnCancelled"
                        | "DialogTurnFailed"
                        | "SystemError"
                )
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        terminal_events.len(),
        1,
        "interrupt must emit exactly one terminal envelope:\n{output}"
    );
    let raw_terminal_count = [
        "DialogTurnCompleted",
        "DialogTurnCancelled",
        "DialogTurnFailed",
        "SystemError",
    ]
    .iter()
    .map(|event_type| {
        output
            .matches(&format!("\"type\":\"{event_type}\""))
            .count()
    })
    .sum::<usize>();
    assert_eq!(
        raw_terminal_count, 1,
        "raw PTY output contains a hidden or duplicate terminal envelope:\n{output}"
    );
    assert_eq!(
        terminal_events[0]["event"]["type"], "DialogTurnCancelled",
        "interrupt must settle as cancelled:\n{output}"
    );
    assert_eq!(
        events.last().expect("stream-json cancellation event")["event"]["type"],
        "DialogTurnCancelled",
        "cancellation must be the final protocol envelope:\n{output}"
    );
    let deprecation_count = output
        .matches("Warning: `bitfun-cli` is deprecated; use `bitfun` instead.")
        .count();
    assert_eq!(
        deprecation_count,
        usize::from(deprecated_entrypoint),
        "deprecated warning count changed:\n{output}"
    );
}

fn strict_stream_json_events(output: &str) -> Vec<serde_json::Value> {
    output
        .lines()
        .filter_map(|raw_line| {
            let line = strip_terminal_sequences(raw_line);
            let line = line.strip_prefix("^C").unwrap_or(&line);
            let is_protocol_candidate = line.contains('{')
                || line.contains("\"event\"")
                || [
                    "DialogTurnCompleted",
                    "DialogTurnCancelled",
                    "DialogTurnFailed",
                    "SystemError",
                ]
                .iter()
                .any(|event_type| line.contains(event_type));
            if !is_protocol_candidate {
                return None;
            }
            let value = serde_json::from_str::<serde_json::Value>(&line).unwrap_or_else(|error| {
                panic!("invalid stream-json PTY line {line:?}: {error}\nfull output:\n{output}")
            });
            assert!(
                value.get("event").is_some(),
                "stream-json PTY record is not an Agentic envelope: {line:?}"
            );
            Some(value)
        })
        .collect()
}

#[test]
fn stream_json_parser_accepts_the_echoed_ctrl_c_prefix_before_an_envelope() {
    let events = strict_stream_json_events(
        "^C{\"id\":\"event-1\",\"event\":{\"type\":\"SessionStateChanged\"}}\n",
    );

    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"]["type"], "SessionStateChanged");
}

fn strip_terminal_sequences(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\x1b' {
            if character != '\r' {
                output.push(character);
            }
            continue;
        }
        match chars.next() {
            Some('[') => {
                for next in chars.by_ref() {
                    if next.is_ascii() && (0x40..=0x7e).contains(&(next as u8)) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            Some(_) | None => {}
        }
    }
    output
}

fn captured_output(captured: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&captured.lock().expect("lock captured PTY output")).into_owned()
}

struct PtyProcess {
    master: Option<Box<dyn portable_pty::MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    captured: Arc<Mutex<Vec<u8>>>,
    reader_thread: Option<thread::JoinHandle<()>>,
}

impl PtyProcess {
    fn spawn(command: CommandBuilder, size: PtySize) -> Self {
        let pair = native_pty_system().openpty(size).expect("open native PTY");
        let mut child = pair
            .slave
            .spawn_command(command)
            .expect("spawn BitFun CLI in native PTY");
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().expect("clone PTY reader");
        let writer = pair.master.take_writer().expect("take PTY writer");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let reader_capture = Arc::clone(&captured);
        let reader_thread = thread::spawn(move || {
            let mut chunk = [0_u8; 4096];
            while let Ok(read) = reader.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                reader_capture
                    .lock()
                    .expect("lock captured PTY output")
                    .extend_from_slice(&chunk[..read]);
            }
        });

        // Detect an immediate startup failure before handing the process to the test.
        if let Some(status) = child.try_wait().expect("poll initial CLI process") {
            panic!("BitFun CLI exited during PTY startup: {status}");
        }

        Self {
            master: Some(pair.master),
            writer: Some(writer),
            child: Some(child),
            captured,
            reader_thread: Some(reader_thread),
        }
    }

    fn output(&self) -> String {
        captured_output(&self.captured)
    }

    fn expect_output(&mut self, expected: &str, timeout: Duration, context: &str) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.output().contains(expected) {
                return;
            }
            if let Some(status) = self
                .child
                .as_mut()
                .expect("PTY process child")
                .try_wait()
                .expect("poll BitFun CLI process")
            {
                let output = self.output();
                self.close_io();
                panic!("{context}; process exited with {status}; output:\n{output}");
            }
            thread::sleep(Duration::from_millis(25));
        }
        let output = self.output();
        self.terminate();
        panic!("{context}; output:\n{output}");
    }

    fn resize(&self, size: PtySize) {
        let master = self.master.as_ref().expect("PTY master");
        master.resize(size).expect("resize native PTY");
        assert_eq!(
            master.get_size().expect("read resized PTY dimensions"),
            size
        );
    }

    fn write(&mut self, bytes: &[u8]) {
        let writer = self.writer.as_mut().expect("PTY writer");
        writer.write_all(bytes).expect("write terminal input");
        writer.flush().expect("flush terminal input");
    }

    fn finish(mut self, timeout: Duration) -> (portable_pty::ExitStatus, String) {
        let deadline = Instant::now() + timeout;
        let status = loop {
            if let Some(status) = self
                .child
                .as_mut()
                .expect("PTY process child")
                .try_wait()
                .expect("poll BitFun CLI process")
            {
                break status;
            }
            if Instant::now() >= deadline {
                let output = self.output();
                self.terminate();
                panic!("interactive process did not exit; output:\n{output}");
            }
            thread::sleep(Duration::from_millis(25));
        };
        self.child.take();
        self.close_io();
        (status, self.output())
    }

    fn close_io(&mut self) {
        self.writer.take();
        self.master.take();
        if let Some(reader_thread) = self.reader_thread.take() {
            reader_thread.join().expect("join PTY reader");
        }
    }

    fn terminate(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.close_io();
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}
