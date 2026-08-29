---
name: multitask
description: Coordinate multi-part work through parallel subagents when two or more workstreams can proceed independently. Use for decomposable tasks where concurrency materially reduces the critical path; skip for trivial or tightly sequential work.
---

# Multitask

Work parallel-first when the task has genuinely independent branches.

## Workflow

1. Identify the dependency graph before implementation. Separate shared contract decisions and blocking prerequisites from branches that can proceed independently.
2. If at least two useful branches are independent, keep coordination and the critical path local, then delegate one or more other branches to background subagents. If the work is sequential or delegation overhead outweighs the benefit, continue locally.
3. Give each subagent a non-overlapping scope, expected output, ownership boundary, and focused verification responsibility. State what must remain unverified when a branch depends on in-flight shared state.
4. Continue independent local work while delegated branches run. Wait for results only when the critical path needs them.
5. Review the returned work at its interfaces, resolve mismatches, integrate the branches, and run final verification against the combined result.

## Rules

- Use the available Task delegation interface with background execution for real parallel work, and use the corresponding wait interface when results are needed. Do not assume a particular subagent exists; choose only from the available catalog.
- Keep shared contracts, dependency decisions, overlapping edits, destructive actions, final integration, and final verification under the main Agent's control.
- Batched or concurrent-looking file edits by the main Agent are not independent execution. Do not claim parallelism unless separate subagents actually ran concurrently.
- Respect the user's authorization boundaries and repository instructions in every delegated scope. This skill does not authorize extra writes, external actions, worktrees, servers, or UI interaction.
- Do not create subagent work merely to satisfy the workflow. Each delegated branch must have a concrete deliverable that can progress without blocking another branch.
