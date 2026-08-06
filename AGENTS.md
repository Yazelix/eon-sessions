# Agent Guidelines

This file is self-contained. Canonical protocol text was rendered into it;
the source repository is needed only to update or verify the import.
Do not edit this generated file directly. Edit `.agent-protocols.local.md`
or `.agent-protocols.exceptions.json`, then render from the pinned source.

## Protocol import record

- Source: `https://github.com/luccahuguet/starcompass`
- Source commit: `f41e799db9d78277f18409985287e96a6fc80f51`
- Profiles: `greenfield`, `terminal-engine`
- Manifest: `.agent-protocols.json` (schema 1)

| Protocol | Version | SHA-256 |
| --- | ---: | --- |
| `AP-SCOPE-001` | 1 | `b3f7e012df0708d4baf8957e3c315878a9eb8cd7fddf637dfde1506609d08444` |
| `AP-CONTRACT-001` | 1 | `0aa692f4c52542111149b691b7d0c015a0c16523cd620a67b0cf335f3284da82` |
| `AP-REFERENCE-001` | 2 | `ecb28af3796a9964dd98c037463a33b7444d9c39a0365783fb0e9ae9fc007b9c` |
| `AP-MINIMAL-001` | 1 | `256c158cc8b226e4baf96d5590531ea180edc9717338dac0fe51a34dd037791f` |
| `AP-DEPENDENCY-001` | 1 | `a389ff9054708574c52ec5e5dd7fc3e2d13b125d2218c70062f50a86761981ca` |
| `AP-OWNERSHIP-001` | 1 | `bdc09117f79b0d8dbe78e2dd8673a2463398aa31880fe27209fd6cc47f58bbf7` |
| `AP-TEST-001` | 1 | `363da7c542521be22233a4cc3373c0d3c3c5a9a0cf37f633cd5029545f4a3bee` |
| `AP-PROOF-001` | 1 | `1af235a56e9711d55362d869fa4057f1658d0aa7fe766030be0897ee5fd7c02b` |
| `AP-PLAN-001` | 4 | `5bc0426788693717e8af011277d9358c65899e839d7c0c71299003ec0d8acc4b` |
| `AP-CI-001` | 1 | `78f1662259cd83f33d22ff4ddd0859ab0d4f704ba4f38756eef40a8b9b787bec` |
| `AP-EXCEPTION-001` | 1 | `f66749229dbbc005e1c3103bfed86cf95169d7b32466c72fd8442e841cadb268` |
| `AP-GIT-001` | 3 | `16d27b0df7ccc94880bb31020e822e32b37503f43c2cf7a69de333300cbfdacf` |
| `AP-PORTABILITY-001` | 1 | `d2800376013bbe1ade3449f835daf8780e60d4c08a79a7f733d4a651c4d6d887` |
| `AP-TERMINAL-001` | 1 | `a8632561b2ff959b1e0ee2abc07f4bce13af289e0a23574cc51aef740e3d9605` |
| `AP-FAILURE-001` | 1 | `38df47dd773c7a85161309a1947dd97afcce93a5bdefc82f1c971475d228482e` |

### Local exceptions

No local exceptions.

## Canonical protocols

### AP-SCOPE-001 — User-owned scope

The user decides product and project scope. An agent may inspect, explain, test,
or make the smallest implementation needed for the chosen goal, but it must not
silently create a feature, compatibility promise, public surface, migration,
repository, or planning item outside that direction.

Required practice:

- Separate safe implementation details from choices that change product scope.
- State consequential assumptions; stop when a missing choice would materially
  change the result.
- Treat a terminal instruction such as “finish” as persistence, not broader
  authority.
- Keep useful out-of-scope observations as findings unless the user has chosen
  a durable planning destination for them.

### AP-CONTRACT-001 — Contract-driven changes

State the irreducible externally observable behavior before choosing the code
shape. Give durable contracts stable identifiers when later code, tests, or
repositories need to cite them.

Required practice:

