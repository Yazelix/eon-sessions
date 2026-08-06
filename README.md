# Orbit

Orbit is a clean-room Rust experiment for durable local terminal sessions in
Yazelix Astra, a greenfield Yazelix line separate from Yazelix Nova. The
experiment tests Orbit as the session runtime beneath Venus, Astra's
greenfield graphical client. Nova remains an independently valuable product on
its current Mars and Zellij architecture during the experiment.

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
The same package owns the ORBS v1 local-session envelope and typed attachment,
frame, lifecycle, key, mouse, focus, paste, and resize messages consumed by
Orbit's server and diagnostic client. Its canonical key validation rejects C0,
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
contract tests feed a deterministic prefix to an authoritative terminal,
reconstruct a fresh terminal from formatter output, and then give both the same
distinguishing future tail.

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

These failures are reproducible in
[`tests/formatter_attachment.rs`](tests/formatter_attachment.rs). Orbit does not
add a fork, binding extension, compatibility wrapper, second terminal engine,
replay fallback, PTY, socket, or protocol implementation in this slice.

Orbit keeps the sole authoritative terminal state and supplies clients with
host-authored structured presentation frames. It does not export a checkpoint,
replicate raw PTY tails, or run another terminal emulator in the client. The
`orb-bi4.3` proof uses `RenderState::update` and its public row and cell
iterators to encode a complete versioned frame containing geometry, styled
graphemes, colors and palette, cursor state, active screen, title, working
directory, and hyperlinks. Version 1 explicitly advertises hyperlink support
and declares Kitty graphics unsupported.

Each frame has a 4 MiB and 100,000-cell bound. Nonblocking client output retains
the initial frame and at most one coalesced latest frame when no non-frame
session message separates them; total queued output is bounded. The co-located
real-PTY proof in [`src/presentation.rs`](src/presentation.rs) covers output before attach,
ordered revisions, alternate-screen rich state, restoration of an inactive
primary screen with pending wrap, split CSI, UTF-8, and APC input, a slow client,
background-only erased cells, and final-state convergence after reattach. The
proof adds no dependency.

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

The package also owns ORBS v1: a 12-byte explicit little-endian header with
`ORBS` magic, one exact revision, a typed message kind, zero reserved flags, and
a bounded payload length. Decoding is incremental and rejects unsupported
revisions, wrong-role or unknown kinds, and message-specific invalid
declarations—including a frame shorter than any canonical ORBF value—from the
header before payload-sized buffering. Complete-message decoding then rejects
malformed typed values, truncation, and trailing bytes.
Arbitrary paste bytes remain opaque; key events retain physical identity,
action, active and consumed modifiers, composition, multi-codepoint text, and
an optional unshifted codepoint. The session layer embeds canonical ORBF frames
without interpreting them again and adds no dependency.

## PTY-lifetime proof

The Linux-only proof uses one foreground binary for both roles:

```sh
cargo run -- serve
cargo run -- client
```

The server owns the PTY, child process, terminal state, presentation extraction,
input encoding, resize, and terminal-generated replies on one thread. The
client and server use only the bounded ORBS v1 codec. A version-negotiated
attachment receives typed outcomes, canonical presentation frames,
acknowledgements, bounded failures, and session exit. Client messages carry
semantic key, mouse, focus, arbitrary paste, and full surface-resize events,
never raw PTY output.

The default socket is `$XDG_RUNTIME_DIR/yazelix-orbit/orbit.sock`, falling back
to `/tmp/yazelix-orbit-$UID/orbit.sock`. Its directory is user-owned and private,
and the socket mode is `0600`. Orbit removes a connection-refused stale socket,
refuses to replace any non-socket path, rejects a simultaneous second client,
retains the last PTY size while detached, reaps an exited child, and removes the
socket after normal or signal-driven shutdown.

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
observe output produced by a blocked foreground command after the client has
disconnected, reconnect to the same shell PID, exercise second-client
rejection, and check aborted attachment, stale-socket identity, permissions,
child exit, startup and established SIGTERM cleanup, foreground-process reaping,
replacement-socket ownership, and terminal request failure after a live child
closes every PTY descriptor, all with bounded timeouts.

## Contract

Orbit keeps a real terminal process and the sole authoritative libghostty state
alive independently of its graphical client. One local client may attach at a
time. On attachment, the client receives a coherent complete structured
presentation frame at one revision followed by later frame revisions in order.
The client sends semantic input for Orbit to encode against authoritative
terminal state. The initial contract covers surviving client exits and
disconnections while Orbit continues running; it does not cover daemon or
machine restarts.

ORBS v1 carrying canonical ORBF v1 frames is the accepted protocol shape for
the first Venus slice. Complete frames prove convergence and define the attach
boundary. Any later patch protocol keeps a complete frame as its resync
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
- one terminal session and one active client
- one product binary, one private canonical protocol library, and one isolated
  repository-governance tool; only the binary owns terminal and PTY authority
- the `orb-bi4.3` gate passed at
  `e4fde443e625180d7332eff4dff366f64bee30a8`; the private
  [`luccahuguet/venus`](https://github.com/luccahuguet/venus) repository owns
  graphical implementation, the canonical codec boundary is proved at
  `838b67652c4df1979e599b9c401ee664ffac66bd`, and Orbit accepts the minimum
  Venus consumer relationship recorded at
  `8149002c7275a00db1c08fded171e09f249dd977`
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

- Contract rows use unique `ORB-C*` IDs and a known status. Proved rows name a
  full Git commit and a check or evidence, and governed metadata cannot refer
  to an unknown contract.
- `bug`, `chore`, `feature`, and `task` Beads are implementation work unless
  labeled `spike`. They require contract routing or an explicit non-product
  declaration. Once closed, they also require a reference gate and closure
  evidence after the latest execution baseline, with current contract and crate
  decision markers and a portability disposition for product work.
- Selected crate-index rows name an exact version or commit, alternatives, and
  an existing evidence Bead. Selected, planned, candidate, and deferred rows
  remain distinct.
- Protocol exceptions recorded in Beads remain visible notices and never count
  as successful gates.

The check cannot establish that an agent read a reference, that an approval or
evidence comment is truthful, that a crate gate was required, or that a runtime
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
The [Astra technology boundaries](docs/STACK.md) record the cross-stack
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
| Product Rust source and tests | 5,645 |
| Governance Rust tool and tests | 765 |
| **Total owned Rust** | **6,410** |
