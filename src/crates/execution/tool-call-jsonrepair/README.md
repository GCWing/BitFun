# Tool-call JSON repair

This crate is a local fork of
[`jsonrepair-rs` 0.2.1](https://github.com/majiayu000/jsonrepair-rs), licensed
under MIT. The upstream license is retained in [`LICENSE`](LICENSE).

It retains the upstream generic repair API for compatibility, and adds
`repair_tool_call_json` for BitFun streamed tool arguments. That profile does
not interpret `#`, `//`, or `/* ... */` as comments: tool arguments are JSON,
not configuration files. This prevents Markdown content whose opening quote
was omitted from being silently discarded as a comment.

The profile still supports bounded syntax recovery needed for malformed model
tool arguments, including missing string quotes, commas, and closing
delimiters. The caller must parse and schema-validate the result before use.

## Upstream regression coverage

The non-CLI regression tests and parity fixture from `jsonrepair-rs` 0.2.1 are
vendored under `tests/`. They differ only in the local crate import path. The
upstream CLI tests are intentionally excluded because this internal library
sets `autobins = false` and does not ship the upstream command-line program.
