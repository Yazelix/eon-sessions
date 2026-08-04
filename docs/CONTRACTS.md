# Orbit contract index

This is the canonical index of Orbit's accepted behavioral contracts,
ownership boundaries, proof status, and remaining gaps. `AGENTS.md` governs how
agents work, Beads govern authorized changes, and this index records what the
system must preserve.

## Status

- **Planned:** accepted contract with no sufficient implementation evidence
- **Partially proved:** useful evidence exists, but a required behavior,
  integration boundary, review, or decision remains open
- **Proved:** the owning Bead is accepted and the listed immutable revision and
  checks cover the contract
- **Retired:** the user explicitly replaced or removed the contract; its ID is
  never reused

## Current contracts

| ID | Contract | Owner | Status | Accepted proof revision | Check or evidence | Gap |
| --- | --- | --- | --- | --- | --- | --- |
| `ORB-C1` | While Orbit remains running, graphical-client disconnection does not kill the PTY or foreground process, and a later client reaches the same live session | Orbit authoritative owner loop with isolated platform PTY lifecycle | Proved | `e4fde443e625180d7332eff4dff366f64bee30a8` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) detach, aborted-attach, reattach, foreground-reaping, and signal-cleanup checks; canonical Rust verification suite | Restart persistence remains outside the initial contract |
| `ORB-C2` | Orbit is the sole libghostty terminal-state owner and the only component that sends terminal-generated responses to the PTY | Orbit platform-neutral authoritative owner loop | Proved | `e4fde443e625180d7332eff4dff366f64bee30a8` | [`only_authoritative_terminal_emits_pty_responses`](../tests/formatter_attachment.rs), `exited_child_output_is_drained_into_authoritative_state`, real-PTY integration checks, and canonical Rust verification suite | Preserve the sole-authority and response path through later client work |
| `ORB-C3` | Exactly one local client may attach; a simultaneous second client receives deterministic rejection without disturbing the active client | Orbit authoritative owner loop with isolated local-runtime mechanics | Proved | `e4fde443e625180d7332eff4dff366f64bee30a8` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) BUSY, aborted-handshake, and socket-identity checks; canonical Rust verification suite | Slow-client and backpressure hardening remains in `orb-bi4.5` under `ORB-C7` |
| `ORB-C4` | Attachment begins with one coherent complete presentation frame at revision N followed by strictly ordered later frame revisions, with no stale final presentation | Orbit authoritative owner loop (presentation publication) | Proved | `e4fde443e625180d7332eff4dff366f64bee30a8` | [`real_pty_reattach_converges_through_complete_ordered_frames`](../src/presentation.rs) and canonical Rust verification suite | Adversarial exit, failure, and sustained-output cases remain in `orb-bi4.5` |
| `ORB-C5` | Clients send semantic key, mouse, focus, paste, and resize events; Orbit encodes them against authoritative terminal state | Orbit platform-neutral authoritative owner loop (input encoding) | Proved | `e4fde443e625180d7332eff4dff366f64bee30a8` | Authoritative key, mouse, focus, paste, resize, and terminal-reply checks in [`tests/formatter_attachment.rs`](../tests/formatter_attachment.rs) and [`tests/lifecycle.rs`](../tests/lifecycle.rs); canonical Rust verification suite | Venus must pin this revision before consuming the input boundary |
| `ORB-C6` | The presentation boundary carries rich terminal state or explicitly declares unsupported capabilities; it never silently collapses the contract to plain text | Orbit authoritative owner loop (presentation extraction) | Proved | `e4fde443e625180d7332eff4dff366f64bee30a8` | `orb-bi4.1` Formatter counterexamples plus the rich-state corpus in [`src/presentation.rs`](../src/presentation.rs) | Version 1 explicitly leaves Kitty graphics unsupported; expanding that capability requires a later user decision |
| `ORB-C7` | A slow, broken, or disconnected client cannot block authoritative PTY processing or cause unbounded buffering or frame history | Orbit authoritative owner loop (attachment transport) | Partially proved | — | `orb-bi4.3` at `e4fde443e625180d7332eff4dff366f64bee30a8`: bounded frame/output constants, adjacent-frame coalescing, and unread-client real-PTY check | `orb-bi4.5` retains adversarial failure cleanup and sustained backpressure hardening |

## Rules

- Contract IDs are stable and repository-qualified. Never renumber or reuse an
  ID; mark an explicitly removed contract retired.
- Only user-approved user-visible behavior, correctness boundaries, ownership
  invariants, and cross-repository interfaces receive contract IDs.
- Do not index functions, modules, dependencies, configuration literals,
  individual tests, or speculative future features.
- Every product implementation Bead names the contract IDs it creates, changes,
  proves, consumes, hardens, or preserves. If requested behavior has no indexed
  contract, stop for user approval before changing code or the index.
- The Bead's Reference gate records how evidence constrains the implementation;
  this index records the durable contract rather than duplicating research
  notes.
- Closing an implementation Bead updates the status, owner, checks, and gap for
  every affected contract.
- A `Proved` contract names the exact accepted proof-bearing commit. A later
  change to its owner or implementation surface reruns the indexed checks and
  advances that revision, or downgrades the status and records the gap.

## Consumer compatibility

A repository consuming an Orbit contract pins the exact accepted proof
revision and contract IDs. Boundary changes are classified as compatible or
breaking and name affected consumers and update order. A client does not hide a
server-contract gap behind an adapter or second source of authority without an
explicit user decision.

The `orb-bi4.3` frame version is accepted at
`e4fde443e625180d7332eff4dff366f64bee30a8` and has no cross-repository
consumer yet. Venus must pin that revision and the consumed contract IDs when
its private repository is created; later breaking changes require an explicit
user decision and coordinated update order.

## Outside the initial index

Orbit's accepted long-term direction includes host-owned durable workspace
topology and native, web, or mobile clients. Multiple sessions, windows, tabs,
splits, restart recovery, remote transport, web, mobile, and multiplayer are not
initial Orbit contracts and receive no IDs until the user authorizes their
implementation scope. macOS credibility is an architecture discipline rather
than a claim of supported behavior; actual macOS runtime support also receives
no contract ID until the user authorizes it.