- Name the consumer, trigger, observable result, and important failure behavior.
- Identify the current sources of truth and decide which one owner survives.
- Choose the cheapest check that can falsify the contract.
- Implement the smallest vertical slice that satisfies it.
- Update the contract first when an intentional behavior change is chosen.

Do not turn implementation details into contracts unless another component must
rely on them.

### AP-REFERENCE-001 — Evidence before code shape

Review the relevant sources before deciding architecture or implementation
shape. Memory, summaries, and reputation are discovery aids, not sufficient
evidence for a consequential decision.

Required practice:

- Read the affected local code, contracts, tests, and repository instructions.
- Inspect designated external references at the subsystem named by local rules.
- Record the concrete mechanism adopted, rejected, or left unresolved.
- Distinguish direct source evidence from inference.
- Revisit the evidence when the proposed shape changes materially.
- Apply source-license wording to the actors, uses, and conditions it actually
  names. Do not infer that an independent user or project acts on behalf of,
  for the benefit of, or under the direction of an agent or tool provider
  merely because the user selected that provider's service. Examples
  introduced by words such as “including” remain scoped by the condition they
  illustrate.
- Distinguish inspecting public source for ideas from copying, adapting,
  redistributing, selecting a dependency, or incorporating the source. A
  restriction on one of those actions does not silently erase required source
  inspection when the requested research itself remains permitted.
- If license interpretation would exclude required evidence, identify the
  exact clause, actor, beneficiary, direction, and requested use. Resolve a
  material ambiguity with the user instead of broadening the restriction by
  association or substituting reputation and secondary summaries for source.

Reference review is a decision gate, not a requirement to copy the reference.

### AP-MINIMAL-001 — Minimum sufficient implementation

Understand the affected flow before choosing the smallest complete solution.
Use the first option that fully satisfies the chosen contract:

1. Make no change when the required behavior already exists.
2. Reuse an existing owner, helper, or pattern in the repository.
3. Use the standard library or a native platform capability.
4. Use an already accepted dependency that owns the behavior.
5. Implement the minimum local code that is correct and maintainable.

Prefer deletion over addition, direct ownership over adapters, and fewer files
over scaffolding. Minimalism must not remove required behavior, trust-boundary
validation, data-loss protection, security, accessibility, or the cheapest
runnable check for non-trivial logic.

Ponytail is the adopted agent-side implementation of this discipline when the
host supports it. Use the upstream project directly rather than copying its
rules or adapters. The reviewed source is
[DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail/tree/16f29800fd2681bdf24f3eb4ccffe38be3baec6b).
If Ponytail is unavailable or disabled, the self-contained requirements above
still apply. Its instruction hooks improve consistency; they do not prove
compliance.

### AP-DEPENDENCY-001 — Dependency gate

Choose dependencies for architectural fit and net system simplicity, not name
recognition or short-term convenience.

Before adding a crate, package, framework, service, or embedded project:

- State the capability and contract it would own.
- Consider the standard library, owned code, and multiple credible candidates.
- Compare maintenance, platform fit, correctness, transitive weight, licensing,
  API stability, and the lines and complexity removed.
- Record the chosen candidate, meaningful rejections, and replacement cost.
- Pin deliberately and add the smallest check that proves the relied-on behavior.

Remove a dependency when it no longer owns enough behavior to justify its cost.

### AP-OWNERSHIP-001 — One owner per invariant

Every invariant, state transition, and user-visible policy must have one clear
owner. Other components may consume its output; they must not independently
reconstruct or reinterpret the same truth.

Required practice:

- Name the owner before adding adapters or synchronization.
- Prefer deleting duplicate owners over reconciling them.
- Keep policy at the highest layer that has the necessary context and mechanism
  at the lowest layer that can enforce it correctly.
- Make cross-boundary data explicit and versioned when independently released
  components depend on it.

### AP-TEST-001 — Strong and few tests

Tests exist to protect contracts, regressions, boundaries, and failure modes
that matter to users or future agents. Prefer one strong test with meaningful
setup and assertions over several thin tests.

Required practice:

- Use TDD for deterministic helpers, parsers, protocol behavior, and regressions
  when the expected behavior can be stated before implementation.
