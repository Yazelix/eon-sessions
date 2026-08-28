# Eon Sessions

Eon Sessions is the repository for Eon's durable terminal sessions. Its Orbit
subsystem is a clean-room Rust experiment that owns session lifetime and
terminal state beneath the Venus subsystem in Eon Desktop. Yazelix Nova remains
an independent product on its current Mars and Zellij architecture.

## Status

The completed `orb-bi4.1` proof rejects the released libghostty formatter as an
exact terminal checkpoint. The `orb-bi4.2` implementation runs one real PTY
shell and one authoritative libghostty terminal in a foreground Orbit process;
a diagnostic client can disconnect and later reach the same live shell. The
completed `orb-bi4.3` proof adds complete structured presentation frames and
ordered revisions; Orbit does not render a terminal. The canonical ORBF v1
values, bounded encoder and decoder, and strict complete-frame revision reducer
live in the dependency-free `orbit-protocol` workspace library so Orbit and an
exact-revision Venus consumer cannot drift into separate schemas. That
consumer boundary is proved at
`838b67652c4df1979e599b9c401ee664ffac66bd`.
The same package owns accepted canonical ORBS v10 at
`59975e9176f5caf8b78dc3273e88d9ecbb75dc3f`, replacing accepted ORBS v9 proof
`aed0bcb7e9ad08c8e3e086c7dad0a0eb3ef16672`, including one input-capable
attachment, one bounded read-only title/CWD observer, frames, lifecycle,
semantic input, selection and copy, terminal clipboard writes, revision-carrying
multi-row previews, typed vertical-wheel outcomes, and bounded signed
whole-row viewport commits. The currently composed Venus and Eon sources
consume exact accepted ORBS v10; independently composed Eonova does too. ORBS
v10 replaces the generic Finish acknowledgement with one
typed result naming the authoritative presentation revision at sequence
completion. A client waits only when it has not presented that exact revision
yet.
Orbit's server and diagnostic client use only the canonical values.
Orbit chooses terminal input at left press when authoritative mouse tracking is
active and Shift is absent; otherwise it chooses host selection. That route is
fixed through release or cancel. Terminal phases reuse Orbit's terminal-aware
mouse encoder, while host phases delegate cell, word, and logical-line gesture
semantics to libghostty. Orbit publishes selected cells through normal frames
and returns bounded plain text frozen at release for the semantic selection
clipboard; explicit copy targets the ordinary clipboard.
That frozen value survives later terminal output, resize or reflow, and active-
screen transitions; a new selection, client loss, or exit clears it.
Its canonical key validation rejects C0,
DEL, and macOS function-key PUA values as associated text before they reach the
terminal encoder; clients represent those inputs with semantic key identity.
Positional modifier bits are accepted only with their corresponding logical
modifier, so encoding and decoding enforce the same canonical key and mouse
values. Mouse press and release require a button, wheel directions are
momentary press events, and buttonless motion means that no button is pressed.
Mouse axes and surface pixel dimensions stay within the selected terminal
mapper's `u16` domain, so extreme client values fail before checked native
coordinate conversion.
Orbit passes held-button semantics to the terminal-aware mapper so active-button
motion remains reportable outside the viewport without treating a wheel tick as
held. Once a fatal negotiation or protocol response is queued, that client
cannot send later terminal input or receive later presentation frames or
session-exit messages while the bounded response drains, and its unread input
bytes and retained input storage are released immediately. Once PTY closure is
observed, semantic requests fail instead of being acknowledged or retained as
undeliverable input while Orbit continues to wait for the known child.

## Attachment-model proof

The proof pins `libghostty-vt` and `libghostty-vt-sys` 0.2.1 from crates.io.
The sys crate builds Ghostty commit
`a887df42c56f6de86c0fe6da9c4eeca37931e083`. The dependency builds on Linux
with Rust 1.96.0 and Zig 0.15.2. Cargo locks ten third-party Rust crates,
including the safe binding, sys crate, and proc-macro stack.

