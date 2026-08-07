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
| `ORB-C1` | While Orbit remains running, graphical-client disconnection does not kill the PTY or foreground process, and a later client reaches the same live session | Orbit authoritative owner loop with isolated platform PTY lifecycle | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) typed detach, silent, aborted, and simultaneous attachment, retained-size reattachment, closed-PTY known-child survival, known-child and writing-descendant exit, foreground reaping, and signal and socket cleanup checks; canonical Rust verification suite | Restart persistence remains outside the initial contract |
| `ORB-C2` | Orbit is the sole libghostty terminal-state owner and the only component that sends terminal-generated responses to the PTY | Orbit platform-neutral authoritative owner loop | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | [`parser_state_and_terminal_replies_survive_client_failure`](../tests/lifecycle.rs) splits private-mode parser state across disconnect, observes one ordered detached DSR reply from the child, rejects attachment-time duplication, and verifies semantic alternate/primary-screen and cursor state through canonical frames; [`only_authoritative_terminal_emits_pty_responses`](../tests/formatter_attachment.rs); multi-read exit draining; canonical Rust verification suite | Ordered client-visible effects remain a separate unproved transport gap |
| `ORB-C3` | Exactly one local client may attach; a simultaneous second client receives deterministic rejection without disturbing the active client | Orbit authoritative owner loop plus canonical `orbit-protocol` session codec | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) strict negotiation, one-second silent-peer release, typed Busy, abort recovery, canonical ordering validation, and socket-identity checks; transport-pressure and terminal-closing exclusions; fatal-client retained-storage release; role-specific ORBS header checks; canonical Rust verification suite | No known initial-slice local attachment gap |
| `ORB-C4` | Attachment begins with one coherent complete presentation frame at revision N followed by strictly ordered later frame revisions, with no stale final presentation | Orbit authoritative owner loop plus canonical `orbit-protocol` session and complete-frame codecs | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | [`real_pty_reattach_converges_through_complete_ordered_frames`](../src/presentation.rs) resizes rich alternate-screen state, detaches, byte-compares the complete reattached frame for exact equality, then exercises a 4 MiB stopped-reader pressure phase, semantic interrupt, strictly ordered revisions, and bounded final primary-screen convergence; [`authoritative_viewport_routes_wheel_and_key_from_terminal_state`](../src/main.rs) retains the resized-state revision when bounded client output is full; terminal-closing frame exclusion; strict canonical codec and reducer checks; canonical Rust verification suite | No known initial-slice Orbit or Venus attachment gap; native Wayland and macOS remain unproved Venus platform surfaces |
| `ORB-C5` | Clients send semantic key, mouse, focus, paste, and resize events; Orbit encodes them against authoritative terminal state | Canonical platform-neutral `orbit-protocol` values plus Orbit authoritative input encoding | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | [`authoritative_input_modes_and_resize_survive_detach`](../tests/lifecycle.rs) captures exact consumed and active key modifiers, ordinary and application-cursor keys, full-surface pixel-mapped mouse with modifiers, focus, multiline bracketed paste, and Kitty release bytes, retains the full resize through detach, applies mode changes while detached, then proves normal key and paste encoding plus suppressed mouse, focus, and Kitty release events after reattachment with no duplicate bytes; complete typed rejection checks; held-button, PTY-pixel, transport-pressure, terminal-closing, and closed-PTY checks; canonical Rust verification suite | No known initial-slice Orbit or Venus semantic-input gap; candidate-list IME and native Wayland quality remain Venus-owned manual surfaces |
| `ORB-C6` | The presentation boundary carries rich terminal state or explicitly declares unsupported capabilities; it never silently collapses the contract to plain text | Orbit authoritative extraction plus canonical `orbit-protocol` values and codecs | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | `orb-bi4.1` Formatter counterexamples; the real-terminal corpus in [`src/presentation.rs`](../src/presentation.rs) checks exact default and palette colors, complete cell style, combining and wide graphemes, erased-cell background, cursor, title, working directory, screen, capabilities, resize, and equal reattachment; strict canonical ORBF-in-ORBS codec coverage; canonical Rust verification suite | Version 1 explicitly leaves Kitty graphics unsupported; expanding that capability requires a later user decision |
| `ORB-C7` | A slow, broken, or disconnected client cannot block authoritative PTY processing or cause unbounded buffering or frame history | Orbit authoritative owner loop (attachment transport) | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | Fixed protocol and PTY-write bounds, whole-input pressure failure and client closure, resized-state revision retention under full client output, one-read steady-state fairness, terminal-response overflow failure, 4 MiB stopped-reader convergence, bounded silent-peer release, deterministic simultaneous rejection, one-shot process-group cleanup, four-read post-exit work bound, and writing-descendant shutdown checks | The proof is limited to the accepted local one-client boundary |
| `ORB-C8` | On a healthy attachment, Orbit routes each vertical-wheel message from authoritative libghostty state. Active mouse tracking sends only mouse input to the PTY; otherwise an active alternate screen with DEC private mode 1007 sends only terminal-aware cursor input; otherwise Orbit applies one row of native viewport movement. Primary movement clamps within retained history and no-history movement is an accepted no-op. Every host-owned wheel action queues Accepted followed by a strictly newer complete frame of the resulting authoritative viewport. A successful semantic key event that emits PTY bytes returns a scrolled primary viewport to the live area and queues its resulting complete frame. Client disconnection does not transfer, reconstruct, or reset viewport ownership; reattachment begins with one coherent frame of the current Orbit-owned viewport. Rejected or malformed input cannot mutate terminal state, and viewport interaction cannot block authoritative PTY processing. | Orbit authoritative owner loop with libghostty native viewport and canonical semantic input | Proved | `840a67c0cb32b334ed54888321d5ca77e58117b0` | [`authoritative_viewport_routes_wheel_and_key_from_terminal_state`](../src/main.rs) checks native one-row movement and clamps, mouse reporting, DEC 1007 and DECCKM routing, accepted alternate-screen no-op framing, key return-live, empty key input, and PTY-pressure rejection; [`authoritative_viewport_survives_detach_and_slow_reader_pressure`](../tests/lifecycle.rs) checks pinned output, exact coherent reattachment, key return-live, clear/prune/resize/reflow convergence, rapid wheel input, stopped-reader release, and final PTY convergence; canonical Rust and governance verification suite | Selection, copy, search, effects, graphics, and restart persistence remain outside this contract |

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
Venus records the minimum accepted consumer relationship at
`8149002c7275a00db1c08fded171e09f249dd977`. Its current product proof
`8929c9f9d151641a343813ddeb6005cb9c771286` preserves that boundary and the
exact `orbit-protocol` dependency while adding bounded complete-frame
replacement and cheaper ASCII shaping. Against Orbit
`847cab1ca37495c5cd45454623bd81909b488564`, the accepted Linux/Xwayland
workload reached combined CPU p95 167 percent under the predeclared 200 percent
ceiling, preserved coherent resize and final convergence, and reattached to the
surviving PTY. Later boundary changes require an explicit compatible-or-breaking
classification and coordinated update order; a Venus adapter or second decoder
does not substitute for updating the shared owner.

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
and native, web, or mobile clients. Eon owns product workspace topology and
policy. Multiple sessions, windows, tabs, splits, restart recovery, remote
transport, web, mobile, and multiplayer are not initial Orbit contracts and
receive no IDs until the user authorizes their implementation scope. macOS
credibility is an architecture discipline rather than a claim of supported
behavior; actual macOS runtime support also receives no contract ID until the
user authorizes it.
