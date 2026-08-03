# Agent Guidelines

Orbit is a clean-room Rust experiment for the durable local terminal-session
runtime beneath Yazelix Nova and, if proven, the Mars graphical client.

## Core rule

The user decides scope. Do not add a feature, compatibility surface, module,
dependency, or planning bead until the user has chosen that direction.

## Irreducible contract

Orbit owns a real PTY process and authoritative libghostty terminal state. The
process survives graphical-client exit or disconnection while Orbit remains
running. Exactly one local client may attach. Attachment supplies a coherent
terminal snapshot and then every subsequent PTY byte in order, without gaps or
duplication.

Persistence across Orbit or machine restarts is outside the initial contract.

## Fixed initial boundaries

- Linux first
- local Unix socket transport
- one terminal session
- one active client; multiplayer is a deliberate non-goal
- one Rust package and binary until separation has a proven owner or contract
- no remote transport, Zellij compatibility, layouts, tabs, plugins, or
  configuration framework

Do not broaden these boundaries without an explicit user decision.

## Clean-room rule

Mars and Mars Next are behavioral references only. Do not port, copy, or adapt
their code or architecture by default. If a particular piece appears worth
reusing, first state the contract it satisfies, why a smaller implementation is
insufficient, and ask the user to choose reuse.

## Method

1. State the user-visible contract for the slice.
2. Choose one owner.
3. Write the cheapest meaningful check.
4. Implement the smallest vertical slice that passes it.
5. Stop and review before widening scope.

Use TDD for deterministic Rust behavior, protocol framing, terminal-state
convergence, PTY lifecycle, and regressions. Do not build speculative
abstractions or scaffolding for later beads.

Keep tests strong and few. Prefer one real contract test over several tests of
implementation details.

## Git workflow

Work directly on `edge`. Do not create or push another branch unless the user
requests it. Keep history linear; do not force-push published history.

## Beads

Use `br` for all issue work. Do not edit `.beads/` files directly. Serialize
`br` writes and run `br sync --flush-only` before committing Beads changes.

Use `bv` only with `--robot-*` flags; bare `bv` opens an interactive TUI.

## LOC discipline

Prefer deleting scope and avoiding abstractions. Update the README LOC
scorecard whenever Rust source changes. Keep `rustfmt` output even when it costs
lines.

## Verification

Run the cheapest exact checks for the changed surface. At minimum, keep these
green for Rust changes:

```sh
cargo fmt --check
cargo check
cargo test
```
