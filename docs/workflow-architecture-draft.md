# Workflow Architecture: A Multi-Agent Orchestration Framework

> Draft for the PR customization description.
> This document describes the **generic workflow architecture** used to
> organize multi-agent development work: a phase pipeline, a separation of
> powers, a recursive dispatch pattern, and a set of quality gates.
> It is written in neutral, formal language and contains no internal names,
> assets, or trade secrets — only the architecture itself.

---

## Table of Contents

1. Overview
2. Core Design Philosophy
3. Workflow Skeleton: Phase 0–5
4. Separation of Powers: Decision / Execution / Review
5. Recursive Fractal Pattern: Dispatch → Execute → Accept
6. Serial vs Parallel Discipline
7. Quality Gate System
8. Atomic Step Specification (Five Elements)
9. Deterministic Execution: "One-Shot" Principle
10. Reporting Chain vs Collaboration Network
11. Decision Derivation: Three Sources
12. Summary

---

## 1. Overview

Multi-agent software projects face a recurring problem: when many agents work
on one codebase, **preparation**, **execution**, and **verification** blur
together, defects leak through, and rework multiplies.

This framework answers with four structural ideas:

1. **A strict phase pipeline** (Phase 0–5) that every task must pass through.
2. **A separation of powers** among decision, execution, and review roles.
3. **A recursive dispatch pattern** ("Dispatch → Execute → Accept") applied
   at every level of decomposition.
4. **Deterministic execution**: the preparation phase is unbounded, the
   execution phase is "one-shot".

The framework treats a workflow not as a checklist game but as an
**assembly line**: the output of each phase is the input of the next, and no
phase may be skipped.

---

## 2. Core Design Philosophy

Three principles underlie all rules:

| Principle | Meaning |
|---|---|
| **Facts** | Correct direction. All decisions rest on verifiable evidence (source lines, command output). Any claim is untrusted until it carries evidence; whoever asserts must provide proof. |
| **Efficiency** | No wasted time. Efficiency = useful time / total time = 1 − waste/total. Waste = rework (change-as-you-go), blind trial-and-error, silent detours (overrunning without self-check), idle waiting, and scope creep. |
| **Result** | Deliverable achieved. Delivery accepts only the complete result, ready to use as-is. |

The three form a loop: facts keep the direction right, efficiency keeps time
from being wasted, and results guarantee arrival. Verification of the result
reversely exposes gaps in facts.

Two derived ideas:

- **99% preparation, 1% execution.** Reconnaissance, planning, and auditing
  are where time goes; writing code is the smallest step.
- **Unlimited preparation.** If anything is uncertain, go back to
  preparation. Do not push forward on uncertainty.

---

## 3. Workflow Skeleton: Phase 0–5

Every task flows through six phases in order.

| Phase | Name | Input | Output |
|---|---|---|---|
| 0 | Requirements | raw user request | clarified requirements document |
| 1 | Reconnaissance | requirements document | reconnaissance report |
| 2 | Planning & Decomposition | reconnaissance report | plan documents (plan, type contract, dispatch prompts) |
| 3 | Dispatch & Execution | plan documents | code |
| 4 | Quality Gate | code | pass / reject |
| 5 | Delivery | accepted code | delivered result |

Rules:

- **No skipping.** Each phase's output is the next phase's input.
- **Reconnaissance before code.** Never jump from requirements to code.
- **Rework discipline.** If execution reveals that earlier preparation was
  insufficient (missed recon, misread requirements, wrong architecture),
  do not patch in place — **return to Phase 0**, analyze the root cause, and
  re-run the full pipeline.
- **Two kinds of rework, different natures.** An *active rollback*
  (self-detected, or caught by a quality gate) is encouraged learning
  behavior. A *delivery rejection* (defect found after delivery to the user)
  means the preparation phase itself was at fault.

---

## 4. Separation of Powers: Decision / Execution / Review

Each task is handled by three peer roles with distinct powers. They are
equal in rank; none reports to another; each holds one power and does not
encroach on the others.

