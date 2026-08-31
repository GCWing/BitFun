//! Controlled execution fixtures only. No archived command enters this module.
use super::*;
use crate::agentic::execution::edit_constraint_guard::{
    ConstraintMatcher, ConstraintOperationScope, ConstraintSource, EditConstraintState,
    ExtractedConstraint,
};
use crate::agentic::WorkspaceBinding;
use bitfun_runtime_ports::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex as StdMutex;

pub(crate) fn guard_state(root: &str) -> EditConstraintState {
    EditConstraintState {
        constraints: vec![ExtractedConstraint {
            id: "fixture:protected".into(),
            description: "Do not modify the protected fixture repository".into(),
            operation_scope: ConstraintOperationScope::All,
            matcher: ConstraintMatcher::PathUnderDir {
                dirs: vec![root.into()],
            },
            source: ConstraintSource::Legacy,
            source_text: None,
        }],
        ..Default::default()
    }
}
#[derive(Debug, Default)]
struct FixtureTerminal {
    calls: AtomicUsize,
    requests: StdMutex<Vec<TerminalExecCommandRequest>>,
}
fn response(output: String, code: i32) -> TerminalExecCommandResponse {
    TerminalExecCommandResponse {
        chunk_id: "fixture".into(),
        wall_time_seconds: 0.0,
        original_output_chars: output.chars().count(),
        output,
        session_id: None,
        exit_code: Some(code),
        completion: None,
    }
}
impl RuntimeServicePort for FixtureTerminal {
    fn capability(&self) -> RuntimeServiceCapability {
        RuntimeServiceCapability::Terminal
    }
}
#[async_trait]
impl TerminalPort for FixtureTerminal {
    async fn exec_command(
        &self,
        r: TerminalExecCommandRequest,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.requests.lock().unwrap().push(r.clone());
        let out = bitfun_services_core::process_manager::create_command(&r.argv[0])
            .args(&r.argv[1..])
            .current_dir(&r.cwd)
            .envs(&r.env)
            .output()
            .unwrap();
        let mut bytes = out.stdout;
        bytes.extend(out.stderr);
        Ok(response(
            String::from_utf8(bytes).unwrap(),
            out.status.code().unwrap_or(-1),
        ))
    }
    async fn exec_command_streaming(
        &self,
        r: TerminalExecCommandRequest,
        _: TerminalExecStreamingOutputSink,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.exec_command(r).await
    }
    async fn write_stdin(
        &self,
        _: TerminalWriteStdinRequest,
    ) -> PortResult<TerminalExecCommandResponse> {
        Ok(response("".into(), 0))
    }
    async fn write_stdin_streaming(
        &self,
        r: TerminalWriteStdinRequest,
        _: TerminalExecStreamingOutputSink,
    ) -> PortResult<TerminalExecCommandResponse> {
        self.write_stdin(r).await
    }
    async fn send_stdin(&self, _: TerminalSendStdinRequest) -> PortResult<()> {
        Ok(())
    }
    async fn control_session(
        &self,
        _: TerminalExecControlRequest,
    ) -> PortResult<TerminalExecCommandResponse> {
        Ok(response("".into(), 0))
    }
}
fn context(root: &Path, port: Arc<dyn TerminalPort>) -> ToolUseContext {
    let mut context =
        ToolUseContext::for_tool_listing(Some(WorkspaceBinding::new(None, root.into())), None);
    context.runtime_handles = context.runtime_handles.with_terminal_port(Some(port));
    context
}
async fn pin_bash(tool: &ExecCommandTool, input: &Value, context: &ToolUseContext) {
    let path = std::env::var("BITFUN_SHELL_TEST_BASH").unwrap_or_else(|_| "/bin/bash".into());
    let shell = ResolvedLocalExecShell {
        display_name: "Fixture Bash".into(),
        path: PathBuf::from(&path),
        shell_type: ShellType::Bash,
    };
    assert_eq!(shell.shell_type, ShellType::Bash);
    tool.prepared_shells.lock().await.insert(
        ExecCommandTool::plan_key(input, context),
        PreparedShell::Local(shell),
    );
}
#[tokio::test]
async fn complete_shell_tool_executes_original_bytes_and_preserves_redirection_order() {
    let root = tempfile::tempdir().unwrap();
    let protected = root.path().join("repo");
    let scratch = root.path().join("scratch");
    std::fs::create_dir(&protected).unwrap();
    std::fs::create_dir(&scratch).unwrap();
    let terminal = Arc::new(FixtureTerminal::default());
    let context = context(&protected, terminal.clone());
    let mut tool = ExecCommandTool::new();
    tool.guard_fixture_state = Some(guard_state(&protected.to_string_lossy()));
    for (n, suffix, expected_capture, expected_file) in [
        (0, "> both 2>&1", "", "outerr"),
        (1, "2>&1 > both", "err", "out"),
    ] {
        let cmd = format!("(printf out; printf err >&2) {suffix}");
        let input = json!({"cmd":cmd,"workdir":scratch,"tty":n==1});
        pin_bash(&tool, &input, &context).await;
        assert!(tool.validate_input(&input, Some(&context)).await.result);
        let results = tool.call_impl(&input, &context).await.unwrap();
        let ToolResult::Result { data, .. } = &results[0] else {
            panic!()
        };
        assert_eq!(data["output"], expected_capture);
        assert_eq!(
            std::fs::read_to_string(scratch.join("both")).unwrap(),
            expected_file
        );
        let requests = terminal.requests.lock().unwrap();
        let r = requests.last().unwrap();
        assert_eq!(r.argv.last().unwrap(), &cmd);
        assert_eq!(r.cwd, scratch);
        assert_eq!(r.tty, n == 1);
    }
    let version = bitfun_services_core::process_manager::create_command(
        std::env::var("BITFUN_SHELL_TEST_BASH").unwrap_or_else(|_| "/bin/bash".into()),
    )
    .arg("--version")
    .output()
    .unwrap();
    eprintln!(
        "Integration shell: {}",
        String::from_utf8_lossy(&version.stdout)
            .lines()
            .next()
            .unwrap()
    );
}
#[tokio::test]
async fn complete_shell_tool_denies_before_spawn_and_saves_literal_answer() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let answer = root.path().join("answer");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(&answer).unwrap();
    std::fs::write(repo.join("source"), "original").unwrap();
    let terminal = Arc::new(FixtureTerminal::default());
    let context = context(&repo, terminal.clone());
    let mut tool = ExecCommandTool::new();
    tool.guard_fixture_state = Some(guard_state(&repo.to_string_lossy()));
    for tty in [true, false] {
        let input = json!({"cmd":"printf changed > source 2>&1","tty":tty});
        pin_bash(&tool, &input, &context).await;
        let v = tool.validate_input(&input, Some(&context)).await;
        assert!(!v.result);
        assert_eq!(v.meta.as_ref().unwrap()["executed"], false);
        assert!(v.blocks_input_rewrite());
        assert!(tool.call_impl(&input, &context).await.is_err());
        assert_eq!(terminal.calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            std::fs::read_to_string(repo.join("source")).unwrap(),
            "original"
        );
    }
    let body="中文 threshold > 100000\nrm /not-an-executed-target\n`touch /another-decoy`\nPath('/decoy').write_text('x')\n";
    let cmd = format!("cat > answer.txt <<'EOF'\n{body}EOF");
    let input = json!({"cmd":cmd,"workdir":answer});
    pin_bash(&tool, &input, &context).await;
    assert!(tool.validate_input(&input, Some(&context)).await.result);
    tool.call_impl(&input, &context).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(answer.join("answer.txt")).unwrap(),
        body
    );
    assert_eq!(terminal.calls.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read_dir(&answer).unwrap().count(), 1);
    assert_eq!(std::fs::read_dir(&repo).unwrap().count(), 1);
}
#[tokio::test]
async fn complete_shell_tool_rechecks_workdir_and_symlink_parent() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let scratch = root.path().join("scratch");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(&scratch).unwrap();
    let terminal = Arc::new(FixtureTerminal::default());
    let context = context(&repo, terminal.clone());
    let mut tool = ExecCommandTool::new();
    tool.guard_fixture_state = Some(guard_state(&repo.to_string_lossy()));
    let allowed = json!({"cmd":"printf x > same","workdir":scratch});
    let denied = json!({"cmd":"printf x > same","workdir":repo});
    pin_bash(&tool, &allowed, &context).await;
    pin_bash(&tool, &denied, &context).await;
    assert!(tool.validate_input(&allowed, Some(&context)).await.result);
    assert!(!tool.validate_input(&denied, Some(&context)).await.result);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&repo, scratch.join("link")).unwrap();
        let input = json!({"cmd":"printf x > link/missing/child","workdir":scratch});
        pin_bash(&tool, &input, &context).await;
        assert!(!tool.validate_input(&input, Some(&context)).await.result);
        std::os::unix::fs::symlink(repo.join("absent"), scratch.join("dangling")).unwrap();
        let input = json!({"cmd":"printf x > dangling","workdir":scratch});
        pin_bash(&tool, &input, &context).await;
        assert!(!tool.validate_input(&input, Some(&context)).await.result);
    }
    assert_eq!(terminal.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
#[ignore = "requires BITFUN_SHELL_TEST_BASH pointing to Bash >= 4"]
async fn complete_shell_bash_combined_append_on_capable_shell() {
    assert!(std::env::var("BITFUN_SHELL_TEST_BASH").is_ok());
    let root = tempfile::tempdir().unwrap();
    let terminal = Arc::new(FixtureTerminal::default());
    let context = context(root.path(), terminal.clone());
    let mut tool = ExecCommandTool::new();
    tool.guard_fixture_state = Some(guard_state(
        &root.path().join("protected").to_string_lossy(),
    ));
    let input = json!({"cmd":"(printf out; printf err >&2) &>> combined","workdir":root.path()});
    for _ in 0..2 {
        pin_bash(&tool, &input, &context).await;
        assert!(tool.validate_input(&input, Some(&context)).await.result);
        let results = tool.call_impl(&input, &context).await.unwrap();
        let ToolResult::Result { data, .. } = &results[0] else {
            panic!()
        };
        assert_eq!(data["exit_code"], 0);
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("combined")).unwrap(),
        "outerrouterr"
    );
}
#[tokio::test]
async fn complete_shell_shared_stdin_poll_cancel_and_data_compatibility() {
    let root = tempfile::tempdir().unwrap();
    let terminal = Arc::new(FixtureTerminal::default());
    let context = context(root.path(), terminal.clone());
    let tool = super::super::stdin::WriteStdinTool::new();
    for chars in ["", "\u{3}", "an unfinished 'quoted program input"] {
        let input = json!({"session_id":1,"chars":chars});
        assert!(tool.validate_input(&input, Some(&context)).await.result);
        tool.call_impl(&input, &context).await.unwrap();
    }
    assert_eq!(terminal.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn complete_shell_diagnostic_spans_and_redaction() {
    let root = tempfile::tempdir().unwrap();
    let terminal = Arc::new(FixtureTerminal::default());
    let context = context(root.path(), terminal);
    let mut tool = ExecCommandTool::new();
    tool.guard_fixture_state = Some(guard_state(&root.path().to_string_lossy()));
    let input = json!({"cmd":"printf '%s' '中文 SECRET_BODY' > 'secret\nfile'"});
    pin_bash(&tool, &input, &context).await;
    let v = tool.validate_input(&input, Some(&context)).await;
    let meta = v.meta.unwrap();
    let message = v.message.unwrap();
    assert!(!message.contains("SECRET_BODY"));
    assert!(!message.contains("secret\nfile"));
    assert!(message.contains("secret\\nfile"));
    let start = meta["source_span"]["start"].as_u64().unwrap() as usize;
    let end = meta["source_span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(
        &input["cmd"].as_str().unwrap()[start..end],
        "'secret\nfile'"
    );
    assert_eq!(meta["source_span"]["unit"], "utf8_bytes");
}
#[tokio::test]
async fn complete_shell_sh_zsh_common_syntax_executes_without_rewrite() {
    for (path, kind) in [("/bin/sh", ShellType::Sh), ("/bin/zsh", ShellType::Zsh)] {
        if !Path::new(path).exists() {
            continue;
        }
        let root = tempfile::tempdir().unwrap();
        let terminal = Arc::new(FixtureTerminal::default());
        let context = context(root.path(), terminal.clone());
        let mut tool = ExecCommandTool::new();
        tool.guard_fixture_state = Some(guard_state(
            &root.path().join("protected").to_string_lossy(),
        ));
        let cmd = "printf x 2>&1 > out; cat < out";
        let input = json!({"cmd":cmd});
        tool.prepared_shells.lock().await.insert(
            ExecCommandTool::plan_key(&input, &context),
            PreparedShell::Local(ResolvedLocalExecShell {
                display_name: "fixture".into(),
                path: path.into(),
                shell_type: kind,
            }),
        );
        assert!(tool.validate_input(&input, Some(&context)).await.result);
        let results = tool.call_impl(&input, &context).await.unwrap();
        let ToolResult::Result { data, .. } = &results[0] else {
            panic!()
        };
        assert_eq!(data["output"], "x");
        assert_eq!(data["exit_code"], 0);
        assert_eq!(
            terminal.requests.lock().unwrap()[0].argv.last().unwrap(),
            cmd
        );
    }
}