By default, the sys crate clones its pinned Ghostty source when
`GHOSTTY_SOURCE_DIR` is unset and may let Zig resolve packages from the network
unless `GHOSTTY_ZIG_SYSTEM_DIR` supplies a prefetched package set. The proof's
local Ghostty source checkout measured approximately 155 MiB; it is external
dependency material, not owned Rust LOC. Future Nix packaging must prefetch both
the Ghostty source and Zig package inputs instead of relying on network access
during the Cargo build.

The public VT formatter cannot satisfy Orbit's exact attachment contract. The
accepted `orb-bi4.1` experiment feeds a deterministic prefix to an authoritative
terminal, reconstructs a fresh terminal from formatter output, and then gives
both the same distinguishing future tail.

| State | Proof result |
| --- | --- |
| Active text, DECCKM, Kitty keyboard flags, palette, working directory, and state-aware Up-key encoding | Preserved in the checked corpus |
| Custom tab stop | Future tab behavior is preserved after explicit repositioning, but tab restoration moves the reconstructed cursor |
| Character set and currently active hyperlink | Preserved for future text when tab-stop replay is omitted |
| Inactive primary screen while alternate is active | Lost; exiting alternate reveals an empty primary screen |
| Pending wrap | Lost; the next printable byte diverges |
| Parser between CSI, UTF-8, or APC bytes | Lost; the shared future suffix diverges |
| Scrollback | Visible history and the checked resize tail converge, but the immediate scrollback row count differs |
| Closed-cell hyperlink metadata, title, and Kitty image storage | Lost |
| PTY-directed effects | Emitted once by the authoritative terminal; the reconstructed terminal has no PTY callback |
| Current SGR style, scrolling region, and character protection | Not classified by this proof; the failure conclusion does not depend on them |

Git commit `ac0a291ebe7fcebe4d7cace914b89363856c4903` preserves the exact
counterexample corpus. The current suite exercises the accepted authoritative
runtime through real PTY lifecycle and structured-presentation checks. The
repository carries no permanent test target for the rejected reconstruction
path. Orbit uses no fork, binding extension, compatibility wrapper, second
terminal engine, or replay fallback.

The focused Orbit terminal conformance corpus reuses seven authoritative unit
and real-PTY tests under one filter:

```sh
cargo test --locked conformance_
```

It covers parser continuation and terminal replies, complete rich reattachment,
semantic input and resize, canonical rich extraction, viewport routing and
retained-history reflow, and frozen selection copy. The contract-oriented case
inventory and assertion policy live in [`docs/CONTRACTS.md`](docs/CONTRACTS.md).
The check is offline, headless, and pinned to the same libghostty path as the
normal suite; it adds no second engine, fixture framework, or replay format.

Orbit keeps the sole authoritative terminal state and supplies clients with
host-authored structured presentation frames. It does not export a checkpoint,
replicate raw PTY tails, or run another terminal emulator in the client. The
optional `serve --ansi-palette-v1 RGB,...` component input accepts exactly 16
six-digit sRGB entries before the command separator. It replaces only
libghostty palette indices 0 through 15 for that process; omission retains the
built-in defaults, and ordinary OSC overrides and resets remain terminal-owned.
The `orb-bi4.3` proof uses `RenderState::update` and its public row and cell
iterators to encode a complete versioned frame containing geometry, styled
graphemes, colors and palette, cursor state, active screen, title, working
directory, and hyperlinks. Version 1 explicitly advertises hyperlink support
and declares Kitty graphics unsupported.

