If you modify `stream_processor.rs`, run the stream integration tests before finishing.

For per-round Router input preparation, isolation from main context compression,
background summaries, and routing protocol behavior, use:

```bash
cargo test --locked -p openbitfun-core --no-default-features --features agent-runtime,git --lib round_model_routing
```

Router context state/budgets live in `openbitfun-agent-runtime::router_context`;
Core only observes messages, invokes the auxiliary fast model and stores its
separate sidecar. Never feed Router summaries back into the main transcript or
use the main compactor's summary/model/config as Router state.

For model retry admission and recovery, use:

```bash
cargo test --locked -p openbitfun-core --no-default-features --features agent-runtime,git --lib agentic::execution::round_executor::tests
```

For complete shell constraint checks, use:

```bash
cargo test -p openbitfun-core --no-default-features --features agent-runtime,git --lib complete_shell
cargo test -p openbitfun-core --no-default-features --features agent-runtime,git --lib edit_constraint_guard
cargo test -p openbitfun-core --no-default-features --features agent-runtime,git --lib exec_command::
cargo test -p openbitfun-core --no-default-features --features agent-runtime,git --lib hook_rewrite
```

The ignored `complete_shell_archive_replay` and `complete_shell_normal_sample_replay`
tests take absolute JSONL input/report paths in `OPENBITFUN_SHELL_REPLAY_INPUT` /
`OPENBITFUN_SHELL_REPLAY_OUTPUT` and `OPENBITFUN_SHELL_NORMAL_INPUT` /
`OPENBITFUN_SHELL_NORMAL_OUTPUT`. They only analyze strings; never execute archived
commands. The ignored Bash append integration test requires `OPENBITFUN_SHELL_TEST_BASH`
to name a trusted Bash 4+ executable and uses only isolated synthetic commands.