- Choose contract-first integration checks for layout, runtime integration,
  architecture choices, forks, and dogfooding surfaces.
- Delete or merge tests that duplicate another proof, assert implementation
  trivia, or preserve scaffolding.
- Test observable effects rather than mirroring literals, defaults, or source
  structure.
- Add absence guards only when absence is itself a security, licensing, size,
  ownership, or known-regression contract.

### AP-PROOF-001 — Explicit proof lifecycle

Claims and proofs have a lifecycle. A passing check supports only the exact
revision, environment, and surface it exercised.

Required practice:

- Record the command or observation, relevant environment, revision, and result.
- Distinguish proposed, implemented, mechanically verified, manually dogfooded,
  accepted, and promoted states.
- Re-run stale proof after relevant code, dependency, platform, or contract
  changes.
- Never promote a narrower check into a broader claim.
- Preserve important negative results; they constrain the next valid design.

### AP-PLAN-001 — Durable planning state

Keep the outcomes and constraints that later work needs in the project's
durable planning system or canonical documentation. An issue represents a
chosen goal, decision, material defect, or schedulable follow-up. Review and
implementation methods belong to that issue.

Required practice:

- After review, fresh-eyes, simplification, or verification, update the owning
  issue's editable fields to describe the accepted state instead of pass
  chronology.
- Create a separate issue only for a material finding outside the owning scope
  or one worth scheduling on its own. Name it after the outcome or finding.
- Record the contract, decision boundary, dependencies, acceptance evidence,
  material negative results, and rejected alternatives that constrain later
  work.
- Reserve append-only comments and audit records for chronology needed as
  evidence. Keep raw command logs and build transcripts with their proof. Omit
  baseline hashes, failed attempts, and candidate scoring unless they constrain
  later work.
- Keep issue status honest: planned, active, blocked, and complete are distinct.
- Model real prerequisites as dependencies; do not create decorative graphs.
- Reconcile planning state with the repository before handoff.
- Use the repository-designated issue tool and never edit its storage directly.

Do not erase approvals, contract changes, material failures, or evidence needed
to understand the accepted result.

### AP-CI-001 — Bounded continuous integration

Hosted automation must buy enough confidence to justify its financial,
latency, security, and maintenance cost.

Before enabling CI:

- Name the protected contract and why local verification is insufficient.
- Bound triggers, job count, timeouts, permissions, artifacts, cache growth, and
  concurrency.
- Prefer one cheap deterministic job before matrices or scheduled runs.
- Make fork and secret behavior explicit.
- Record the evidence required to expand, reduce, or remove the workflow.

Private-repository minutes and cache storage are product constraints, not an
invisible externality.

### AP-EXCEPTION-001 — Explicit local exceptions

A local rule may narrow, replace, or suspend an imported protocol only through
an explicit exception approved by the user or named project authority.

Each exception records:

- the protocol ID;
- its exact scope;
- the reason the canonical rule does not fit;
- who approved it and when;
- an expiry or review condition when the exception is temporary.

Unrecorded conflicts are drift. A local rule that merely adds detail without
changing the canonical requirement is an overlay, not an exception.

### AP-GIT-001 — Safe repository history

Repository history is shared user state. Preserve unrelated work, follow the
local branch and promotion model, and use the least destructive operation that
achieves the requested result.

Required practice:

- Inspect status and repository instructions before editing.
- Treat existing and concurrent changes as user-owned unless proven otherwise.
- Fold a correction into the current task's unpublished commit when it belongs
  to the same unit of work. Refresh and reverify dependent local commits and
  generated artifacts.
- Use a follow-up commit after a push, promotion, release, external pin, or any
  other point where someone outside the current local work can rely on the
  revision.
- Do not reset, discard, force-push, rewrite published history, or create a
  branch without authority from the user or repository policy.
- Verify the intended diff before committing and the remote state after pushing.
- Make rollbacks additive through a reviewed revert unless policy says otherwise.

### AP-PORTABILITY-001 — Portable core with explicit platform seams

