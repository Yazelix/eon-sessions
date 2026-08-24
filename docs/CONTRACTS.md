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

Candidate `orb-orbit-signed-scroll-batch-x12` advances exact ORBS v5 to v6.
One revision-bound request commits up to 1,024 signed whole rows in one native
viewport operation and returns requested/applied rows, one newer complete
frame, and the next adjacent row or edge. Routing changes return terminal-owned
without PTY input or viewport mutation. `ORB-C1` through `ORB-C9` and `ORB-C11`
remain partially proved until an immutable candidate passes their canonical
checks; `ORB-C10`, `ORB-C12`, `ORB-C13`, ORBF v1, management v1, and direct
dependencies are unchanged.

Commit `e86a036a4481ea2be55012a38c13f75204b81278` extends `ORB-C8` with a
16 MiB primary-history byte budget. Fixed 120×40 measurements retained 15,704
plain, 15,704 styled, and 14,930 Unicode rows while confirming lazy residency;
the canonical Rust, governance, consumer-import, Clippy, and diff checks passed.
ORBF v1, ORBS v5, direct dependencies, and the other contracts are unchanged.

Commit `7de9980ffbd6417758698d5c33356204eeb24de5` strengthens the detached-survivor
proof and simplifies the accepted `ORB-C12` shutdown owner without changing
behavior. Canonical Rust, governance, consumer-import, Clippy, diff, and Beads
checks passed. `ORB-C13`, ORBF, ORBS, and direct dependencies are unchanged.

Commit `69c402737799f03e615473956954a043647a4713` changes `ORB-C3` and
refreshes preserved `ORB-C1`, `ORB-C2`, and `ORB-C4` through `ORB-C13` under
exact ORBS v5. One input-capable attachment and one bounded read-only title/CWD
observer may coexist on the private presentation endpoint. Focused codec,
attachment, split/rapid OSC, slow-observer, lifecycle, and full canonical Rust,
governance, consumer-import, Clippy, and diff checks passed at that exact
revision. ORBF v1, management v1, and direct dependencies are unchanged.

Commit `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` is the historical proof of
the retired cgroup-backed `ORB-C12` behavior. It remains evidence for why that
mechanism was stronger, not a proof of the replacement contract.

Commit `74c3b78de7726239badccd5c9867b85a13209bcf` corrects the `ORB-C4`
real-PTY proof by handing attachment from the non-reading pressure client to a
responsive client before sending semantic interrupt. The proof neither requires
the pressure client to remain writable nor drops interrupt-fairness coverage.
It passed 50 consecutive local runs and the complete hosted CI surface. Runtime
behavior, ORBF v1, ORBS v4, dependencies, and the other contract proofs are
unchanged.

Commit `fd47f8641482a37525702ab676d68c0395e72720` preserves `ORB-C3`,
`ORB-C4`, `ORB-C7`, and `ORB-C11` while centralizing attachment client defaults
after the initial extraction at `6fcf98228f9e5e6430390fe1da1c0e4bace01aed`.
Focused transport checks and the complete canonical suite passed at the
correction revision; protocols and dependencies are unchanged.

Commit `6ddb89974dd3d5af48349d169e2ed4081e56564e` moves the unchanged ORBS v4
public values, codec mechanics, and canonical tests into
[`session/mod.rs`](../crates/protocol/src/session/mod.rs),
[`session/codec.rs`](../crates/protocol/src/session/codec.rs), and
[`session/tests.rs`](../crates/protocol/src/session/tests.rs), preserving
`ORB-C3` through `ORB-C9` and `ORB-C11`. Their listed revisions remain the
runtime proofs; the source-organization proof is the split commit. `ORB-C10`
and the other contracts are unaffected.

