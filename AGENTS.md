# Agent Guidelines

Orbit is a clean-room Rust experiment for the durable local terminal-session
runtime beneath Venus, the greenfield graphical client for Yazelix Nova. Mars
remains the current graphical product while the experiment runs.

`Logimux` is this repository's shorthand for Superlogical's currently unnamed
terminal multiplexer. It is not an official Superlogical product name.

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

`orb-bi4.3` is the repository boundary gate. If it passes, create the private
`luccahuguet/venus` repository before `orb-bi4.4` and keep graphical Venus code
out of Orbit. The proven structured presentation contract is the
cross-repository boundary.

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
