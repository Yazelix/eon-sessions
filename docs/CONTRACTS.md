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

Commit `2e41997cbec202afc5222f084ffb98973a01b0f0` hardens `ORB-C4` and
`ORB-C6` by rejecting invalid ORBF v1 cell-width topology while preserving
`ORB-C3`, `ORB-C5`, and `ORB-C7` through `ORB-C11`. Focused complete-frame
encode/raw-decode, standalone-row codec, non-mutating reducer, viewport, and
reflow checks plus the canonical Rust, governance, consumer-import, Clippy, and
diff suite passed at that exact revision.
`ORB-C1` and `ORB-C2` remain proved at
`9c0617a97612cdd045ed67b5cd7c87244eb888e6`.

| ID | Contract | Owner | Status | Accepted proof revision | Check or evidence | Gap |
| --- | --- | --- | --- | --- | --- | --- |
| `ORB-C1` | While Orbit remains running, graphical-client disconnection does not kill the PTY or foreground process, and a later client reaches the same live session | Orbit authoritative owner loop with isolated platform PTY lifecycle | Proved | `9c0617a97612cdd045ed67b5cd7c87244eb888e6` | Prior accepted lifecycle evidence; [`transient_pty_eio_recovers_when_the_live_child_reopens_the_terminal`](../tests/lifecycle.rs) checks bounded Linux close/reopen recovery; [`simultaneous_stale_socket_claim_has_one_reachable_owner`](../tests/lifecycle.rs) checks one reachable owner, one bounded explicit loser, and identity-safe cleanup; canonical Rust verification suite | restart persistence remains outside the initial contract |
| `ORB-C2` | Orbit is the sole libghostty terminal-state owner and the only component that sends terminal-generated responses to the PTY | Orbit platform-neutral authoritative owner loop | Proved | `9c0617a97612cdd045ed67b5cd7c87244eb888e6` | [`ansi_palette_is_bounded_and_resets_to_supplied_defaults`](../src/main.rs) checks bounded startup parsing and libghostty-owned default, override, and reset semantics; [`parser_state_and_terminal_replies_survive_client_failure`](../tests/lifecycle.rs) checks retained parser state and ordered terminal replies; [`only_authoritative_terminal_emits_pty_responses`](../tests/formatter_attachment.rs); canonical Rust verification suite | ordered client-visible effects remain a separate unproved transport gap |
| `ORB-C3` | Exactly one local client may attach; a simultaneous second client receives deterministic rejection without disturbing the active client | Orbit authoritative owner loop plus canonical `orbit-protocol` session codec | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) checks exact-version attachment, one-second silent-peer release, typed Busy, abort recovery, canonical ordering, socket identity, and unsupported-version disconnect; canonical Rust verification suite | no known initial-slice local attachment gap |
| `ORB-C4` | Attachment begins with one coherent complete presentation frame at revision N followed by strictly ordered later frame revisions, with no stale final presentation | Orbit authoritative owner loop with synchronized presentation publication plus canonical `orbit-protocol` session and complete-frame codecs | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | Prior accepted attachment and pressure evidence; [`synchronized_presentation_coalesces_defers_and_times_out`](../src/main.rs) checks ordinary, split, one-read, timeout, attachment, and pressure behavior; [`real_pty_synchronized_output_holds_split_large_update`](../src/presentation.rs) checks replies and ordered effects while a split update larger than one PTY read emits exactly one final frame; [`orbf_v1_cell_width_topology_is_validated_at_every_acceptance_boundary`](../crates/protocol/src/lib.rs) rejects malformed topology at complete-frame encode/raw decode, standalone-row encode/decode, and direct reducer admission without state mutation; [`direct_rows_match_rich_canonical_frame_rows`](../src/presentation.rs) checks producer-derived wrapped-wide, viewport-edge, and reflow frames; canonical Rust verification suite | native Wayland and macOS remain unproved Venus platform surfaces |
| `ORB-C5` | Clients send semantic key, mouse, focus, paste, and resize events; Orbit encodes them against authoritative terminal state | Canonical platform-neutral `orbit-protocol` values plus Orbit authoritative input encoding | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | [`authoritative_input_modes_and_resize_survive_detach`](../tests/lifecycle.rs) covers accepted semantic input; [`transient_pty_eio_recovers_when_the_live_child_reopens_the_terminal`](../tests/lifecycle.rs) proves later semantic paste reaches the same authoritative PTY after reopen; canonical Rust verification suite | candidate-list IME and native Wayland quality remain Venus-owned manual surfaces |
| `ORB-C6` | The presentation boundary carries rich terminal state or explicitly declares unsupported capabilities; it never silently collapses the contract to plain text | Orbit authoritative extraction and synchronized publication plus canonical `orbit-protocol` values and codecs | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | Prior accepted palette and rich-terminal evidence; direct adjacent-row checks cover canonical styled graphemes, widths, wrapping, hyperlinks, protection, and background-only cells; [`orbf_v1_cell_width_topology_is_validated_at_every_acceptance_boundary`](../crates/protocol/src/lib.rs) covers canonical width roles and boundary fragments; strict canonical ORBF-in-ORBS codec coverage; canonical Rust verification suite | version 1 still explicitly leaves Kitty graphics unsupported |
| `ORB-C7` | A slow, broken, or disconnected client cannot block authoritative PTY processing or cause unbounded buffering or frame history | Orbit authoritative owner loop (attachment transport) | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | Prior accepted bounded-pressure evidence; [`authoritative_viewport_survives_detach_and_slow_reader_pressure`](../tests/lifecycle.rs) checks atomic frame-result replacement, stopped-reader release, session survival, and reattachment; canonical Rust verification suite | the proof remains limited to the accepted local one-client boundary |
| `ORB-C8` | On a healthy exact-version attachment, a client may preview one vertical direction against the exact current complete-frame revision. Orbit returns terminal-routed with no row when active mouse tracking or alternate-screen DEC private mode 1007 owns the wheel; otherwise it returns the current column count and either one canonical adjacent row or an authoritative-edge result. Preview does not move the viewport, clear selection, send PTY input, or advance the presentation revision. An actual vertical wheel returns one typed terminal-routed outcome after successful PTY admission, or one atomic viewport outcome with applied rows -1, 0, or 1 and a strictly newer complete authoritative frame. Host-owned preview and wheel requests fail before mutation during synchronized output. A successful semantic key event that emits PTY bytes returns a scrolled primary viewport to the live area. Client disconnection does not transfer, reconstruct, or reset viewport ownership; rejected or malformed input cannot mutate terminal state; viewport interaction remains bounded. | Orbit authoritative owner loop with libghostty native viewport, one canonical row extractor, bounded output queue, and canonical ORBS v4 | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | Canonical codec checks cover exact-version framing, preview rows, typed wheel outcomes, malformed values, and bounds; [`authoritative_viewport_routes_wheel_and_key_from_terminal_state`](../src/main.rs) checks no-side-effect previews, stale revisions, routing, exact PTY bytes, one-row clamps, selection policy, synchronized output, and pressure; [`selection_copy_is_authoritative_bounded_and_client_scoped`](../tests/lifecycle.rs) checks preview rows against the next authoritative frames; [`authoritative_viewport_survives_detach_and_slow_reader_pressure`](../tests/lifecycle.rs) checks atomic result replacement and reattachment; canonical Rust and governance verification suite | Venus and Eon/Eonova have not yet adopted the breaking ORBS v4 producer; arbitrary history, pixels, gesture phase, velocity, effects, graphics, and restart persistence remain outside this contract |
| `ORB-C9` | On a healthy ORBS v4 attachment, a client may begin one linear cell selection against the exact current complete-frame revision, update and finish it with ordered current-viewport cells, then request the frozen bounded plain text. Orbit rejects stale revisions, invalid coordinates, invalid phase order, formatting overflow, and pressure without partially changing accepted state or returning partial text. Orbit alone resolves and owns selection, selected presentation, and copied text. A non-selection terminal mutation cancels an active selection and clears selected presentation before mutation; a read-only vertical preview does neither. Once Finish succeeds, later terminal output, resize or reflow, and active-screen transitions cannot reinterpret or erase the immutable frozen candidate. A newer valid Begin, client loss, or exit clears it without resetting the ORB-C8 viewport. Selection and copy work and buffering remain bounded and cannot block authoritative PTY processing or cleanup. | Orbit authoritative owner loop with libghostty current-viewport selection and canonical ORBS v4 | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | `orb-j0m.3` focused codec, owner-state, pressure, and real-PTY lifecycle checks, including frozen-copy retention through alternate-screen output and resize; ORBS v4 preview and committed-wheel selection-policy checks; canonical Rust, governance, Clippy, and consumer-import suites | native clipboard effects, richer selection gestures, search, graphics, and restart persistence remain outside this contract |
| `ORB-C10` | Every Orbit-owned PTY child advertises `TERM=eon` and `COLORTERM=truecolor`; the accepted launch environment supplies the repository-owned compiled `eon` terminfo entry, whose initial capability profile inherits `xterm-256color`, so terminfo lookup and the accepted `clear` workflow succeed | Orbit platform PTY seam and [`terminfo/eon.terminfo`](../terminfo/eon.terminfo); Eon distribution installs the entry | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | `orb-j0m.6` isolated `tic`, `infocmp`, and real-PTY `TERM`, `clear`, and post-clear marker checks; rerun of the same source, lookup, and real-PTY behavior; canonical Rust and governance verification suite; Eon `EON-C3` and `EON-C4` composition proof `eae70e8d3ed4348b389f320dfd49d7db29478546` installs the prior accepted Orbit revision | no known Nix alpha distribution gap; other distribution formats remain outside the accepted scope |
| `ORB-C11` | On a healthy ORBS v4 attachment, each terminal-emitted clipboard write with exactly one non-empty UTF-8 `text/plain` representation is transported once, in order, with its normalized standard, selection, or primary destination before the complete frame produced by the same PTY turn. Orbit rejects invalid UTF-8 or NUL text, clears, unsupported representations, and payloads above the existing 1 MiB copy bound. Detached writes are denied and never retained or replayed; bounded handoff or client-output pressure disconnects the client rather than silently coalescing an admitted effect. Orbit owns terminal interpretation and bounded transport while the client owns native clipboard policy and delivery. | Orbit authoritative owner loop with libghostty normalized clipboard effects and canonical ORBS v4 | Proved | `2e41997cbec202afc5222f084ffb98973a01b0f0` | `orb-ll1.1` focused canonical-codec, callback admission, output-pressure, and real-PTY ordered-effect checks; ORBS v4 ordered-effect and queue replacement checks; canonical Rust, governance, Clippy, and consumer-import suites | Venus native clipboard delivery, ORBS v4 adoption, and Eonova runtime acceptance remain open |