Each frame has a 4 MiB and 100,000-cell bound. Nonblocking client output retains
the initial frame and at most one coalesced latest frame when no non-frame
session message separates them; total queued output is bounded. The owner loop
honors libghostty's authoritative DEC 2026 synchronized-output mode: PTY
parsing, replies, and ordered effects continue while complete frames are held,
then one latest frame is published when the mode ends. A one-second watchdog
clears an abandoned mode. A new client's initial frame is deferred until the
hold ends or times out. Orbit holds one unresolved revision-bound vertical
preview or signed scroll and resolves it once against the complete state after
release instead of failing the client gesture. The owner loop processes one PTY
read per readiness turn, and the shared terminal-response and semantic-input
backlog has a fixed byte bound. Pressure rejects a whole new semantic input and
closes that client instead of retaining or partially
delivering unbounded data. The co-located real-PTY proof in
[`src/presentation.rs`](src/presentation.rs) covers output before attach,
ordered revisions, alternate-screen rich state, restoration of an inactive
primary screen with pending wrap, split CSI, UTF-8, and APC input, a non-reading
client during continuous PTY output, background-only erased cells, semantic
interrupt fairness, and final-state convergence after reattach. The proof adds
no dependency.

[`crates/protocol`](crates/protocol) is the sole ORBF v1 format owner. Its
strict decoder validates magic, version, dimensions, tags, reserved flags,
UTF-8, cursor bounds, truncation, trailing bytes, and the 4 MiB/100,000-cell
limits before making payload-sized allocations. Canonical incremental size
accounting prevents Orbit from retaining rows or cells beyond the same frame
budget while it materializes authoritative state. Its rich owned-frame corpus
round-trips byte-for-byte, every truncated prefix is rejected, and its reducer
accepts only strictly newer complete revisions. The package is private,
`std`-only, platform-neutral, and has no direct or transitive dependency;
libghostty, PTYs, sockets, input, and rendering remain outside it.

The accepted ORBS v10 producer uses a 12-byte explicit little-endian header with `ORBS`
magic, one exact revision, a typed message kind, zero reserved flags, and a
bounded payload length. Decoding is
incremental and rejects unsupported revisions, wrong-role or unknown kinds, and
message-specific invalid declarations—including a frame shorter than any
canonical ORBF value—from the header before payload-sized buffering.
Complete-message decoding then rejects
malformed typed values, truncation, and trailing bytes.
Arbitrary paste bytes remain opaque; key events retain physical identity,
action, active and consumed modifiers, composition, multi-codepoint text, and
an optional unshifted codepoint. Left-pointer phases carry bounded surface
positions and current modifiers, while Begin also carries the exact frame
revision and monotonic press time. Orbit pins terminal-versus-host routing;
libghostty owns single cell, double word, and triple logical-line press-drag
behavior for host selections. Copy and gesture cancellation remain explicit
without transferring terminal authority. A vertical preview request names an
already-presented frame; Orbit reads current authoritative state and returns
that revision with up to one active viewport of nearest-first canonical rows
without moving the viewport or sending PTY input. An accepted Finish reports
the authoritative presentation revision at sequence completion, independent of
its pinned route. An accepted vertical wheel returns either terminal-routed or
an atomic applied-row count and newer complete frame. A signed viewport commit
may likewise lag current presentation output; Orbit resolves current terminal
routing, moves at most 1,024 rows in one libghostty operation, and returns one
typed result with the requested and applied distance, one newer complete frame,
and the next bounded row window or authoritative edge. A request claiming a
future revision fails before mutation. The session layer embeds canonical ORBF
frames without interpreting them again and adds no dependency.

## PTY-lifetime proof

The Linux-only proof uses one foreground binary for both roles:

```sh
tic -x terminfo/eon.terminfo
cargo run -- serve
cargo run -- client
```

The first command installs Orbit's `eon` terminal identity in the current
user's terminfo database. Eon distribution remains responsible for installing
the same entry in its runtime closure.

The server owns the PTY, child process, terminal state, presentation extraction,
input encoding, resize, and terminal-generated replies on one thread. The
client and server use only the bounded ORBS v10 codec. An exact-version
attachment receives typed outcomes, canonical presentation frames,
acknowledgements, bounded failures, and session exit. Client messages carry
semantic key, mouse, focus, arbitrary paste, full surface-resize, and
revision-bound selection plus revision-carrying vertical-preview and bounded
signed-scroll events, never raw PTY output.
One separately negotiated observer receives only an acknowledgement, bounded
coalescible title/CWD metadata at exact presentation revisions, failures, and
session exit.

