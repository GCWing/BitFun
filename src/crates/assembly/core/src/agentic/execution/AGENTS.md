If you modify `stream_processor.rs`, run the stream integration tests before finishing.

For complete shell constraint checks, use:

```bash
cargo test -p bitfun-core --no-default-features --features agent-runtime,git --lib complete_shell
cargo test -p bitfun-core --no-default-features --features agent-runtime,git --lib edit_constraint_guard
cargo test -p bitfun-core --no-default-features --features agent-runtime,git --lib exec_command::
cargo test -p bitfun-core --no-default-features --features agent-runtime,git --lib hook_rewrite
```

The ignored `complete_shell_archive_replay` and `complete_shell_normal_sample_replay`
tests take absolute JSONL input/report paths in `BITFUN_SHELL_REPLAY_INPUT` /
`BITFUN_SHELL_REPLAY_OUTPUT` and `BITFUN_SHELL_NORMAL_INPUT` /
`BITFUN_SHELL_NORMAL_OUTPUT`. They only analyze strings; never execute archived
commands. The ignored Bash append integration test requires `BITFUN_SHELL_TEST_BASH`
to name a trusted Bash 4+ executable and uses only isolated synthetic commands.