Do not let one operating system's APIs, process model, paths, packaging, or
event facilities become an accidental foundation when supported targets are
broader.

Required practice:

- Keep platform-neutral contracts and state in the core.
- Isolate OS-specific code behind the smallest meaningful seam.
- Evaluate Linux and macOS implications before adopting foundational runtime,
  process, graphics, filesystem, or transport dependencies.
- Prove platform behavior on the platform; compilation alone is narrower
  evidence.
- Record intentionally unsupported platforms rather than implying portability.

### AP-TERMINAL-001 — Terminal state authority

One component must be authoritative for terminal semantic state. Renderers,
transports, and clients consume versioned projections; they must not invent a
second emulator state machine.

Required practice:

- Define ownership of parsing, grid state, modes, scrollback, selection,
  graphics, title, and process attachment.
- Specify whether clients receive raw bytes, checkpoints plus tails, structured
  frames, or another explicit replication contract.
- Treat reconnect, resize, alternate-screen state, partial parser input, and
  slow consumers as contract cases.
- Test semantic equivalence at attachment boundaries, not only visual similarity.

### AP-FAILURE-001 — Designed lifecycle and failure semantics

Long-lived sessions and services must define lifecycle, recovery, and pressure
behavior before happy-path feature breadth.

Required practice:

- Name state transitions for start, attach, detach, crash, restart, shutdown,
  version mismatch, and abandoned resources.
- Bound queues, memory, retries, timeouts, and slow-consumer behavior.
- Make cancellation ownership and cleanup idempotent.
- Preserve enough durable state to meet the recovery contract and no more.
- Test failures deterministically where possible and dogfood the remaining
  process and platform interactions.

## Repository-local rules

### Orbit repository rules

Orbit is a clean-room Rust experiment for the durable local terminal-session
runtime beneath Venus, the greenfield graphical client for Yazelix Astra.
Astra is a separate greenfield Yazelix line. Yazelix Nova remains an
independently valuable product on its current Mars and Zellij architecture
while the experiment runs.

Use `Superlogical terminal multiplexer` for Superlogical's published first
product. Do not invent a shorter product name or research codename.

## Core rule

The user decides scope. Do not add a feature, compatibility surface, module,
dependency, or planning bead until the user has chosen that direction.

## Irreducible contract

Orbit owns a real PTY process and the sole authoritative libghostty terminal
state. The process survives graphical-client exit or disconnection while Orbit
remains running. Exactly one local client may attach. Attachment supplies one
coherent complete host-authored structured presentation frame followed by
ordered frame revisions. The client sends semantic input; Orbit encodes it
against authoritative terminal state and alone answers terminal queries.

The client does not reconstruct terminal authority from Formatter output or
raw PTY bytes. The presentation boundary must account explicitly for rich
terminal state such as styled graphemes, cursor, title, hyperlinks, and graphics
resources or capabilities rather than becoming a plain-text grid ceiling.

Persistence across Orbit or machine restarts is outside the initial contract.

## Contract index

`docs/CONTRACTS.md` is the canonical index of accepted behavior, ownership,
proof status, checks, and gaps. Contract IDs are stable and repository-qualified
as `ORB-C*`; never renumber or reuse them.

Every product implementation bead must name the contract IDs it creates,
changes, proves, consumes, hardens, or preserves. A repository-tooling or
documentation bead instead states explicitly that it changes no product
contract. Do not introduce behavior without an indexed contract. If requested
work has no applicable contract, or would change, replace, retire, or add one,
stop for explicit user approval before editing the index or code.

Index only user-visible behavior, correctness boundaries, ownership invariants,
and cross-repository interfaces. Do not create contract IDs for functions,
modules, dependencies, configuration literals, individual tests, or speculative
future features.

## Contract proof lifecycle

`Proved` means the index names an accepted proof-bearing Git commit and the
canonical checks that passed for that exact revision. Passing checks in an
uncommitted worktree is candidate evidence and remains `Partially proved`.

Every later bead that touches a proved contract's owner or implementation
surface must classify the contract as preserved, hardened, or changed. It must
rerun the indexed checks against the resulting revision and update the proof
commit, or downgrade the contract and record the gap. Consumption and dogfood
do not silently refresh proof.