The default socket is `$XDG_RUNTIME_DIR/yazelix-orbit/orbit.sock`, falling back
to `/tmp/yazelix-orbit-$UID/orbit.sock`. Its directory is user-owned and private,
and the socket mode is `0600`. Orbit removes a connection-refused stale socket,
serializes concurrent stale-socket claims, refuses to replace any non-socket
path, admits at most one input-capable client and one metadata observer after
role negotiation, rejects an excess role with `Busy`, retains the last PTY size
while detached, reaps an exited child, and removes the socket after normal or
signal-driven shutdown.

The private same-boot management owner is enabled with
`serve SOCKET --management-v1 SESSION_ID RUN_ID COMPONENT_GENERATION -- COMMAND`.
It derives `SOCKET.management` and `SOCKET.record`, publishes the live record
only after the PTY, terminal owner, and both sockets are usable, and permits one
UID- and identity-validated management lease. A launcher may retain an empty
owned `SOCKET.record` inode. Orbit either wins its lock, marks it, and replaces
the pathname with Live, or fails startup without publishing when the launcher
already holds the lock. Lease loss is non-destructive;
the surface carries only acquire, status, and explicit stop. Natural exit or a
successful stop atomically replaces the live record with a typed terminal
tombstone and removes only the exact sockets. Messages and records are limited
to 4 KiB, identities to 128 UTF-8 bytes, and negotiation to one second. Eon and
Eonova consume this owner for accepted same-boot recovery and owner-routed
Stop; this mode does not promise logout, reboot, machine-restart, topology, or
same-UID isolation.

Orbit starts a Linux PTY Session with ordinary user process permissions and no
service-manager or delegated-cgroup requirement. SIGINT, SIGTERM, SIGHUP, and
an authorized management Stop send SIGHUP to the initial PTY process group and
the terminal's current foreground group. Orbit gives the child up to 500 ms to
exit. If it remains unreaped, Orbit sends SIGKILL to its still-stable initial
group and re-reads the terminal's foreground group before escalating it. If the
child exits during the grace period, Orbit sends no later signal to the cached
foreground-group number. Orbit requires the direct child to be reaped within
two seconds before reporting success. A child that had already exited before
shutdown is reaped without signaling its recyclable numeric identity.

This is terminal cleanup, not whole-process-tree ownership. A deliberately
detached process, a non-foreground group outside the initial group, or a
SIGHUP-ignoring foreground job whose shell exits during the grace period may
survive. Orbit does not scan process names, ancestry, session IDs, or procfs to
find it. Client disconnection remains non-destructive and separate from
explicit Session stop. This is not a macOS support claim.

The proof pins `libc` 0.2.189 for the small Linux `openpty`, controlling-terminal,
poll, resize, and signal boundary. This avoids `portable-pty` 0.9.0's general
cross-platform abstraction and dependency stack. `rustix` 1.1.4 exposes the
lower-level PTY calls, but a complete open-PTY path also needs
`rustix-openpty` 0.2.0. On Linux that alternative adds `rustix-openpty`,
`rustix`, and `linux-raw-sys` beyond packages already present in Orbit. Orbit
keeps Cargo unchanged for this proof. `pty-process` 0.5.3 is the narrower
credible alternative: a focused scratch build passed the canonical checks and
reduced owned Rust by 44 lines while adding `pty-process`, `rustix`, and
`linux-raw-sys` to the normal Linux graph. Adopting it remains a separate crate
decision.

Linux PTY, process-group, descriptor, polling, signal, socket-path, permission,
and EOF mechanics live in one concrete `src/platform.rs` seam. The owner loop
sees platform-neutral PTY I/O and readiness outcomes; terminal authority,
semantic input, and the attachment protocol do not contain `libc` values. This
is an isolation boundary, not a macOS backend or support claim.

