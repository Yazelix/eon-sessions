# Agent Guidelines

This file is self-contained. Canonical protocol text was rendered into it;
the source repository is needed only to update or verify the import.
Do not edit this generated file directly. Edit `.agent-protocols.local.md`
or `.agent-protocols.exceptions.json`, then render from the pinned source.

## Protocol import record

- Source: `https://github.com/luccahuguet/starcompass`
- Source commit: `c524d3c47592ce006b749d0c6db7fd0c3478ce63`
- Profiles: `greenfield`, `terminal-engine`
- Manifest: `.agent-protocols.json` (schema 1)

| Protocol | Version | SHA-256 |
| --- | ---: | --- |
| `AP-SCOPE-001` | 3 | `cae30f9031beea4f831a89445f35807f50618e0f2cb0432f3b94bec40da9aeef` |
| `AP-CONTRACT-001` | 4 | `cdf2e5d69cefd34ceeeaa6b7511b21f921e2be50c17d3c01b85120b20234cff7` |
| `AP-REFERENCE-001` | 3 | `186fa74aa530c9c76ae507685461dff2b1506ecb5087230ded51f02cecb9231a` |
| `AP-MINIMAL-001` | 4 | `612bc9c62a20adf7332724d4663af0a0e591d624de2a76b1c257a9ed428db1f6` |
| `AP-DEPENDENCY-001` | 2 | `ded49add538b82de0a9c522bc8a34720f4ebbb47f39fc2f7ddb25e7aad1700d3` |
| `AP-OWNERSHIP-001` | 2 | `d218a4e0625b659ec366284110bdfce02bd66cb229ab6ba07f2317092ea13053` |
| `AP-TEST-001` | 3 | `58b5837cb679e958192b366bb15d6e34649f5a91ff9f4accdc7edb4ef5cdb873` |
| `AP-PROOF-001` | 4 | `2762fdf80ba2a36bb1e8844a44959bea7a6309dcd7a5796029a06a7ad9c26692` |
| `AP-PLAN-001` | 9 | `a1671c41d8a5c059a7138914ae4903b1d305473cf9681e293fa7d16c0b939b15` |
| `AP-CI-001` | 3 | `c7cb65a81ce8434d02f2d19306bf93358f3624d0b77886bc91d4de93f7a779ba` |
| `AP-EXCEPTION-001` | 2 | `2c4f00299922edac83286821af07ad485e5a630bca6acbbce918d87614abb395` |
| `AP-GIT-001` | 5 | `f2c3254311de57a23a13aa382fe3285e65bb34e535ecd813d1be783ca79d4bf9` |
| `AP-PORTABILITY-001` | 2 | `a2f5025bc7a10436b5c2f8f42c6002ccf1e361f05abbff3dea6507a3426394c0` |
| `AP-TERMINAL-001` | 2 | `c1805e12ca5c41d3ed2a6b24739fcded2ad986758fda01fdfa3132c9f8351d9c` |
| `AP-FAILURE-001` | 4 | `450e48b4e1532ff50a9a1da7faa49e3539450e0676f3c530f888d33b4b752e80` |

### Local exceptions

No local exceptions.

## Canonical protocols

### AP-SCOPE-001 — User-owned scope

The user owns scope. Inspection, audit, diagnosis, explanation, and
recommendation authorize no implementation, external-state, or durable-planning
write. Implement only a chosen outcome.

Do not silently add features, compatibility promises, public surfaces,
migrations, repositories, or planning items. Preserve user-owned inputs and
artifacts; replace or delete an exact target only when the outcome requires it.
Otherwise write a distinct result. State consequential assumptions and stop
when a missing scope choice would materially change the result. “Finish” adds
persistence, not authority. Report out-of-scope findings unless the user chose
a durable destination.

### AP-CONTRACT-001 — Contract-driven changes