For a proof that begins uncommitted, keep the contract partial until the user
accepts and authorizes the proof-bearing commit. Commit the candidate while the
Bead remains in progress, then update the index with that exact revision and
close the Bead in a metadata follow-up. Never point proof at a moving branch.

## Fixed initial boundaries

- Linux first
- local Unix socket transport
- one terminal session
- one active client; multiplayer is a deliberate non-goal
- one Rust package and binary through the `orb-bi4.3` headless convergence
  proof
- no remote transport, Zellij compatibility, layouts, tabs, plugins, or
  configuration framework

Do not broaden these boundaries without an explicit user decision.

## Linux-first portable core

Linux is the only required implementation, packaging, and verification target
for the initial experiment. This does not authorize a Linux-shaped product
core. Terminal authority, semantic input, presentation frames, revisions,
protocols, and durable state must remain platform-neutral.

Keep PTY creation, process groups, signals, polling, file-descriptor flags,
socket paths and permissions, and platform-specific EOF or error behavior
behind one narrow platform seam. Prefer the smallest concrete seam justified by
current code; do not build a generic portability framework, a speculative trait
hierarchy, a macOS backend, cross-compilation, or macOS CI until the user
chooses that scope.

Every product implementation Bead records one portability disposition for its
result: `neutral`, `isolated platform dependency`, or `macOS blocker`. A blocker
states the exact assumption, affected contracts or boundary, replacement shape,
and estimated removal cost, then stops for user choice before it becomes part
of a cross-repository protocol or durable format.

The crate gate evaluates macOS feasibility, but portability alone does not
justify a larger dependency. Actual macOS behavior or support receives a
contract ID only after the user authorizes that product scope.

`orb-pmp` is the platform-seam gate. It must close after the Linux PTY candidate
and before `orb-bi4.3` begins the structured-presentation implementation.

`orb-bi4.3` was the repository boundary gate and passed. The private
`luccahuguet/venus` repository owns graphical implementation; `orb-bi4.4` is
the Orbit-side cross-repository acceptance gate. Keep graphical Venus code out
of Orbit. The proven structured presentation contract is the cross-repository
boundary.

## Clean-room rule

Mars and Mars Next are behavioral references only. Do not port, copy, or adapt
their code or architecture by default. If a particular piece appears worth
reusing, first state the contract it satisfies, why a smaller implementation is
insufficient, and ask the user to choose reuse.

## Execution baseline and concurrency

Read-only discovery does not require an execution baseline or Beads comment.
During a review, audit, or fresh-eyes pass, inspect the work first. If you find
no concrete production-code, test, or manifest edit, report the result and stop
without adding run comments. Once you identify a concrete edit, record the
baseline before making that edit. Do not add a baseline to reserve the
possibility of a change.

Before the first production-code, test, or manifest edit for an implementation
bead, claim it and add an append-only `Execution baseline` comment containing:

- the current `HEAD` commit and `git hash-object AGENTS.md`
- the Bead's `updated_at` value and affected contract IDs with their routing
- the intended production, test, manifest, and generated surfaces
- pre-existing dirty paths that the run must preserve
- the active agent or session identity

The baseline freezes the governing Bead scope, contracts, gates, and repository
instructions for that run. Later policy or Bead changes apply to the next run,
not retroactively, unless the user explicitly adopts them for the active run or
an unaddressed P0 or P1 correctness risk requires a stop. Explicit new user
direction always wins; record a `Rebaseline` comment before continuing under a
materially changed scope.

Other agents may inspect or review claimed work, but must treat its listed
implementation surfaces and governing Bead fields as read-only unless the user
explicitly coordinates shared work. They may append factual findings without
rewriting the claimant's baseline or gates.

## Evidence-first implementation gate

Every implementation bead must classify its references as required before
code, conditional on a named failure, comparison only, or rejected scope. If an
implementation bead names a reference without a classification, treat it as
required before code.