Commit `9d6d2bb37f20ab4ad9e186c7bc715eabef43e757` is the accepted proof for
`ORB-C1` through `ORB-C9`. It preserves ORBF v1 while replacing the runtime
session boundary with ORBS v2. Commit
`292b2451c9a1d99390334771a681c7f481c996f2` proves `ORB-C10`.
Commit `3ee7c80005f3d2bbe81e539799327803716f6174` proves the Orbit-owned
`ORB-C11` producer boundary. Venus native delivery and Eonova acceptance remain
separate consumer proofs.
Commit `f4f0b0a82333d088ad40e2b63108d4905466e8f2` proves the optional
bounded Eon palette launch input for `ORB-C2` and `ORB-C6` while preserving the
existing ORBS revision and standalone defaults.
Commit `9bc87191dd90fd3d7db939127f1ebedfccd2b48d` remains the accepted proof
for `ORB-C1` and `ORB-C2`. Commit
`6de95296d252c119d4fdba2d9b03cec1a09355ae` hardens `ORB-C6` and preserves
`ORB-C3` through `ORB-C5` plus `ORB-C7` through `ORB-C11`; it changes neither
valid ORBF v1 bytes nor ORBS v3. Commit
`cb0703010c3a4944980404b665abff795be09fb4` remains the prior proof for those
routed contracts.

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

