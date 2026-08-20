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

Commit `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` proves `ORB-C12` and
refreshes the preserved `ORB-C1` through `ORB-C11` contracts. The focused and
canonical Rust, governance, consumer-import, Clippy, and diff checks passed at
that exact revision. ORBF v1, ORBS v4, and direct dependencies are unchanged.

Commit `74c3b78de7726239badccd5c9867b85a13209bcf` corrects the `ORB-C4`
real-PTY proof by handing attachment from the non-reading pressure client to a
responsive client before sending semantic interrupt. The proof neither requires
the pressure client to remain writable nor drops interrupt-fairness coverage.
It passed 50 consecutive local runs and the complete hosted CI surface. Runtime
behavior, ORBF v1, ORBS v4, dependencies, and the other contract proofs are
unchanged.

The Session cgroup is a lifecycle scope, not a hostile-code sandbox: a
same-user process can move itself into an external cgroup, at which point it is
outside Orbit's membership result. Cgroup handoff does not detach a process
from its controlling terminal, so ordinary kernel hangup semantics still apply
until the process leaves that terminal session.

| ID | Contract | Owner | Status | Accepted proof revision | Check or evidence | Gap |
| --- | --- | --- | --- | --- | --- | --- |
| `ORB-C1` | While Orbit remains running, graphical-client disconnection does not kill the PTY or foreground process, and a later client reaches the same live session | Orbit authoritative owner loop with isolated platform PTY lifecycle | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | Prior accepted lifecycle evidence; [`transient_pty_eio_recovers_when_the_live_child_reopens_the_terminal`](../tests/lifecycle.rs) checks bounded Linux close/reopen recovery; [`simultaneous_stale_socket_claim_has_one_reachable_owner`](../tests/lifecycle.rs) checks one reachable owner, one bounded explicit loser, and identity-safe cleanup; canonical Rust verification suite | restart persistence remains outside the initial contract |
| `ORB-C2` | Orbit is the sole libghostty terminal-state owner and the only component that sends terminal-generated responses to the PTY | Orbit platform-neutral authoritative owner loop | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | [`ansi_palette_is_bounded_and_resets_to_supplied_defaults`](../src/main.rs) checks bounded startup parsing and libghostty-owned default, override, and reset semantics; [`conformance_c2_parser_state_and_terminal_replies_survive_client_failure`](../tests/lifecycle.rs) checks retained parser state, the exact PTY reply, and absence of a duplicate after client failure; canonical Rust verification suite | ordered client-visible effects remain a separate unproved transport gap |
| `ORB-C3` | Exactly one local client may attach; a simultaneous second client receives deterministic rejection without disturbing the active client | Orbit authoritative owner loop plus canonical `orbit-protocol` session codec | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | [`tests/lifecycle.rs`](../tests/lifecycle.rs) checks exact-version attachment, one-second silent-peer release, typed Busy, abort recovery, canonical ordering, socket identity, and unsupported-version disconnect; canonical Rust verification suite | no known initial-slice local attachment gap |
| `ORB-C4` | Attachment begins with one coherent complete presentation frame at revision N followed by strictly ordered later frame revisions, with no stale final presentation | Orbit authoritative owner loop with synchronized presentation publication plus canonical `orbit-protocol` session and complete-frame codecs | Proved | `74c3b78de7726239badccd5c9867b85a13209bcf` | Prior accepted attachment and pressure evidence; [`synchronized_presentation_coalesces_defers_and_times_out`](../src/main.rs) checks ordinary, split, one-read, timeout, attachment, and pressure behavior; [`real_pty_synchronized_output_holds_split_large_update`](../src/presentation.rs) checks replies and ordered effects while a split update larger than one PTY read emits exactly one final frame; [`orbf_v1_cell_width_topology_is_validated_at_every_acceptance_boundary`](../crates/protocol/src/lib.rs) rejects malformed topology at complete-frame encode/raw decode, standalone-row encode/decode, and direct reducer admission without state mutation; [`conformance_c4_real_pty_reattach_converges_through_complete_ordered_frames`](../src/presentation.rs) checks exact current-frame reattachment, non-reading-client pressure, responsive semantic interrupt, strictly increasing revisions, pending wrap, split parser tails, and final convergence; canonical Rust verification suite | native Wayland and macOS remain unproved Venus platform surfaces |
| `ORB-C5` | Clients send semantic key, mouse, focus, paste, and resize events; Orbit encodes them against authoritative terminal state | Canonical platform-neutral `orbit-protocol` values plus Orbit authoritative input encoding | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | [`conformance_c5_authoritative_input_modes_and_resize_survive_detach`](../tests/lifecycle.rs) covers accepted semantic input; [`transient_pty_eio_recovers_when_the_live_child_reopens_the_terminal`](../tests/lifecycle.rs) proves later semantic paste reaches the same authoritative PTY after reopen; canonical Rust verification suite | candidate-list IME and native Wayland quality remain Venus-owned manual surfaces |
| `ORB-C6` | The presentation boundary carries rich terminal state or explicitly declares unsupported capabilities; it never silently collapses the contract to plain text | Orbit authoritative extraction and synchronized publication plus canonical `orbit-protocol` values and codecs | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | Prior accepted palette and rich-terminal evidence; [`conformance_c6_direct_rows_match_rich_canonical_frame_rows`](../src/presentation.rs) and direct adjacent-row checks cover canonical styled graphemes, widths, wrapping, hyperlinks, protection, and background-only cells; [`orbf_v1_cell_width_topology_is_validated_at_every_acceptance_boundary`](../crates/protocol/src/lib.rs) covers canonical width roles and boundary fragments; strict canonical ORBF-in-ORBS codec coverage; canonical Rust verification suite | version 1 still explicitly leaves Kitty graphics unsupported |
| `ORB-C7` | A slow, broken, or disconnected client cannot block authoritative PTY processing or cause unbounded buffering or frame history | Orbit authoritative owner loop (attachment transport) | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | Prior accepted bounded-pressure evidence; [`authoritative_viewport_survives_detach_and_slow_reader_pressure`](../tests/lifecycle.rs) checks atomic frame-result replacement, stopped-reader release, session survival, and reattachment; canonical Rust verification suite | the proof remains limited to the accepted local one-client boundary |
| `ORB-C8` | On a healthy exact-version attachment, a client may preview one vertical direction against the exact current complete-frame revision. Orbit returns terminal-routed with no row when active mouse tracking or alternate-screen DEC private mode 1007 owns the wheel; otherwise it returns the current column count and either one canonical adjacent row or an authoritative-edge result. Preview does not move the viewport, clear selection, send PTY input, or advance the presentation revision. An actual vertical wheel returns one typed terminal-routed outcome after successful PTY admission, or one atomic viewport outcome with applied rows -1, 0, or 1 and a strictly newer complete authoritative frame. Host-owned preview and wheel requests fail before mutation during synchronized output. A successful semantic key event that emits PTY bytes returns a scrolled primary viewport to the live area. Client disconnection does not transfer, reconstruct, or reset viewport ownership; rejected or malformed input cannot mutate terminal state; viewport interaction remains bounded. | Orbit authoritative owner loop with libghostty native viewport, one canonical row extractor, bounded output queue, and canonical ORBS v4 | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | Canonical codec checks cover exact-version framing, preview rows, typed wheel outcomes, malformed values, and bounds; [`conformance_c8_authoritative_viewport_routes_wheel_and_key_from_terminal_state`](../src/main.rs) checks no-side-effect previews, stale revisions, routing, exact PTY bytes, one-row clamps, selection policy, synchronized output, and pressure; [`conformance_c9_selection_copy_is_authoritative_bounded_and_client_scoped`](../tests/lifecycle.rs) checks preview rows against the next authoritative frames; [`authoritative_viewport_survives_detach_and_slow_reader_pressure`](../tests/lifecycle.rs) checks atomic result replacement and reattachment; canonical Rust and governance verification suite | Venus and Eon/Eonova have not yet adopted the breaking ORBS v4 producer; arbitrary history, pixels, gesture phase, velocity, effects, graphics, and restart persistence remain outside this contract |
| `ORB-C9` | On a healthy ORBS v4 attachment, a client may begin one linear cell selection against the exact current complete-frame revision, update and finish it with ordered current-viewport cells, then request the frozen bounded plain text. Orbit rejects stale revisions, invalid coordinates, invalid phase order, formatting overflow, and pressure without partially changing accepted state or returning partial text. Orbit alone resolves and owns selection, selected presentation, and copied text. A non-selection terminal mutation cancels an active selection and clears selected presentation before mutation; a read-only vertical preview does neither. Once Finish succeeds, later terminal output, resize or reflow, and active-screen transitions cannot reinterpret or erase the immutable frozen candidate. A newer valid Begin, client loss, or exit clears it without resetting the ORB-C8 viewport. Selection and copy work and buffering remain bounded and cannot block authoritative PTY processing or cleanup. | Orbit authoritative owner loop with libghostty current-viewport selection and canonical ORBS v4 | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | [`conformance_c9_selection_copy_is_authoritative_bounded_and_client_scoped`](../tests/lifecycle.rs) checks authoritative selected presentation and frozen Unicode copy through detach, alternate-screen output, and resize; `orb-j0m.3` focused codec, owner-state, pressure, and real-PTY lifecycle checks, including frozen-copy retention through alternate-screen output and resize; ORBS v4 preview and committed-wheel selection-policy checks; canonical Rust, governance, Clippy, and consumer-import suites | native clipboard effects, richer selection gestures, search, graphics, and restart persistence remain outside this contract |
| `ORB-C10` | Every Orbit-owned PTY child advertises `TERM=eon` and `COLORTERM=truecolor`; the accepted launch environment supplies the repository-owned compiled `eon` terminfo entry, whose initial capability profile inherits `xterm-256color`, so terminfo lookup and the accepted `clear` workflow succeed | Orbit platform PTY seam and [`terminfo/eon.terminfo`](../terminfo/eon.terminfo); Eon distribution installs the entry | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | `orb-j0m.6` isolated `tic`, `infocmp`, and real-PTY `TERM`, `clear`, and post-clear marker checks; rerun of the same source, lookup, and real-PTY behavior; canonical Rust and governance verification suite; Eon `EON-C3` and `EON-C4` composition proof `eae70e8d3ed4348b389f320dfd49d7db29478546` installs the prior accepted Orbit revision | no known Nix alpha distribution gap; other distribution formats remain outside the accepted scope |
| `ORB-C11` | On a healthy ORBS v4 attachment, each terminal-emitted clipboard write with exactly one non-empty UTF-8 `text/plain` representation is transported once, in order, with its normalized standard, selection, or primary destination before the complete frame produced by the same PTY turn. Orbit rejects invalid UTF-8 or NUL text, clears, unsupported representations, and payloads above the existing 1 MiB copy bound. Detached writes are denied and never retained or replayed; bounded handoff or client-output pressure disconnects the client rather than silently coalescing an admitted effect. Orbit owns terminal interpretation and bounded transport while the client owns native clipboard policy and delivery. | Orbit authoritative owner loop with libghostty normalized clipboard effects and canonical ORBS v4 | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | `orb-ll1.1` focused canonical-codec, callback admission, output-pressure, and real-PTY ordered-effect checks; ORBS v4 ordered-effect and queue replacement checks; canonical Rust, governance, Clippy, and consumer-import suites | Venus and Eon/Eonova still require ORBS v4 adoption. Accepted v3 consumer evidence covers native x86_64 Linux Wayland primary-selection delivery; ordinary clipboard delivery, Wayland without data-control, broader compositors, and macOS policy remain unproved. |
| `ORB-C12` | After Orbit has launched one PTY Session, an accepted SIGINT, SIGTERM, or SIGHUP succeeds only after the direct PTY child is reaped and every process still in that Session's exact Linux containment is gone. While the direct child remains unreaped, shutdown sends SIGHUP only to that child by positive PID and gives the exact containment a fixed 500 ms to empty. Orbit never targets a numeric process group during shutdown. It then kills any remaining contained processes, requires `cgroup.events` to report `populated 0`, and removes only the verified owned containment. Distinct job-control groups, `setsid` descendants, daemon-style forks, and concurrent forks remain owned while contained; `nohup`, `disown`, and `setsid` alone do not transfer ownership. A process launched or migrated into another lifecycle manager's scope is outside Orbit's forced-cleanup membership. Orbit does not explicitly signal it through a retained process-group relationship, but ordinary kernel terminal hangup may still signal it while it remains in the PTY foreground group; full signal isolation requires terminal-session detachment. Client disconnection remains the non-destructive `ORB-C1` detach path. The containment is not a security boundary against deliberate same-user membership manipulation. Orbit fails before PTY exec if containment cannot be established and returns bounded non-success if identity, escalation, reaping, empty verification, or cleanup fails. | Linux cgroup-v2 membership owned by the kernel; mechanics isolated in Orbit's platform seam; direct-child signaling and reaping, deadlines, escalation, verification, and exit truth owned by the Orbit owner loop | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | [`shutdown_hups_the_unreaped_direct_child`](../tests/lifecycle.rs) checks positive-PID graceful delivery and exact cleanup; [`signal_shutdown_empties_the_owned_session_containment`](../tests/lifecycle.rs) checks initial, foreground, distinct-background, `setsid`, and concurrent-fork shapes against an unrelated sibling; [`explicit_handoff_leaves_process_outside_session_shutdown_scope`](../tests/lifecycle.rs) checks a same-initial-process-group descendant transferred into a separate exact scope is not explicitly group-signaled by Orbit while a different contained group owns the foreground, survives Orbit stop, and is cleaned through that scope; [`writing_descendant_cannot_hold_server_open_after_known_child_exit`](../tests/lifecycle.rs) checks exact containment cleanup after the direct child has already been reaped; [`stale_containment_fails_before_the_pty_child_starts`](../tests/lifecycle.rs) checks fail-before-exec behavior; [`widened_containment_is_explicit_shutdown_failure`](../tests/lifecycle.rs) checks private-mode revalidation before destructive cleanup; focused lifecycle suite | Eon handoff remains pending; macOS has no accepted lifecycle-scope replacement |
| `ORB-C13` | After Orbit has atomically published Ready, abrupt loss of its Eon supervisor on the same boot and login does not stop or hold open that exact Orbit run. One replacement local Eon may acquire the sole private management lease only after a live handshake matches the bounded logical Session ID and fresh run ID, exact record, component and management generations, Orbit process ID and start identity, both endpoint identities, and the expected peer UID. Files and process metadata locate or corroborate a candidate but never authorize attach or stop. Lease disconnect before stop is non-destructive; two replacements produce one winner and one Busy loser; accepted stop is not cancelled by disconnect. Management carries only bounded identity, live status, and explicit stop, with one request and response in flight; canonical ORBS remains a separate one-presentation-client boundary and retains `ORB-C1` through `ORB-C11`. Explicit stop succeeds only through the validated lease after `ORB-C12` succeeds; validation or transport failure authorizes no PID, process-group, process-name, pathname, cgroup, or pidfd fallback. Natural child exit and successful explicit stop atomically replace the live record with one non-authoritative terminal tombstone containing termination reason and the exact exit-code-or-signal outcome, then remove only exact endpoints. Eon bounds start and recovery to five seconds; Orbit bounds an incomplete handshake to one second, each management message and record to 4 KiB, each identity to 128 UTF-8 bytes, and failure detail to 1 KiB. Runtime directories are exact UID-owned 0700 directories and sockets and regular records are exact UID-owned 0600 objects; symlinks, unexpected types, wrong ownership or mode, replacement, overflow, malformed or incompatible state, and slow peers fail closed without blocking PTY, presentation, child observation, or shutdown. The contract is per live Orbit run: Eon retains topology, enumeration, cleanup, and recovery policy; no workspace topology, retained log, Orbit or machine restart, logout or reboot survival, multiplayer, remote access, public protocol, service-manager requirement, or same-UID sandbox is promised. Platform-neutral identity, lease, status, and failure values use isolated Linux spawn/stdio, peer-credential, process/start-identity, endpoint, permission, and polling mechanics; Linux is first and macOS remains unproved. | Orbit authoritative owner loop plus the existing canonical protocol package for the private bounded management values; Linux mechanics isolated in Orbit's platform seam; Eon remains the later consumer and policy owner | Proved | `86aa130629c09dce61d0f232150298656fa5cef4` | `orb-implement-orbit-session-management-owner-91e` owner proof: canonical codec and Linux identity/artifact negatives; launcher-loss continuity with the same Orbit, PTY child, and newer canonical ORBS state; one-winner lease race; non-cancellable owner-routed `ORB-C12` stop after manager disconnect; typed tombstones; exact endpoint and containment cleanup; complete workspace Rust, governance, Clippy, dependency, and diff checks. Eon acceptance `0bf0b165d06b4a8162be497011070f61f6c2000a` composes exact Orbit `86aa130629c09dce61d0f232150298656fa5cef4` through three full Eon Sessions and one EonTerm Session, supervisor loss, diagnostic reattachment, contending replacements, non-reusing creation, and complete owner-routed Stop on x86_64 Linux. | macOS has no accepted implementation or proof; Eonova rollout remains separate |

