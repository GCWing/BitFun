use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

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

#[test]
fn interactive_startup_survives_resize_multiline_input_and_emits_cleanup() {
    let storage = tempfile::tempdir().expect("create isolated CLI storage");
    let user_root = storage.path().join("user-root");
    let home_root = storage.path().join("home");
    std::fs::create_dir_all(&user_root).expect("create isolated user root");
    std::fs::create_dir_all(&home_root).expect("create isolated home root");

    let pair = native_pty_system()
        .openpty(INITIAL_SIZE)
        .expect("open native PTY");
    let PtyPair { master, slave } = pair;

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_bitfun-cli"));
    command.cwd(storage.path());
    command.env("BITFUN_E2E_STORAGE_GUARD", "1");
    command.env("BITFUN_E2E_USER_ROOT", &user_root);
    command.env("BITFUN_E2E_HOME", &home_root);
    command.env("HOME", &home_root);
    command.env("USERPROFILE", &home_root);
    command.env("TERM", "xterm-256color");

    let mut child = slave
        .spawn_command(command)
        .expect("spawn bitfun-cli in native PTY");
    drop(slave);

    let mut reader = master.try_clone_reader().expect("clone PTY reader");
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

    wait_for_output(&captured, "\x1b[?2004h", Duration::from_secs(30)).unwrap_or_else(|| {
        terminate(&mut child);
        panic!(
            "interactive startup did not enable bracketed paste; output:\n{}",
            captured_output(&captured)
        );
    });
    #[cfg(unix)]
    assert!(
        captured_output(&captured).contains("\x1b[?1049h"),
        "interactive TUI must enter the alternate screen"
    );

    master.resize(RESIZED_SIZE).expect("resize native PTY");
    assert_eq!(
        master.get_size().expect("read resized PTY dimensions"),
        RESIZED_SIZE
    );

    let mut writer = master.take_writer().expect("take PTY writer");
    #[cfg(unix)]
    writer
        .write_all(b"\x1b[200~alpha\r\nbeta\x1b[201~")
        .expect("send bracketed paste");
    #[cfg(windows)]
    {
        let mut rapid_input = b"alpha".to_vec();
        rapid_input.extend(std::iter::repeat_n(b'a', 251));
        rapid_input.extend_from_slice(b"\rbeta");
        writer
            .write_all(&rapid_input)
            .expect("send rapid multiline key input across the batch boundary");
    }
    writer.flush().expect("flush terminal input");

    wait_for_output(&captured, "alpha", Duration::from_secs(15)).unwrap_or_else(|| {
        terminate(&mut child);
        panic!(
            "interactive startup did not render multiline input; output:\n{}",
            captured_output(&captured)
        );
    });
    wait_for_output(&captured, "beta", Duration::from_secs(15)).unwrap_or_else(|| {
        terminate(&mut child);
        panic!(
            "interactive startup did not render the multiline input tail; output:\n{}",
            captured_output(&captured)
        );
    });
    assert!(
        !captured_output(&captured).contains("Welcome to BitFun CLI!"),
        "multiline input was submitted instead of remaining in the startup editor"
    );

    writer.write_all(&[0x03]).expect("send Ctrl+C");
    writer.flush().expect("flush Ctrl+C");

    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll bitfun-cli process") {
            break status;
        }
        if Instant::now() >= deadline {
            terminate(&mut child);
            panic!(
                "interactive startup did not exit after Ctrl+C; output:\n{}",
                captured_output(&captured)
            );
        }
        thread::sleep(Duration::from_millis(25));
    };

    drop(writer);
    drop(master);
    reader_thread.join().expect("join PTY reader");

    let output = captured_output(&captured);
    assert!(
        status.success(),
        "unexpected process status {status}:\n{output}"
    );
    assert!(
        output.contains("alpha"),
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

fn wait_for_output(
    captured: &Arc<Mutex<Vec<u8>>>,
    expected: &str,
    timeout: Duration,
) -> Option<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if captured_output(captured).contains(expected) {
            return Some(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    None
}

fn captured_output(captured: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&captured.lock().expect("lock captured PTY output")).into_owned()
}

fn terminate(child: &mut Box<dyn portable_pty::Child + Send + Sync>) {
    let _ = child.kill();
    let _ = child.wait();
}