| ID | Contract | Owner | Status | Accepted proof revision | Check or evidence | Gap |
| --- | --- | --- | --- | --- | --- | --- |
| `ORB-C1` | While Orbit remains running, graphical-client disconnection does not kill the PTY or foreground process, and a later client reaches the same live session | Orbit concrete runtime coordinator with isolated platform PTY lifecycle | Partially proved | `d96fff2015cded947c881c454216c6bf0ad69b7b` | Accepted [`src/runtime.rs`](../src/runtime.rs) preserves the concrete single-threaded lifecycle and detach composition; prior accepted lifecycle evidence; [`transient_pty_eio_recovers_when_the_live_child_reopens_the_terminal`](../tests/lifecycle.rs) checks bounded Linux close/reopen recovery; [`simultaneous_stale_socket_claim_has_one_reachable_owner`](../tests/lifecycle.rs) checks one reachable owner, one bounded explicit loser, and identity-safe cleanup; canonical Rust verification suite | Restart persistence remains outside the initial contract |
| `ORB-C2` | Orbit is the sole libghostty terminal-state owner and the only component that sends terminal-generated responses to the PTY | Orbit platform-neutral concrete runtime coordinator | Partially proved | `d96fff2015cded947c881c454216c6bf0ad69b7b` | Accepted [`src/runtime.rs`](../src/runtime.rs) keeps the sole terminal, response callback, and bounded PTY queue together; accepted [`src/interaction.rs`](../src/interaction.rs) preserves selection invalidation before authoritative VT writes; [`ansi_palette_is_bounded_and_resets_to_supplied_defaults`](../src/main.rs) checks bounded startup parsing and libghostty-owned default, override, and reset semantics; [`conformance_c2_parser_state_and_terminal_replies_survive_client_failure`](../tests/lifecycle.rs) checks retained parser state, the exact PTY reply, and absence of a duplicate after client failure; canonical Rust verification suite | Ordered client-visible effects remain a separate unproved transport gap |
| `ORB-C3` | Exactly one local input-capable client and one bounded read-only metadata observer may attach concurrently through explicit roles on the private presentation endpoint. The observer receives only initial and changed terminal-authored title, raw working directory, exact presentation revision, and lifecycle; it cannot send terminal, presentation, process, or lifecycle mutations. A simultaneous excess role, incompatible or malformed peer, slow reader, disconnect, and exit fail deterministically without disturbing the PTY, child, other role, or cleanup. | Orbit attachment transport and concrete runtime coordinator plus canonical `orbit-protocol` ORBS v6 session codec | Partially proved | `69c402737799f03e615473956954a043647a4713` | [`src/attachment.rs`](../src/attachment.rs) owns role negotiation, one observer, role rejection, ordered initial metadata, latest-value replacement, framed reads, and disconnect; [`src/runtime.rs`](../src/runtime.rs) reads libghostty title/CWD in the sole owner loop and publishes only complete changed values; [`metadata_observer_is_read_only_and_keeps_only_the_latest_change`](../src/attachment.rs) checks role immutability and bounded replacement; [`metadata_observer_streams_inactive_title_and_cwd_without_owning_input`](../tests/lifecycle.rs) checks concurrent interactive use, exact initial state, observer-input rejection and recovery, excess `Busy`, split and rapid OSC title/CWD, slow-reader pressure, child exit, and socket cleanup; prior one-second negotiation, abort, ordering, identity, and unsupported-version checks plus the canonical Rust verification suite | Venus and Eon have not yet adopted the exact ORBS v6 proof; visible text, agent observation, multiple observers, multiple interactive clients, remote access, and process authority remain outside this contract |
| `ORB-C4` | Attachment begins with one coherent complete presentation frame at revision N followed by strictly ordered later frame revisions, with no stale final presentation | Orbit attachment transport with synchronized presentation publication coordinated by the concrete runtime plus canonical `orbit-protocol` session and complete-frame codecs | Partially proved | `d96fff2015cded947c881c454216c6bf0ad69b7b` | Accepted [`src/runtime.rs`](../src/runtime.rs) keeps synchronized publication in the unchanged owner-loop order; accepted [`src/interaction.rs`](../src/interaction.rs) preserves post-negotiation revision and publication ordering; [`src/attachment.rs`](../src/attachment.rs) owns initial-frame retention and latest-frame replacement; prior accepted attachment and pressure evidence; [`synchronized_presentation_coalesces_defers_and_times_out`](../src/runtime.rs) checks ordinary, split, one-read, timeout, attachment, and pressure behavior; [`real_pty_synchronized_output_holds_split_large_update`](../src/presentation.rs) checks replies and ordered effects while a split update larger than one PTY read emits exactly one final frame; [`orbf_v1_cell_width_topology_is_validated_at_every_acceptance_boundary`](../crates/protocol/src/lib.rs) rejects malformed topology at complete-frame encode/raw decode, standalone-row encode/decode, and direct reducer admission without state mutation; [`conformance_c4_real_pty_reattach_converges_through_complete_ordered_frames`](../src/presentation.rs) checks exact current-frame reattachment, non-reading-client pressure, responsive semantic interrupt, strictly increasing revisions, pending wrap, split parser tails, and final convergence; canonical Rust verification suite | Native Wayland and macOS remain unproved Venus platform surfaces |
| `ORB-C5` | Clients send semantic key, mouse, focus, paste, and resize events; Orbit encodes them against authoritative terminal state | Canonical platform-neutral `orbit-protocol` values plus the Orbit semantic interaction owner | Partially proved | `b2fbfe1a718b77dbd37d9c82370bec83584384ae` | Accepted [`kitty_release_never_falls_back_to_text`](../src/interaction.rs) prevents a text-bearing Space release from becoming a second insertion under Kitty reporting; actual Codex CLI 0.149.0 dogfood receives one literal Space and one release sequence per physical press; prior accepted ORB-C5 lifecycle checks and the canonical Rust verification suite remain green | Candidate-list IME and native Wayland quality remain Venus-owned manual surfaces |
| `ORB-C6` | The presentation boundary carries rich terminal state or explicitly declares unsupported capabilities; it never silently collapses the contract to plain text | Orbit authoritative extraction and synchronized publication plus canonical `orbit-protocol` values and codecs | Partially proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | Prior accepted palette and rich-terminal evidence; [`conformance_c6_direct_rows_match_rich_canonical_frame_rows`](../src/presentation.rs) and direct adjacent-row checks cover canonical styled graphemes, widths, wrapping, hyperlinks, protection, and background-only cells; [`orbf_v1_cell_width_topology_is_validated_at_every_acceptance_boundary`](../crates/protocol/src/lib.rs) covers canonical width roles and boundary fragments; strict canonical ORBF-in-ORBS codec coverage; canonical Rust verification suite | version 1 still explicitly leaves Kitty graphics unsupported |
| `ORB-C7` | A slow, broken, or disconnected attachment or metadata observer cannot block authoritative PTY processing, the other role, cleanup, or cause unbounded buffering or history | Orbit attachment transport in the concrete runtime coordinator | Partially proved | `69c402737799f03e615473956954a043647a4713` | Accepted [`src/runtime.rs`](../src/runtime.rs) preserves nonblocking poll, flush, disconnect, and PTY-pressure ordering; accepted [`src/interaction.rs`](../src/interaction.rs) preserves whole-input and result admission before mutation while [`src/attachment.rs`](../src/attachment.rs) owns the bounded output queue, replaceable suffix, nonblocking flush, and close lifecycle; prior accepted bounded-pressure evidence; [`authoritative_viewport_survives_detach_and_slow_reader_pressure`](../tests/lifecycle.rs) checks atomic frame-result replacement, stopped-reader release, session survival, and reattachment; [`metadata_observer_streams_inactive_title_and_cwd_without_owning_input`](../tests/lifecycle.rs) checks a slow observer under sustained metadata pressure while the interactive role drains; canonical Rust verification suite | The proof remains limited to the accepted local one-input-client plus one-observer boundary |
| `ORB-C8` | Each Orbit Session configures libghostty with a 16 MiB primary-history byte budget; retained row count is content-dependent and libghostty may exceed the budget to preserve the active viewport. On a healthy exact-version attachment, a client may preview one vertical direction against the exact current complete-frame revision without mutation. After that preview, the client may commit a nonzero signed distance from -1,024 through 1,024 rows against the same revision: negative moves toward older history and positive moves toward the live area. If active mouse tracking or alternate-screen DEC private mode 1007 has taken ownership, Orbit returns terminal-owned without PTY input, viewport mutation, or revision advance. Otherwise Orbit applies the distance once, clears an active selection, advances presentation authority once, and returns requested and actual applied rows, one complete authoritative frame, and the next canonical adjacent row or edge in the requested direction. Movement clamps at retained-history edges. Stale authority, synchronized output, malformed or out-of-range distance, and output pressure fail before mutation. Existing physical vertical wheels retain their typed terminal-routed or one-row viewport behavior, and a successful semantic key that emits PTY bytes returns primary history to the live area. Client disconnection does not transfer, reconstruct, or reset viewport ownership; viewport interaction remains bounded. | Orbit semantic interaction owner with libghostty native viewport and byte-budgeted history, one canonical row extractor, bounded output queue, and canonical ORBS v6 | Partially proved | `e86a036a4481ea2be55012a38c13f75204b81278` | The accepted revision remains proof for the 16 MiB budget and prior viewport behavior. Candidate [`vertical_scroll_batches_are_bounded_and_canonical`](../crates/protocol/src/session/tests.rs) covers exact signed bounds, malformed values, atomic typed results, and next-preview shape; [`conformance_c8_signed_scroll_batch_is_atomic_bounded_and_authoritative`](../src/interaction.rs) covers one-message movement, both edges, stale authority, selection, routing changes, synchronization, and pressure; [`authoritative_viewport_survives_detach_and_slow_reader_pressure`](../tests/lifecycle.rs) commits six rows in one request before detach and exact reattachment. | Candidate acceptance, exact proof revision, and Venus consumer adoption remain pending; arbitrary line-count guarantees, pixels, gesture phase, velocity, effects, graphics, and restart persistence remain outside this contract |
| `ORB-C9` | On a healthy ORBS v6 attachment, a client may begin one linear cell selection against the exact current complete-frame revision, update and finish it with ordered current-viewport cells, then request the frozen bounded plain text. Orbit rejects stale revisions, invalid coordinates, invalid phase order, formatting overflow, and pressure without partially changing accepted state or returning partial text. Orbit alone resolves and owns selection, selected presentation, and copied text. A non-selection terminal mutation cancels an active selection and clears selected presentation before mutation; a read-only vertical preview does neither. Once Finish succeeds, later terminal output, resize or reflow, and active-screen transitions cannot reinterpret or erase the immutable frozen candidate. A newer valid Begin, client loss, or exit clears it without resetting the ORB-C8 viewport. Selection and copy work and buffering remain bounded and cannot block authoritative PTY processing or cleanup. | Orbit semantic interaction owner with libghostty current-viewport selection and canonical ORBS v6 | Partially proved | `413809cd6ac34e70bb3bf051d7a2a8d7da1aa357` | Accepted [`authoritative_selection_rejects_stale_input_and_freezes_copy`](../src/interaction.rs) checks owner-state failures, mutation ordering, pressure, and frozen copy; [`conformance_c9_selection_copy_is_authoritative_bounded_and_client_scoped`](../tests/lifecycle.rs) checks authoritative selected presentation and frozen Unicode copy through detach, alternate-screen output, and resize; `orb-j0m.3` focused codec, owner-state, pressure, and real-PTY lifecycle checks, including frozen-copy retention through alternate-screen output and resize; ORBS v4 preview and committed-wheel selection-policy checks; canonical Rust, governance, Clippy, and consumer-import suites | Native clipboard effects, richer selection gestures, search, graphics, and restart persistence remain outside this contract |
| `ORB-C10` | Every Orbit-owned PTY child advertises `TERM=eon` and `COLORTERM=truecolor`; the accepted launch environment supplies the repository-owned compiled `eon` terminfo entry, whose initial capability profile inherits `xterm-256color`, so terminfo lookup and the accepted `clear` workflow succeed | Orbit platform PTY seam and [`terminfo/eon.terminfo`](../terminfo/eon.terminfo); Eon distribution installs the entry | Proved | `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` | `orb-j0m.6` isolated `tic`, `infocmp`, and real-PTY `TERM`, `clear`, and post-clear marker checks; rerun of the same source, lookup, and real-PTY behavior; canonical Rust and governance verification suite; Eon `EON-C3` and `EON-C4` composition proof `eae70e8d3ed4348b389f320dfd49d7db29478546` installs the prior accepted Orbit revision | no known Nix alpha distribution gap; other distribution formats remain outside the accepted scope |
| `ORB-C11` | On a healthy ORBS v6 attachment, each terminal-emitted clipboard write with exactly one non-empty UTF-8 `text/plain` representation is transported once, in order, with its normalized standard, selection, or primary destination before the complete frame produced by the same PTY turn. Orbit rejects invalid UTF-8 or NUL text, clears, unsupported representations, and payloads above the existing 1 MiB copy bound. Detached writes are denied and never retained or replayed; bounded handoff or client-output pressure disconnects the client rather than silently coalescing an admitted effect. Orbit owns terminal interpretation and bounded transport while the client owns native clipboard policy and delivery. | Orbit attachment transport with concrete runtime clipboard interpretation, libghostty normalized effects, and canonical ORBS v6 | Partially proved | `d96fff2015cded947c881c454216c6bf0ad69b7b` | [`src/attachment.rs`](../src/attachment.rs) owns ordered bounded output and pressure disconnect while accepted [`src/runtime.rs`](../src/runtime.rs) retains clipboard interpretation and effect-before-frame publication; `orb-ll1.1` focused canonical-codec, callback admission, output-pressure, and real-PTY ordered-effect checks; ORBS v4 ordered-effect and queue replacement checks; canonical Rust, governance, Clippy, and consumer-import suites | Accepted consumer evidence covers native x86_64 Linux Wayland primary-selection delivery; ordinary clipboard delivery, Wayland without data-control, broader compositors, and macOS policy remain unproved. |
| `ORB-C12` | After Orbit has launched one PTY Session, an accepted SIGINT, SIGTERM, SIGHUP, or management Stop succeeds only after Orbit has reaped its direct PTY child. If the child has already exited, Orbit reaps it without signaling its recyclable numeric identity. Otherwise shutdown sends SIGHUP to the initial PTY process group and, when distinct and available, the terminal's current foreground process group, then gives the direct child up to 500 ms to exit. If it remains unreaped, Orbit sends SIGKILL to its still-stable initial group and re-reads the terminal's foreground group before escalating it. If the child exits during the grace period, Orbit sends no later signal to the cached foreground-group number, so a SIGHUP-ignoring foreground job may survive. Orbit then requires the direct child to be reaped within two seconds. A signal target that is already gone is benign; other signaling or reaping failures are bounded non-success. Client disconnection remains the non-destructive `ORB-C1` detach path. Deliberately detached processes, non-foreground groups outside the initial group, and SIGHUP-ignoring foreground jobs whose shell exits during the grace period may survive. Orbit does not scan process names, ancestry, session IDs, or procfs and does not claim whole-descendant cleanup. | Orbit's concrete runtime coordinator owns graceful timing, escalation, direct-child reaping, and result truth; Linux PTY and process-group mechanics stay isolated in the platform seam | Proved | `7de9980ffbd6417758698d5c33356204eeb24de5` | [`shutdown_hups_the_unreaped_direct_child`](../tests/lifecycle.rs) checks graceful direct-child HUP; [`shutdown_escalates_foreground_and_permits_a_detached_process`](../tests/lifecycle.rs) checks forced foreground escalation plus exact ordinary-disposition detached and unrelated survivors; [`stale_socket_and_safe_signal_shutdown`](../tests/lifecycle.rs) checks no delayed forced foreground signal after shell exit; [`writing_descendant_cannot_hold_server_open_after_known_child_exit`](../tests/lifecycle.rs) checks bounded server exit after the direct child exits; canonical Rust, governance, consumer-import, Clippy, and diff checks | Eon consumer adoption remains pending; macOS remains unproved |
| `ORB-C13` | A launcher may create and retain an empty exact owned mode-0600 record inode before spawning Orbit. Orbit serializes the Ready boundary by attempting one exclusive lock. A held claim stops startup without marking or publishing; after winning, Orbit revalidates and marks the inode before atomically replacing its pathname with the canonical live record. Only a launcher that wins the same lock while the retained inode is still empty may use pre-Ready local-child rollback authority. After Orbit has atomically published Ready, abrupt loss of its Eon supervisor on the same boot and login does not stop or hold open that exact Orbit run. One replacement local Eon may acquire the sole private management lease only after a live handshake matches the bounded logical Session ID and fresh run ID, exact record, component and management generations, Orbit process ID and start identity, both endpoint identities, and the expected peer UID. Files and process metadata locate or corroborate a candidate but never authorize attach or stop. Lease disconnect before stop is non-destructive; two replacements produce one winner and one Busy loser; accepted stop is not cancelled by disconnect. Management carries only bounded identity, live status, and explicit stop, with one request and response in flight; canonical ORBS remains a separate one-input-capable-client plus one-metadata-observer boundary and retains `ORB-C1` through `ORB-C11`. Explicit stop succeeds only through the validated lease after `ORB-C12` succeeds; validation or transport failure authorizes no PID, process-group, process-name, pathname, procfs-scan, or pidfd fallback. Natural child exit and successful explicit stop atomically replace the live record with one non-authoritative terminal tombstone containing termination reason and the exact exit-code-or-signal outcome, then remove only exact endpoints. Eon bounds start and recovery to five seconds; Orbit bounds an incomplete handshake to one second, each management message and record to 4 KiB, each identity to 128 UTF-8 bytes, and failure detail to 1 KiB. Runtime directories are exact UID-owned 0700 directories and sockets and regular records are exact UID-owned 0600 objects; symlinks, unexpected types, wrong ownership or mode, replacement, overflow, malformed or incompatible state, and slow peers fail closed without blocking PTY, presentation, child observation, or shutdown. The contract is per live Orbit run: Eon retains topology, enumeration, cleanup, and recovery policy; no workspace topology, retained log, Orbit or machine restart, logout or reboot survival, multiplayer, remote access, public protocol, service-manager requirement, or same-UID sandbox is promised. Platform-neutral identity, lease, status, and failure values use isolated Linux spawn/stdio, peer-credential, process/start-identity, endpoint, permission, and polling mechanics; Linux is first and macOS remains unproved. | Orbit private management owner plus the existing canonical protocol package for the private bounded management values; Linux mechanics isolated in Orbit's platform seam; Eon remains the consumer and policy owner | Proved | `7de9980ffbd6417758698d5c33356204eeb24de5` | Accepted management proof `3186519af709a94005974a97212814c05715a99d` remains valid; [`managed_run_survives_launcher_loss_and_has_one_replacement_owner`](../tests/lifecycle.rs) and [`management_authority_negatives_fail_closed_without_stopping_session`](../tests/lifecycle.rs) preserve owner-routed Stop authorization, lease, failure, tombstone, and recovery ordering against the corrected `ORB-C12`; canonical Rust, governance, consumer-import, Clippy, and diff checks | Eon consumer adoption remains pending; macOS remains unproved. |

## Focused terminal conformance corpus

The current corpus groups six accepted `orb-8s0` tests and the candidate x12
test under one offline, headless command without copying their stimuli or
expectations:

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
| `conformance_c8_signed_scroll_batch_is_atomic_bounded_and_authoritative` | One large signed commit, both history edges, stale authority, routing changes, synchronized output, selection, and pressure | Orbit performs one native viewport movement and returns one result with exact requested/applied rows, one frame, and the next preview | Exact result count, rows, revision, adjacent preview, PTY absence, and failure non-mutation | `ORB-C8` |
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
`orb-orbit-typed-wheel-outcomes-5nq`; Venus source
`a768e9a1bcb61eac5a21d25b7463c9dc44aa2df8` consumes exact Orbit proof
`7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c`, Eon acceptance
`0bf0b165d06b4a8162be497011070f61f6c2000a` composes exact Orbit source
`86aa130629c09dce61d0f232150298656fa5cef4` and that Venus source, and Eonova
source `d8a8729f442f3d535b30fd22ac8dc7b6da4626dd` pins exact Eon
`ced9e4ae11ed21a0f05d50cd470491adffa73b54`. The replacement has no
dual-version support, adapter, feature probe, or compatibility window. ORBF
remains at v1.

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