Run this gate after choosing a concrete edit and recording its execution
baseline. You may inspect references during a read-only pass without adding a
`Reference gate` comment. If you proceed to an edit, complete the steps below
before changing production code, tests, or manifests.

After recording the execution baseline and before editing production code,
tests, or manifests for an implementation bead:

1. Reread the complete Bead with `br show <id> --json` and its comments with
   `br comments list <id> --json`
2. Read the relevant entries in `docs/REFERENCES.md`
3. Inspect every required reference at a recorded release or commit
4. Add a pre-implementation, append-only `Reference gate` comment with
   `br comments add <id> --message "..."` that records:
   - contract IDs affected
   - exact reference identities
   - the relevant contract, mechanism, or independently reproduced failure
   - the lesson Orbit accepts
   - the surrounding behavior and scope Orbit rejects
   - the consequence for ownership, dependency choice, code shape, and the
     first contract check
5. Only then choose or implement the production code shape

Research or a `Reference gate` comment written after implementation does not
satisfy this gate. If evidence contradicts the bead, requires an unapproved
dependency, changes an ownership boundary, or reaches a stated decision gate,
stop and ask the user before code.

A research spike may write focused proof code as part of gathering evidence.
It must stop at its declared decision boundary and may not turn the proof into
production architecture without a user decision.

## Crate gate

`docs/CRATES.md` is the durable index of direct and architecture-shaping crate
decisions. Studying a crate in `docs/REFERENCES.md` does not approve it.

A `Crate gate` is required before adding, replacing, or removing a direct
crate, enabling a dependency feature that expands ownership or build surface,
making a major-version change, or choosing a crate instead of a reasonable
implementation using the standard library or existing dependencies. Routine
lockfile regeneration from an unchanged manifest is not a crate decision, but
record why it changed.

Compare a broad credible candidate set, normally at least three implementation
shapes including no new direct crate or reuse of an existing dependency. Do not
pad the set with unsuitable crates; when fewer credible options exist, record
where the search was made and why the set is smaller. For each candidate,
record exact release or commit and compare:

- fit to the affected contracts and ownership architecture
- project-owned LOC avoided or introduced
- direct and transitive dependency footprint
- API maturity, maintenance and upgrade burden
- Linux fit, macOS feasibility, and future portability implications
- unsafe, native, build-tool, offline/Nix, license, and security consequences
- focused proof or measurement where claims cannot be settled from source

Add an append-only `Crate gate` comment with the candidate matrix, accepted and
rejected shapes, recommendation, and effect on code ownership and the first
check. Stop for user approval before changing the manifest. The gate may select
no new crate. At closure, update `docs/CRATES.md` with the selected version,
alternatives, dependency footprint, LOC result, and evidence Bead.

## Cross-repository compatibility

Before another repository consumes an Orbit contract, it records the exact
Orbit proof commit and contract IDs. Any later change to that boundary must be
classified as compatible or breaking, list every known consumer, and define
the update order and support or removal of the previous revision. Breaking
changes, adapters, and compatibility windows require an explicit user choice.
Clients must report a gap back to the owning contract rather than silently
compensating and creating a second owner.

## Validation consumers and test doubles

A diagnostic client, recorded-frame corpus, replay source, renderer experiment,
or fake child process proves only the boundary it actually exercises. It does
not become an alternate product backend or an independent owner of Orbit,
Venus, or Astra behavior.

Protocol-faithful validation pins the exact Orbit proof commit and contract IDs
and consumes the canonical wire representation. Do not maintain a second
handwritten protocol schema, reparse raw PTY bytes, or compensate for missing
server state in a test client. If replay would require a public trace format,
new compatibility surface, adapter, package, or binary, stop for explicit user
choice before adding it.

Mark lossy projections explicitly. A terminal or text projection may prove
attachment, ordering, reconnect, resize, semantic input, and failure behavior,
but it cannot prove `ORB-C6` rich-presentation fidelity. Likewise, an Astra
process or package probe may prove Astra-owned launch and orchestration policy,
but not Orbit runtime or Venus rendering behavior.

## Protocol exceptions