## Focused terminal conformance corpus

The accepted `orb-8s0` corpus groups six existing authoritative tests under one
offline, headless command without copying their stimuli or expectations:

```sh
cargo test --locked conformance_
```

The command runs the pinned `libghostty-vt` 0.2.1 path backed by Ghostty
`a887df42c56f6de86c0fe6da9c4eeca37931e083`. Protocol-only checks remain in
the normal suite because they do not exercise that terminal engine.

| Case | Stimulus | Orbit-visible observation | Assertion policy | Owner |
| --- | --- | --- | --- | --- |
| `conformance_c2_parser_state_and_terminal_replies_survive_client_failure` | Split DEC 1049 input across client loss, then cursor visibility and position query | The authoritative parser completes the alternate-screen transition, emits one PTY response, and later restores primary | Exact reply bytes and single emission; semantic screen and cursor state | `ORB-C2` |
| `conformance_c4_real_pty_reattach_converges_through_complete_ordered_frames` | Primary pending wrap, rich alternate screen, resize, detach under continuous output, responsive semantic interrupt, then split CSI, UTF-8, and APC tails | Reattachment starts at the complete current frame; later revisions strictly increase and converge on the final primary state | Exact reattached-frame equality and revision order; semantic final convergence | `ORB-C4` |
| `conformance_c5_authoritative_input_modes_and_resize_survive_detach` | Resize plus key, mouse, focus, paste, Kitty-keyboard, mode change, and reattach | The PTY receives terminal-state-aware input once and retains the requested dimensions | Exact PTY bytes and `stty` dimensions | `ORB-C5` |
| `conformance_c6_direct_rows_match_rich_canonical_frame_rows` | Wrapped styled text, RGB and palette colors, hyperlink, protection, combining and wide graphemes, viewport movement, and resize | Direct row extraction equals the corresponding canonical frame rows before and after reflow | Exact canonical-row equality; semantic presence of accepted rich fields and width roles | `ORB-C6` |
| `conformance_c8_authoritative_viewport_routes_wheel_and_key_from_terminal_state` | Preview and wheel at history edges, mouse tracking, alternate screen, synchronized output, semantic key, and pressure | Orbit returns typed terminal or viewport outcomes without forbidden mutation and routes exact terminal input when required | Exact typed outcomes, revisions, and PTY bytes; semantic no-mutation and boundedness | `ORB-C8` |
| `conformance_c9_selection_copy_is_authoritative_bounded_and_client_scoped` | Wide and combining text, viewport movement, reversed selection, detach, alternate-screen output, and resize | Selected presentation is client-scoped while finished copy stays frozen across later terminal mutations | Exact copied Unicode text; semantic selection visibility, scope, and freeze | `ORB-C9` |