The subprocess checks in [`tests/lifecycle.rs`](tests/lifecycle.rs) use typed
protocol responses, presentation frames, and PTY state as synchronization. They
prove negotiation failure, handshake ordering, ordinary and special-key input,
multiline paste, mouse and focus acceptance, resize through `stty`,
reversed selection across a soft wrap with wide and combining graphemes, exact
copy after later terminal activity, and client-scoped selection cleanup,
observe output produced by a blocked foreground command after the client has
disconnected, reconnect to the same shell PID, exercise second-client
rejection, and check aborted attachment, stale-socket identity, permissions,
child exit, startup and established SIGTERM cleanup, foreground-process reaping,
intentional foreground-job survival after the shell exits during grace,
replacement-socket ownership, and bounded recovery after a live child closes
every PTY descriptor and later reopens its terminal, plus simultaneous
stale-socket ownership, all with bounded timeouts.

## Contract

Orbit keeps a real terminal process and the sole authoritative libghostty state
alive independently of its graphical client. One local input-capable client
may attach at a time, while one read-only metadata observer may coexist. On
attachment, the input-capable client receives a coherent complete structured
presentation frame at one revision followed by later frame revisions in order.
The observer receives only live title, working directory, revision, and
lifecycle metadata. The client sends semantic input for Orbit to encode against
authoritative terminal state. The initial contract covers surviving client and
observer exits and disconnections while Orbit continues running; it does not
cover Orbit or machine restarts.