| Power | Role | Responsibility |
|---|---|---|
| **Decision** | Coordinator | Requirements intake, planning & decomposition, dispatch & tracking, direction rulings |
| **Execution** | Executor | Receives atomic steps, executes them exactly, returns results |
| **Review** | Reviewer | Quality gates: review, test, acceptance; pass / reject with evidence |

Checks and balances:

- **Decision** does not perform execution (no editing code, no running
  builds directly).
- **Execution** does not make decisions (the only exit at a decision point
  is to report back to the coordinator).
- **Review** is independent of execution (no self-review).
- At serial nodes, all three gates must pass before the next wave begins.

The three roles are themselves "coordinators" in miniature: each can
decompose its own third of the responsibility and dispatch to a further
sub-team (a recursive structure, see §5).

---

## 5. Recursive Fractal Pattern: Dispatch → Execute → Accept

The entire organization uses **one pattern, recursively**:

```
Dispatcher ──→ Executor ──→ Acceptor
```

| Level | Dispatch | Execute | Accept |
|---|---|---|---|
| Top coordination | coordinator | team leads | chief reviewer |
| Within a team | sub-coordinator | A/B/C cell (prompt / execute / review) | team reviewer |
| Within an agent | own judgment | sub-agent | self-check |
| Multiple teams | coordinator orchestration | team relay | team alignment → reviewer |

Key rules:

1. **One universal pattern.** Team splitting is just the recursive
   application of this pattern (nested fractal, not a hierarchy).
2. **Context isolation.** Each role loads only the context needed for its
   own duty (~30%), so contexts do not pollute each other: the prompt role
   only "finds", the executor only "does", the reviewer only "reviews".
3. **Rejection loop.** If review finds a bug, it is returned to the
   executor for fixing; at most 3 rounds. Beyond 3 rounds, escalate to the
   coordinator.
4. **Flexible cell sizing.** The minimum cell is A (executor) + C
   (reviewer). More roles are added as complexity demands; multiple cells
   may run in parallel with a single reviewer accepting the combined result.

---

## 6. Serial vs Parallel Discipline

Parallelism and serialization are decided by **dependency**, not by habit:

- **No dependency → parallel.** Reconnaissance, independent modules, and
  draft planning run fully in parallel to eliminate waiting waste.
- **Dependency → serial.** Reconnaissance → planning → execution →
  verification → delivery; each step is the premise of the next, and
  skipping means rework.

At a serial node, the work must pass **all three quality gates** before
advancing to the next wave. Parallel branches must finally be **converged
serially**: one designated executor runs the full gate suite and makes the
commit. During parallel execution each track runs only its own scoped tests;
the full regression suite runs only at the convergence node (intermediate
compile failures in parallel tracks are normal, not flaky).

Two clarifying notes:

- Efficiency is *maximized time utilization*, not *rushing*. The only
  question for two steps is: "Is this step the premise of the next?" If yes,
  serialize for quality; if no, parallelize for efficiency.
- Unlimited preparation is not unlimited procrastination: preparation is
  always done fully, and time pressure never justifies shrinking it.

---

## 7. Quality Gate System

At serial nodes, three gates run **in parallel**, all three mandatory:

| Gate | Role | Criterion |
|---|---|---|
| Review | Reviewer | Logic, architecture, and compliance, full-chain; evidence with file:line |
| Test | Executor | Compile clean, tests green, linter zero warnings |
| Acceptance | Reviewer/Acceptor | Feature-by-feature comparison against the original requirements; check for empty stubs |

Discipline:

- Any gate failing → return for fixing → **all three gates re-run**.
- **Gate blind-spot law:** a green gate does not mean the delivery is
  wireable — gates must cover the real wiring path (integration smoke).
- **Review conclusions are reviewed too:** language/framework semantics
  must be verified empirically.
- Repeated failure → the root cause is in preparation → return to Phase 0.
  Multiple bugs are converged to a shared root cause and fixed once.