Before code shape, state the smallest observable contract: consumer, trigger,
result, and important failures. Of the contracts consistent with the request
and evidence, choose the fewest unsupported guarantees or restrictions; leave
reasonable future behavior unspecified. Give it a stable ID only when later
consumers need one.

Keep docs, help, examples, and configuration aligned with current commands,
paths, flags, defaults, and availability; label planned, partial, or gated
behavior. Choose one source-of-truth owner, the cheapest falsifying check, and
the smallest complete vertical slice. Update the contract first for an
intentional behavior change. Implementation details are not contracts unless a
component must rely on them.

### AP-REFERENCE-001 — Evidence before code shape

Before a consequential code shape, read affected instructions, code, contracts,
tests, and required subsystem references. Record the adopted, rejected, or
unresolved mechanism; separate evidence from inference and revisit it after a
material shape change. Memory, summaries, and reputation are discovery only.

Apply source-license terms to their exact actors, uses, conditions,
beneficiaries, and direction. Choosing a provider does not make a user act for
it; an “including” example remains scoped by its condition. Inspection is
distinct from copying, adaptation, redistribution, dependency selection, and
incorporation, so a restriction on one does not spread to another. If an
interpretation would block required evidence, identify the exact clause and
roles and resolve material ambiguity with the user. Review does not require
reuse.

### AP-MINIMAL-001 — Minimum sufficient implementation

After identifying the contract, evidence, owner, flow, and boundaries, take the
first sufficient option: no change; existing owner, helper, or pattern;
standard library or native platform; accepted dependency that owns the
behavior; minimum correct local code.

Judge the whole lifecycle, including duplicate truth, coordination, coupling,
migration and removal, portability, operations, proof, and agent context. Scope
instructions narrowly; retain non-obvious constraints and reusable
behavior-changing workflows, not generic or duplicated policy. Load details
only when needed.

A patch is not minimal if it preserves a wrong or duplicate owner, treats a
symptom below its shared cause, bypasses a boundary, or raises downstream cost.
Prefer deletion, direct ownership, and fewer files; use patch size only between
equally correct system shapes. Never remove required behavior, trust-boundary
validation, data-loss protection, security, accessibility, or the cheapest
runnable check for non-trivial logic.