Venus source `e5e37a3df119ee2bcfa2493a2ce5a46307493732` consumes canonical ORBF v1
over ORBS v6 at exact Orbit proof
`780f5d746175b4a9b71df57c51ed4bfcc4c4c375`. Eon source
`10c402b3754d600a6bb0a0da0de2ff6d1d4feca3` composes that accepted pair;
Eonova source `92acf64e8c43531bd4c5d639bdfa99b717dfe5b3` consumes the resulting Eon
runtime. Accepted x86_64 Linux evidence
covers same-boot recovery, owner-routed Stop, and native Wayland delivery to the
primary selection. Ordinary clipboard delivery, Wayland without data-control,
broader compositors, and macOS remain unproved. No adapter, feature probe,
dual-version support, or compatibility window exists.
ORBS v7 at `baf8aa28dcaa50484cd221aa7730defedc2356bb` expanded the accepted
ORBS v6 one-row preview into a bounded row window.
ORBS v8 at `d9b22eb294f8f42b4f49324fd5467eab239c2917` replaced client-authored
viewport cells with bounded pointer positions and monotonic press time while
preserving every other v7 message family.
ORBS v9 at `aed0bcb7e9ad08c8e3e086c7dad0a0eb3ef16672` moved terminal-versus-host
left-pointer routing into Orbit, used Shift as the host-selection bypass, pinned
each route through completion, and tagged release copy for the semantic
selection clipboard while explicit copy targeted the ordinary clipboard.
ORBS v10 at `59975e9176f5caf8b78dc3273e88d9ecbb75dc3f` replaced the generic Finish
acknowledgement with an exact presentation revision so clients can serialize
rapid pointer sequences without timing or route inference.
Complete frames prove convergence and define the
attach boundary. Any later patch protocol keeps a complete frame as its resync
fallback. The preferred later replication shape uses revisioned row patches
during steady operation, bounded history pages, and a separate ordered stream
for terminal effects. That
direction changes no current contract and requires a user-approved protocol
decision plus measured evidence before implementation. The
[design rationale](docs/RATIONALE.md#replication-boundary) records the model and
its recovery rules.

The [contract index](docs/CONTRACTS.md) assigns stable `ORB-C*` identities to
accepted behavior and records its owner, proof status, checks, and remaining
gap. Candidate implementation evidence does not become proved until its owning
Bead is accepted and the exact proof-bearing commit is recorded.

## Initial boundaries

- Linux first and the only required initial target; the product core and
  cross-repository contracts remain macOS-credible without promising a macOS
  backend, build, CI, packaging, or support
- local Unix socket transport
- one terminal session, one input-capable client, and one read-only metadata
  observer
- one product binary, one private canonical protocol library, and one isolated
  repository-governance tool; only the binary owns terminal and PTY authority
- the `orb-bi4.3` gate passed at
  `e4fde443e625180d7332eff4dff366f64bee30a8`; the private
  [`Yazelix/eon-desktop`](https://github.com/Yazelix/eon-desktop) repository
  owns graphical implementation through Venus, the canonical codec boundary is
  proved at
  `838b67652c4df1979e599b9c401ee664ffac66bd`, Orbit accepts the minimum Venus
  consumer relationship recorded at
  `8149002c7275a00db1c08fded171e09f249dd977`, and current Venus proof
  `8929c9f9d151641a343813ddeb6005cb9c771286` preserves that boundary while
  passing the pre-graduation workload envelope
- no multiplayer, remote transport, Zellij compatibility, layouts, tabs,
  plugins, or configuration framework
- Mars and Mars Next may inform behavior, but their code and architecture are
  not foundations for Orbit

## Development

```sh
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo run --locked --quiet -p orbit-governance -- .
cargo install --locked \
  --git https://github.com/luccahuguet/starcompass.git \
  --rev 95c29fa76a971726b65e1d1dc06c518d525c46a2 \
  --root target/starcompass
target/starcompass/bin/starcompass check-consumer \
  --overlay .agent-protocols.local.md \
  --exceptions .agent-protocols.exceptions.json \
  --manifest .agent-protocols.json \
  --agents AGENTS.md
cargo clippy --locked --workspace --all-targets -- -D warnings
git diff --check
br ready
```

Beads contain the dependency-ordered experiment plan.

### Orbit–Venus performance baseline

[`tools/perf/orbit-venus-baseline.sh`](tools/perf/orbit-venus-baseline.sh)
repeats the manual Linux baseline without vendoring a benchmark corpus or
adding a product mode. It consumes exact caller-supplied Orbit and Venus
binaries. Throughput mode also consumes an exact upstream
[vtebench](https://github.com/alacritty/vtebench) checkout whose release binary
and existing `benchmarks/` corpus have already been built. Lifecycle mode uses
exact caller-supplied Neovim and Yazi binaries.

Run the local checks first, then leave the desktop idle. Benchmark runs open
native Venus windows and 50 short-lived `foot` windows, so they can change
focus and tiling temporarily. Evidence includes local paths, machine and display
details, and process logs; review it before sharing.

```sh
tools/perf/orbit-venus-baseline.sh --self-check

export ORB_BASELINE_CPUS=9-15
export ORB_BASELINE_GRID='58 93'
export ORB_BASELINE_NVIM="$(command -v nvim)"
export ORB_BASELINE_YAZI="$(command -v yazi)"

tools/perf/orbit-venus-baseline.sh throughput \
  /exact/orbit/bin/yazelix-orbit \
  /exact/venus/bin/yazelix-venus \
  /exact/vtebench-checkout \
  /tmp/eon-performance

tools/perf/orbit-venus-baseline.sh lifecycle \
  /exact/orbit/bin/yazelix-orbit \
  /exact/venus/bin/yazelix-venus \
  /tmp/eon-performance
```

The harness requires native Wayland plus `pidstat`, `taskset`, `ss`, and
standard GNU command-line tools. Throughput also requires Git; lifecycle also
requires `foot`. It validates required commands and host load before launching
Orbit, then rejects any grid mismatch before the measured workload starts.
Each mode attempt creates a unique evidence directory beneath the supplied
parent and records the exact harness, hashes, revisions, environment, process
samples, and summaries; throughput also retains raw vtebench DAT files. This
output is an internal, intentionally unstable evidence artifact rather than a
product trace format.

vtebench measures blocking PTY consumption, not frame rate or visual latency.
The lifecycle reattachment check proves process survival, transport attachment,
and authoritative grid continuity. Native Wayland does not expose another
client's keypress-to-pixel or first-visible-frame timing, so the harness reports
those visual metrics as unavailable instead of substituting title changes,
wrong-output screenshots, or internal timestamps.

The [CI workflow](.github/workflows/ci.yml) runs on pushes to `edge` that
change Cargo metadata, Rust source or tests, `build.rs`, Rust toolchain files,
the governance tool or its governed metadata, or the workflow itself. It can
also be run deliberately with `gh workflow run ci.yml --ref edge`. One standard
Ubuntu job installs a checksum-verified Zig 0.15.2 toolchain, logs the toolchain
versions, installs Starcompass from its immutable public Git revision, and runs
the workspace and consumer-import checks above. Superseded runs are cancelled,
and each job is limited to ten minutes.

The repository or its owning account must retain a $0 Actions product budget
with **Stop usage when budget limit is reached**, budget threshold alerts, and
included-usage alerts enabled. CI can consume the private repository's
included runner minutes, but this budget prevents paid overage. The workflow
does not run for unrelated documentation.

The governance command checks only deterministic repository facts:

- Contract sections and crate-decision records retain their canonical labeled
  fields. Contract sections use unique `ORB-C*` IDs and a known status; proved
  sections name a full Git commit and a check or evidence. `docs/CONTRACTS.md`,
  `docs/CRATES.md`, and Beads cannot refer to an unknown contract.
- Selected crate decisions name an exact version or commit, alternatives, and
  an existing evidence Bead. Selected, planned, candidate, and deferred
  decisions remain distinct.
- Beads JSON must parse so those contract and crate-evidence links can be
  checked. `br doctor` owns tracker schema, duplicate IDs, storage, and
  integrity.

The check does not classify implementation work or interpret execution,
reference, crate, closure, portability, or protocol-exception prose;
`AGENTS.md` and applicability-aware review own that policy. It cannot establish
that an agent read a reference, that evidence is truthful, or that a runtime
contract actually passes. It deliberately does not parse `.agent-protocols.*`
or generated `AGENTS.md`: Starcompass owns that interpretation. CI installs
Starcompass from exact public Git revision
`95c29fa76a971726b65e1d1dc06c518d525c46a2` and runs its source-independent
`check-consumer` command against the four local import files. That proves their
structure, hashes, framing, and local suffix agree; it treats the canonical
protocol section as opaque. Complete canonical-protocol authentication still
uses a source checkout at the exact commit in `.agent-protocols.json`.

The [contract index](docs/CONTRACTS.md) is the durable behavioral source of
truth. The [design rationale](docs/RATIONALE.md) records where the idea came
from, the ownership hypothesis, relevant prior art, tradeoffs, and explicit
graduation and stop criteria. The [reference map](docs/REFERENCES.md) connects
existing projects and crates to the implementation slice where their evidence
is useful. The [crate decision index](docs/CRATES.md) records which direct or
architecture-shaping dependencies are selected, rejected, or still pending.
The [Eon technology boundaries](docs/STACK.md) record the cross-stack
language, WebAssembly, extension, and client-framework posture without
authorizing those deferred features. The [changelog](CHANGELOG.md) records
accepted user- and consumer-visible changes without duplicating candidate
evidence or internal development work.
References are studied when a slice begins; they do not authorize additional
features or dependencies.

Implementation Beads begin with a recorded execution baseline, then pass the
reference gate and any triggered crate gate before code shape is chosen. Proof
is tied to exact commits, cross-repository consumers pin those revisions, and
only the user may grant a narrowly recorded protocol exception. Linux-specific
runtime mechanics stay behind a narrow platform seam while the core ownership
and protocol remain platform-neutral.

## LOC scorecard

| Surface | Lines |
| --- | ---: |
| Product Rust source and tests | 13,999 |
| Governance Rust tool and tests | 767 |
| Eon terminfo source | 2 |
| Manual performance harness | 647 |
| Shell test fixtures | 104 |
| **Total owned Rust** | **14,766** |
| **Total owned implementation source** | **15,519** |
