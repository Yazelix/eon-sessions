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
| `ORB-C1` | While Orbit remains running, graphical-client disconnection does not kill the PTY or foreground process, and a later client reaches the same live session | Orbit authoritative owner loop with isolated platform PTY lifecycle | Proved | `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) typed detach, silent, aborted, and simultaneous attachment, retained-size reattachment, closed-PTY known-child survival, known-child and writing-descendant exit, foreground reaping, and signal and socket cleanup checks; canonical Rust verification suite | Restart persistence remains outside the initial contract |
| `ORB-C2` | Orbit is the sole libghostty terminal-state owner and the only component that sends terminal-generated responses to the PTY | Orbit platform-neutral authoritative owner loop | Proved | `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779` | [`parser_state_and_terminal_replies_survive_client_failure`](../tests/lifecycle.rs) splits private-mode parser state across disconnect, observes one ordered detached DSR reply from the child, rejects attachment-time duplication, and verifies semantic alternate/primary-screen and cursor state through canonical frames; [`only_authoritative_terminal_emits_pty_responses`](../tests/formatter_attachment.rs); multi-read exit draining; canonical Rust verification suite | Ordered client-visible effects remain a separate unproved transport gap |
| `ORB-C3` | Exactly one local client may attach; a simultaneous second client receives deterministic rejection without disturbing the active client | Orbit authoritative owner loop plus canonical `orbit-protocol` session codec | Proved | `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) strict negotiation, one-second silent-peer release, typed Busy, abort recovery, canonical ordering validation, and socket-identity checks; transport-pressure and terminal-closing exclusions; fatal-client retained-storage release; role-specific ORBS header checks; canonical Rust verification suite | Terminal-semantic attachment hardening remains in `orb-bi4.5.3` |
| `ORB-C4` | Attachment begins with one coherent complete presentation frame at revision N followed by strictly ordered later frame revisions, with no stale final presentation | Orbit authoritative owner loop plus canonical `orbit-protocol` session and complete-frame codecs | Proved | `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779` | [`real_pty_reattach_converges_through_complete_ordered_frames`](../src/presentation.rs) includes a 4 MiB stopped-reader pressure phase, continuously emitting child, semantic interrupt, ordered complete frames, and coherent reattach; retained dimensions on the real reattachment frame; bounded multi-read final-state materialization; terminal-closing frame exclusion; strict canonical codec and reducer checks | Semantic-input and rich-fidelity attachment hardening remains in `orb-bi4.5.3` |
| `ORB-C5` | Clients send semantic key, mouse, focus, paste, and resize events; Orbit encodes them against authoritative terminal state | Canonical platform-neutral `orbit-protocol` values plus Orbit authoritative input encoding | Proved | `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779` | Complete typed key and mouse codec checks, including encode/decode rejection of terminal-forbidden associated text, side-only modifier bits, buttonless press/release, wheel release/motion, and mapper-domain coordinate or surface overflow; `outside_mouse_reporting_uses_held_button_semantics`; transport-pressure, terminal-closing, and closed-PTY input exclusion; authoritative ordinary/special key, mouse, focus, arbitrary multiline paste, full-surface PTY pixel resize, and terminal-reply checks; canonical Rust verification suite | Native Venus input-method and window integration remain consumer work, not an Orbit fallback surface |
| `ORB-C6` | The presentation boundary carries rich terminal state or explicitly declares unsupported capabilities; it never silently collapses the contract to plain text | Orbit authoritative extraction plus canonical `orbit-protocol` values and codecs | Proved | `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779` | `orb-bi4.1` Formatter counterexamples, the real-terminal rich-state corpus in [`src/presentation.rs`](../src/presentation.rs), strict canonical ORBF-in-ORBS codec coverage, and canonical Rust verification suite | Version 1 explicitly leaves Kitty graphics unsupported; expanding that capability requires a later user decision |
| `ORB-C7` | A slow, broken, or disconnected client cannot block authoritative PTY processing or cause unbounded buffering or frame history | Orbit authoritative owner loop (attachment transport) | Proved | `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779` | Fixed protocol and PTY-write bounds, whole-input pressure failure and client closure, one-read steady-state fairness, terminal-response overflow failure, 4 MiB stopped-reader convergence, bounded silent-peer release, deterministic simultaneous rejection, one-shot process-group cleanup, four-read post-exit work bound, and writing-descendant shutdown checks | The proof is limited to the accepted local one-client boundary |

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

A repository consuming Orbit contracts records each contract ID and its exact
accepted proof revision. Boundary changes are classified as compatible or
breaking and name affected consumers and update order. A client does not hide a
server-contract gap behind an adapter or second source of authority without an
explicit user decision.

ORBS v1, ORBF v1, and their canonical `orbit-protocol` package are accepted at
`838b67652c4df1979e599b9c401ee664ffac66bd`. Venus consumes the package at
`c905bf9610581747f1b07565814b501ca66cfaa6`; that revision is an ancestor of
`838b67652c4df1979e599b9c401ee664ffac66bd` and the current runtime proof
`8e3ba0beeb18157dc5a48c68c38812fa8d8fb779`. The protocol package, root
manifest, and lockfile are byte-identical across those revisions. The
intervening Orbit changes are compatible runtime resource-lifetime,
transport-pressure, lifecycle, and terminal-authority hardening, so they
require no Venus manifest migration.
Venus records the accepted consumer relationship at
`8149002c7275a00db1c08fded171e09f249dd977`. Later boundary changes require an
explicit compatible-or-breaking classification and coordinated update order;
a Venus adapter or second decoder does not substitute for updating the shared
owner.

`Complete` in `ORB-C4` means the entire ORBF v1 presentation payload for its
revision. Version 1 carries the current visible presentation and its metadata;
it does not transfer scrollback history or unsupported graphics. A future
snapshot, patch, history, effect, or resource design needs a user-approved
contract change before it alters this boundary. The accepted shared codec
preserves this wire contract and introduces no compatibility window or
independent release surface.

The ORBS v1 lineage intentionally and atomically replaced Orbit's private,
unaccepted line-oriented diagnostic syntax before Venus implementation. No
accepted external consumer used that syntax. Orbit's server, diagnostic client,
output queue, and tests moved together, and Venus pins the canonical package at
`c905bf9610581747f1b07565814b501ca66cfaa6`. The user selected no adapter,
compatibility window, or support for the removed syntax.

## Outside the initial index

Orbit's accepted long-term direction includes host-owned terminal-session state
and native, web, or mobile clients. Astra owns product workspace topology and
policy. Multiple sessions, windows, tabs, splits, restart recovery, remote
transport, web, mobile, and multiplayer are not initial Orbit contracts and
receive no IDs until the user authorizes their implementation scope. macOS
credibility is an architecture discipline rather than a claim of supported
behavior; actual macOS runtime support also receives no contract ID until the
user authorizes it.