Orbit accepts ORBF v1 and the breaking ORBS v2 replacement at
`9d6d2bb37f20ab4ad9e186c7bc715eabef43e757`. Venus revision
`2d36c72dc87ca5416e22d6afdc35c6ab4e2fb832` consumes that exact canonical
package and preserves the native pair accepted by `orb-j0m.4`. Eon composition
proof `eae70e8d3ed4348b389f320dfd49d7db29478546` pins Orbit
`00b136318bea13e3f08d490468f069de6f6b9bd2` and that Venus revision. Later
boundary changes require an explicit compatible-or-breaking classification and
coordinated update order; an adapter or second decoder does not substitute for
updating the shared owner and every known consumer.

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
output queue, and tests moved together. The user selected no adapter,
compatibility window, or support for the removed syntax.

The user accepted `ORB-C9` as an owner-first breaking replacement of ORBS v1
with ORBS v2. Orbit proved v2 first, Venus consumed it through `ven-4sn`, and
`orb-j0m.4` accepted the real pair. There is no dual-version support, adapter,
feature probe, or compatibility window.

The user accepted `ORB-C11` as an owner-first breaking replacement of ORBS v2
with ORBS v3. Orbit proves the producer first in `orb-ll1.1`; a separately
owned Venus consumer must then adopt that exact proof before Eonova can accept
native clipboard delivery. The update adds no dual-version support, adapter,
feature probe, or compatibility window.

The user accepted the `ORB-C8` direct-manipulation boundary as an owner-first
breaking replacement of ORBS v3 with v4. Orbit proves the producer first in
`orb-orbit-typed-wheel-outcomes-5nq`; Venus must then pin that exact proof before
the Eon/Eonova runtime can accept the pair. The replacement has no dual-version
support, adapter, feature probe, or compatibility window. ORBF remains at v1.

The synchronized-output proof at
`26b4b9465b3f0e74f091a0aae93fddc61412b893` is a compatible behavioral
hardening. ORBF v1, ORBS v3, and their canonical package bytes are unchanged,
so existing consumers need no decoder, schema, or compatibility adapter.

## Outside the initial index

Orbit's accepted long-term direction includes host-owned terminal-session state
and native, web, or mobile clients. Eon owns product workspace topology and
policy. Multiple sessions, windows, tabs, splits, restart recovery, remote
transport, web, mobile, and multiplayer are not initial Orbit contracts and
receive no IDs until the user authorizes their implementation scope. macOS
credibility is an architecture discipline rather than a claim of supported
behavior; actual macOS runtime support also receives no contract ID until the
user authorizes it.
