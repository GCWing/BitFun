# Complete shell command analysis

This module extracts syntax facts for task file constraints. It performs no IO,
reads no environment variables or scripts, launches no process, and returns no
permission decision. An empty operation list is not proof that a program is
read-only. Core applies session constraints and filesystem/path policy separately.

## Supported subset

The lexer retains quoting, escapes, expansion attributes, redirection kinds,
source spans and a queued here-doc body for each delimiter. The parser recognizes
simple commands, sequences, `&&`, `||`, pipelines and subshells. Static Bash FD
copy/close/move is separate from file open/append/read-write operations. Compact
redirects and Bash `>& filename`, `&>` and `&>>` are file operations. For `sh`,
Bash-only redirects are unsupported; zsh support is limited to this common subset.
The executable's own version still determines whether a syntax is valid at run
time (for example, Bash 3.2 does not implement `&>>`). No command is rewritten.

Quoted here-docs consumed by ordinary commands remain data. Bash/sh/zsh `-c`,
here-doc and here-string programs are recursively analyzed. Command/process/
backtick substitutions are conservatively unresolved, including substitutions
outside a literal answer body. Python/Node recognizers are retained from the
legacy guard but run only on extracted interpreter programs. Their literal
operations are useful facts, not completeness evidence; a recognized write also
retains an unresolved effect so another dynamic write cannot be washed out.
External scripts, unknown wrapper options, eval, xargs, patch and implicit Git
worktree changes remain unresolved. Ordinary build/test programs retain the
existing execution policy; their runtime side effects are outside this analyzer.

Absolute `cd` without parent traversal propagates success and failure directory
sets. Semicolon and `||` paths retain possible old directories. Subshell and
pipeline directories never escape into the parent. Relative/logical `cd`,
CDPATH/OLDPWD-sensitive changes, functions, loops and branch syntax are either
uncertain or unsupported. POSIX analysis requires a POSIX cwd; Windows shell path
translation is not inferred from a client-side drive path.

Spans are half-open UTF-8 byte ranges in the original command. Nested decoded
program facts refer to their containing argument/body span because quote removal
and `<<-` tab stripping can change offsets. Diagnostics contain bounded reasons
and escaped paths, never entire commands, here-doc bodies or environment values.

## Parser choice and bounds

A purpose-built lexer and recursive-descent parser keep the supported language
small and explicit. Tree-sitter Bash 0.25.1 was evaluated but not adopted: its
error recovery on multiple here-docs could expose literal body data as commands.
`shlex`/`shell-words` do not preserve the required syntax contexts. No new external
dependency is needed; the owner-local `shell-analysis` feature gates the parser,
while the interpreter recognizers reuse the baseline filename matcher's regex dependency.

Limits: 1 MiB source, 32 group/program/control nesting levels, 32,768 tokens,
4,096 file or descriptor facts, 8 possible directories, and 64 diagnostic facts.
Core additionally bounds path inspection to 32 KiB and 256 components per target.
Exceeded limits are explicit failures, never empty successful results. There is
no analysis or permission cache. Core retains bounded shell execution selections
between preflight and dispatch and rechecks constraints immediately before spawn.

## Integration and compatibility

Only complete ExecCommand uses this module. WriteStdin, Git's existing facade,
and direct-file tools retain their existing guards. No task data schema changes,
new force option, protection disable switch, or constraint relaxation is added.
Shell errors retain `failure_kind=edit_constraint_guard`, outer validation errors,
and `blocks_input_rewrite`, and add a bounded JSON details record to both metadata
and the existing model observation string. Thus old consumers still read the
message while the model receives the classification and `executed=false`.

Local path checks inspect the nearest existing ancestor, follow symlinks, and
then reuse tool path resolution. Remote checks use only the workspace filesystem
port. Until that port can resolve links authoritatively, symlinks and unavailable
metadata are unresolved; no local mirror supplies evidence. Existing remote cwd
containment remains unchanged. This does not remove symlink races, startup-file
side effects, alias/function overrides, cross-process FD state or the need for
runtime permissions, sandboxing and mount restrictions.

## Verification

Run the focused commands in the owner and Core guides. The 85-case fixture is
copied unchanged in semantic content; it is parser data, never a shell script.
Actual execution fixtures construct isolated temporary directories. The ignored
archive and normal-sample replay tests require explicit input/output paths and
call analyzers only. Do not execute archive commands or read their script inputs.
Tests inject an in-memory constraint snapshot only at the session-state lookup
boundary; parser, path checks, tool dispatch and Hook pipeline remain real code.

Rollback the complete analyzer feature and ExecCommand adapter changes together.
There is no data migration. Do not deploy a partial integration or disable the
existing guard as a fallback; the legacy parser's known defects would return.