Do not improvise around a missed gate. Stop and record the unmet rule and the
state of the work. Only the user may choose one of these outcomes:

- restart or redo the affected work under a fresh execution baseline
- accept a narrowly named exception and decide whether its evidence counts
- reject or remove the work

Record an accepted exception in an append-only `Protocol exception` comment
with the exact rule, reason, scope, user decision, evidence retained, and effect
on contract proof. An exception never pretends the skipped gate passed, does
not apply to another Bead, and does not authorize unrelated scope or
dependencies.

## Bead execution size

An epic is a planning container, not a production-code, test, or manifest
implementation unit. Do not claim an epic for implementation. Before code,
activate or create a child Bead with one code-owning repository, one primary
invariant or failure domain, one cohesive contract cluster, and one cheapest
meaningful proof. Sequence cross-repository work as owner implementation,
consumer implementation, and acceptance rather than editing both owners in one
run. Close the epic only after its required children and acceptance evidence
are reconciled.

During Beads priming and before claiming any non-epic implementation Bead,
check whether it spans independent state machines, repository owners, or proof
surfaces. Split it before implementation when those parts can fail and be
proved independently. Repeated fresh-eyes findings across different subsystems
are evidence that future work needs smaller Beads; they are not a reason to
stop reviewing or to cap the number of passes.

## Method

For a review, audit, or fresh-eyes request, investigate read-only first. Stop
without Beads run comments when you find no concrete edit. If you find one,
enter this method before changing files.

1. State the user-visible behavior and indexed contract IDs for the slice.
2. Claim the Bead and record the execution baseline.
3. Pass the evidence-first implementation gate.
4. Pass the crate gate if dependency choice is in scope.
5. Choose one owner and the smallest code shape justified by the evidence.
6. Write the cheapest meaningful check.
7. Implement the smallest vertical slice that passes it.
8. Stop and review before widening scope.

Use TDD for deterministic Rust behavior, protocol framing, terminal-state
convergence, PTY lifecycle, and regressions. Do not build speculative
abstractions or scaffolding for later beads.

Keep tests strong and few. Prefer one real contract test over several tests of
implementation details.

## Git workflow

Work directly on `edge`. Do not create or push another branch unless the user
requests it. Keep history linear; do not force-push published history.

## Changelog

`CHANGELOG.md` is the chronological source for accepted user- and
consumer-visible changes. Update its `Unreleased` section in the Bead that
accepts runtime behavior, commands, socket or wire behavior, cross-repository
contracts, packaging requirements, breaking changes, releases, or graduation
decisions.

Do not add candidate behavior before its proof-bearing commit is accepted. Do
not record refactors, test counts, LOC changes, reference research, Beads
maintenance, or repository-governance edits. Breaking entries name the affected
`ORB-C*` contracts and exact proof revision. Create versioned sections only
when a release exists.

## Beads

Use `br` for all issue work. Do not edit `.beads/` files directly. Serialize
`br` writes and run `br sync --flush-only` before committing Beads changes.

Use `bv` only with `--robot-*` flags; bare `bv` opens an interactive TUI.

Do not close an implementation bead unless its append-only `Reference gate`
comment and any required `Crate gate` comment follow its execution baseline and
predate implementation. Its notes record the chosen ownership and code shape,
rejected alternatives, contract checks and results, dependency footprint,
owned LOC, remaining limitations, and exact verification commands. Any
protocol exception must be explicit and user-approved.
Update `docs/CONTRACTS.md` with the resulting owner, proof status, checks, and
gap for every affected contract before closure.
Update `docs/CRATES.md` for every accepted direct or architecture-shaping crate
decision before closure.
Decision and research beads instead record their evidence, rejected
alternatives, and explicit user decision or stop reason. Retrospective comments
or notes do not repair a skipped pre-implementation gate.

## LOC discipline

Prefer deleting scope and avoiding abstractions. Update the README LOC
scorecard whenever Rust source changes. Keep `rustfmt` output even when it costs
lines.

## Verification

Run the cheapest exact checks for the changed surface. At minimum, keep these
green for Rust changes:

```sh
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
git diff --check
```