- **Root-cause repair rule:** "fixed" means the root cause is eliminated
  (reproduction no longer occurs + all paths sealed), not that the symptom
  is temporarily absent. If a bug has been "fixed" N times without healing,
  the previous N−1 fixes were all symptom-level — return to Phase 0 and
  re-investigate the root cause from scratch, never patch on top of the old
  hypothesis.

---

## 8. Atomic Step Specification (Five Elements)

Every dispatched step must carry five elements; a step missing any one is
not ready to dispatch:

1. **Input location** — exactly where the inputs are (file path, resource).
2. **Action instruction** — exactly what to do, in imperative terms.
3. **Expected output** — what the step should produce.
4. **Acceptance assertion** — how to verify the output is correct.
5. **Failure fallback** — what to do if the step deviates (which phase to
   return to, which assertion to fix).

The standard for "preparation is complete": **any person** executing the
step — even one with no background — must produce the same result ten
thousand times. The executor does not judge; the instruction leaves no room
for judgment. Executors will be lazy, misjudge, and misunderstand — that is a
certainty, not an accident. The instruction must be designed so that being
wrong is hard.

---

## 9. Deterministic Execution: "One-Shot" Principle

Two complementary ideas:

- **The Tsien ballistic curve** (both ends fixed, middle free): the
  requirement (start) is fixed, the delivery (end) is fixed. How to get there
  in the middle is the coordinator's business. Deviation is allowed in the
  middle, but the deviation-handling path (which phase to roll back to,
  which assertion to fix) is predefined during preparation, and the
  acceptance assertions at the end are written before implementation begins.
  Deviation is only a path fluctuation; it never changes the delivery.
- **One-shot execution**: preparation rounds are unbounded; execution has
  only one chance. No changing-as-you-go.

Pressure transfer: the coordinator converts uncertainty into instruction
determinism during preparation. What flows downward is not pressure but
determinism. When execution deviates, the first question is always "where is
the instruction not deterministic enough", never "where is the executor
disobedient".

---

## 10. Reporting Chain vs Collaboration Network

These two communication structures are **orthogonal**:

- **Reporting chain** (vertical, strict, single-line): dispatch and status
  reports flow through a single line without skipping levels.
- **Collaboration network** (horizontal, free): peers within a cell and
  within a team may communicate directly without going through their
  superior.

Rank indicates reporting order only, not collaboration restrictions.

---

## 11. Decision Derivation: Three Sources

During execution, all decisions (architecture rulings, plan selection, gap
handling, priority ordering) are derived from three sources, in order:

1. **Requirements** — the original requirement ID / authoritative source
   clauses.
2. **Purpose** — the final delivery definition (a complete, ready-to-use
   deliverable).
3. **Iron rules** — the framework's own rules and quality criteria.

The user participates only at the requirements stage and the delivery-result
stage; in between, decisions are made autonomously by this derivation. Only
three situations are escalated to the user:

- The requirements themselves are contradictory or missing.
- The delivery definition has been rejected by the user.
- The iron rules conflict and cannot be resolved.

One additional boundary: autonomous decisions must **not expand into
resource commitments the user never asked for** (downloading model files,
introducing new dependencies, building unrequested alternative solutions).
If the user has already set a direction, follow it; if not, ask before
investing.

---

## 12. Summary

This framework organizes multi-agent development around four structural
pillars:

1. **A six-phase pipeline** (requirements → recon → planning → execution →
   quality gate → delivery) that every task passes through without skipping.
2. **A separation of powers** among decision, execution, and review — three
   peer roles that check and balance each other.
3. **A recursive fractal pattern** (Dispatch → Execute → Accept) applied at
   every level, with context isolation and a bounded rejection loop.
4. **Deterministic execution** — unlimited preparation, one-shot execution,
   five-element atomic steps, both-ends-fixed trajectory, and three-source
   decision derivation.

The design philosophy — facts, efficiency, results — is the source of every
rule: facts keep the direction right, efficiency keeps time from being
wasted, and results guarantee arrival.

---

*This document is a draft for the PR description. It describes only the
generic workflow architecture and intentionally contains no internal names,
local assets, or proprietary information.*