When available, use upstream
[Ponytail](https://github.com/DietrichGebert/ponytail/tree/16f29800fd2681bdf24f3eb4ccffe38be3baec6b)
as a fallible code-shape bias after these constraints. Its absence does not
suspend them, and its hooks do not prove compliance.

### AP-DEPENDENCY-001 — Dependency gate

Before adding a dependency, name its capability and contract. Compare owned
code, the standard library, and credible candidates for correctness,
maintenance, platform and license fit, transitive weight, API stability, and
net complexity. Record the choice, meaningful rejections, and replacement cost;
pin it and prove the relied-on behavior. Remove it when its ownership no longer
justifies its cost.

### AP-OWNERSHIP-001 — One owner per invariant

Give every invariant, state transition, and user-visible policy one owner;
consumers must not reconstruct or reinterpret it. Name the owner before adapters
or synchronization and delete duplicates. Put policy at the highest layer with
enough context and enforcement at the lowest correct layer. Make cross-boundary
data explicit and versioned for independently released consumers.

### AP-TEST-001 — Strong and few tests

Protect meaningful contracts, regressions, boundaries, and failures with a few
strong tests. Use TDD for deterministic helpers, parsers, protocol behavior,
and regressions whose expected behavior is known first; use contract-first
integration checks for layout, runtime, architecture, forks, and dogfood.

For consequential agent-instruction, prompt, or skill changes, run isolated
representative tasks without supplying the expected conclusion. Structural
checks prove structure, not agent behavior. Test observable effects; delete
duplicate proof, implementation trivia, and scaffolding. Guard absence only
when absence is a security, licensing, size, ownership, or known-regression
contract.

### AP-PROOF-001 — Explicit proof lifecycle

Proof supports only its recorded command or observation, revision, environment,
result, and exercised surface. Distinguish proposed, implemented, mechanically
verified, dogfooded, accepted, and promoted states; rerun stale proof and never
widen its claim.

For performance claims, measure the bottleneck, compare the same representative
workload and environment with a recorded baseline, retain a correctness oracle,
and report a distribution or bound. Preserve constraining negative results.

A review or simplification pass that materially changes its subject invalidates
completion. Repeat it on the revised state and declare convergence only after a
full pass finds no actionable in-scope bugs or simplifications.

### AP-PLAN-001 — Durable planning state

Keep later-needed outcomes and constraints in the designated planning system or
canonical docs. Issues represent chosen goals, decisions, material defects, or
schedulable follow-ups; methods stay in their owning issue.

A run includes automatic continuations and ends when control returns. Planning
reads are unrestricted. A run that writes planning state or implementation uses
one shape:

1. Own at most one issue; create, claim, update, implement, or close only it.
2. With explicit user authorization, create, update, or close a named or
   accepted planning-only batch; claim nothing and edit no implementation.

Do not combine these shapes in one run.

Bind implementation work to its issue before planning or code writes. When that
issue completes, blocks, or hands off, return without starting another. Report
unapproved findings; create separate issues only for material out-of-scope or
independently schedulable work, and outside an authorized batch defer creation.

Keep editable fields at accepted current state after review, simplification, or
verification; reserve append-only history for needed evidence. Preserve
contracts, decisions, dependencies, acceptance evidence, material negative
results and failures, constraining rejections, and approvals. Keep raw logs with
their proof. Keep status honest, model only real prerequisites, and reconcile
before handoff. Use the designated issue tool; never edit its storage directly.

### AP-CI-001 — Bounded continuous integration

Before hosted automation, name its contract and why local proof is
insufficient. Bound triggers, jobs, timeouts, permissions, artifacts, cache,
concurrency; specify fork and secret behavior. Prefer one cheap deterministic
job. Record when to expand, reduce, or remove. Confidence must justify its
financial, latency, security, and maintenance costs, including private minutes
and storage.

### AP-EXCEPTION-001 — Explicit local exceptions

Only an exception approved by the user or named authority may narrow, replace,
or suspend an import. Record protocol ID, exact scope, reason, approver, date,
and any expiry or review condition. Unrecorded conflicts are drift; additive
detail is an overlay.

### AP-GIT-001 — Safe repository history

Repository history is shared user state. Inspect status and local policy;
preserve unrelated or concurrent work and use the least destructive sufficient
operation.

Amend the unpublished task commit for same-task corrections; refresh downstream
commits and generated artifacts. After external reliance (push, promotion,
release, or pin), commit corrections separately. Never reset, discard,
force-push, rewrite published history, or create a branch without authority.
Verify intended diff before commit and remote state after push. Roll back with a
reviewed revert unless local policy says otherwise.

### AP-PORTABILITY-001 — Portable core with explicit platform seams

Keep multi-OS contracts and state neutral; isolate system APIs, processes,
paths, packaging, and events. Evaluate targets before foundational runtime,
graphics, filesystem, or transport choices. Prove behavior on each platform,
not by compilation; name unsupported targets.

### AP-TERMINAL-001 — Terminal state authority

One component owns terminal parsing, grid, modes, scrollback, selection,
graphics, title, and process attachment; others consume versioned projections,
never a second emulator. Define replication as raw bytes, checkpoints plus
tails, structured frames, or another contract. Cover reconnect, resize,
alternate screen, partial input, and slow consumers; test semantic attachment
equivalence, not visuals alone.

### AP-FAILURE-001 — Designed lifecycle and failure semantics

Long-lived sessions and services define start, attach/detach, crash/restart,
shutdown, version mismatch, abandonment; bound queues, memory, retries,
timeouts, and slow-consumer impact. Own cancellation and idempotent cleanup;
persist only required recovery state. Test deterministic failures when
practical; dogfood other process/platform cases.

## Repository-local rules

### Eon Sessions repository rules

Eon Sessions is the repository for Eon's durable terminal sessions. Its Orbit
subsystem is a clean-room Rust experiment beneath the Venus subsystem in Eon
Desktop. Yazelix Nova remains an independently valuable product on its current
Mars and Zellij architecture while the experiment runs.

Use `Superlogical terminal multiplexer` for Superlogical's published first
product. Do not invent a shorter product name or research codename.

## Status

Orbit's accepted runtime proofs remain x86_64 Linux. On 2026-09-11 the user
activated one Nix-only `aarch64-darwin` expansion for Eon. `ORB-C14` owns that
planned platform proof. Orbit is the sole active implementation frontier;
Venus and Eon macOS implementation wait for its accepted revision. Apple
Silicon macOS is not supported until the contract is proved.

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

## Fixed product boundaries

- x86_64 Linux proved first; `aarch64-darwin` is the only active expansion
- local Unix socket transport
- one terminal session
- one active client; multiplayer is a deliberate non-goal
- one Rust package and binary through the `orb-bi4.3` headless convergence
  proof
- no remote transport, Zellij compatibility, layouts, tabs, plugins, or
  configuration framework

Do not broaden these boundaries without an explicit user decision.

## Portable core and active Apple Silicon frontier

x86_64 Linux remains the only proved Orbit platform. `aarch64-darwin` is the
only additional implementation and verification target authorized by the user;
its initial package path is Nix-only. Terminal authority, semantic input,
presentation frames, revisions, protocols, and durable state remain one shared
platform-neutral core.

Keep PTY creation, process groups, signals, polling, file-descriptor flags,
socket paths and permissions, and platform-specific EOF or error behavior
behind one narrow platform seam. Prefer the smallest concrete seam justified by
current code. Add only Darwin mechanics required by `ORB-C14`; do not build a
generic portability framework, speculative trait hierarchy, second owner loop,
or another platform backend. One bounded Apple Silicon hosted-CI job may prove
headless runtime behavior that cannot run locally, but compilation alone is not
runtime proof.

Every product implementation Bead records one portability disposition for its
result: `neutral`, `isolated Linux dependency`, `isolated Darwin dependency`,
or `platform blocker`. A blocker states the exact assumption, affected
contracts or boundary, replacement shape, and estimated removal cost, then
stops for user choice before it becomes part of a cross-repository protocol or
durable format.

Portability alone does not justify a larger dependency. `x86_64-darwin`,
signing, notarization, direct distribution, remote transport, and restart
persistence remain outside `ORB-C14`.

`orb-pmp` is the platform-seam gate. It must close after the Linux PTY candidate
and before `orb-bi4.3` begins the structured-presentation implementation.

`orb-bi4.3` was the repository boundary gate and passed. The private
`Yazelix/eon-desktop` repository owns graphical implementation through Venus;
`orb-bi4.4` is the Orbit-side cross-repository acceptance gate. Keep graphical
Venus code out of Orbit. The proven structured presentation contract is the
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

## Validation consumers and test doubles

A diagnostic client, recorded-frame corpus, replay source, renderer experiment,
or fake child process proves only the boundary it actually exercises. It does
not become an alternate product backend or an independent owner of Orbit,
Venus, or Eon behavior.

Protocol-faithful validation pins the exact Orbit proof commit and contract IDs
and consumes the canonical wire representation. Do not maintain a second
handwritten protocol schema, reparse raw PTY bytes, or compensate for missing
server state in a test client. If replay would require a public trace format,
new compatibility surface, adapter, package, or binary, stop for explicit user
choice before adding it.

Mark lossy projections explicitly. A terminal or text projection may prove
attachment, ordering, reconnect, resize, semantic input, and failure behavior,
but it cannot prove `ORB-C6` rich-presentation fidelity. Likewise, an Eon
process or package probe may prove Eon-owned launch and orchestration policy,
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