A mismatch is classified from the smallest reproducer: an Orbit regression if
the adapter or ownership path changed, an upstream behavior change or defect if
only the pinned engine changed, an unsupported candidate capability in a
separately authorized engine spike, or a corpus expectation error when the
assertion exceeds the indexed contract. A mismatch does not broaden product
compatibility; product fixes and upstream submissions require their own
authorized work.

The inventory is the minimum test-only boundary a future candidate-engine spike
may consume: stimulus, Orbit-visible observation, and assertion policy. It does
not authorize a shared runtime trait, production adapter, second engine, public
trace, golden corpus, or dependency. External VT and Unicode certification
remains owned by `orb-vt-unicode-baselines-i32`; performance remains owned by
`orb-1sd`.

Commit `9d6d2bb37f20ab4ad9e186c7bc715eabef43e757` is the accepted proof for
`ORB-C1` through `ORB-C9`. It preserves ORBF v1 while replacing the runtime
session boundary with ORBS v2. Commit
`292b2451c9a1d99390334771a681c7f481c996f2` proves `ORB-C10`.
Commit `3ee7c80005f3d2bbe81e539799327803716f6174` proves the Orbit-owned
`ORB-C11` producer boundary. Venus consumes it at source proof
`2d3498258920736eb1bdae2b8869b6547b9735d4`; Venus metadata
`4f799fef1c3e10e86d70b708708f7c2702d7aee8` records Eon
`0e25ebc2311d7e41edf90c940f8211dd5839bb83` and Eonova
`4fda9b67b0faa33561624633229135e5e2d579ea` as separate acceptance evidence.
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
with ORBS v3. Orbit proved the producer first in `orb-ll1.1`; Venus source proof
`2d3498258920736eb1bdae2b8869b6547b9735d4` adopted that exact package, Eon
`0e25ebc2311d7e41edf90c940f8211dd5839bb83` packaged the pair, and Eonova
`4fda9b67b0faa33561624633229135e5e2d579ea` accepted native primary-selection
delivery. Orbit and its consumers use no dual-version support, adapter, feature
probe, or compatibility window. Unproved native destinations and platforms
remain gaps.

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
