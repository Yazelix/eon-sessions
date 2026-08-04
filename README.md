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
`orb-bi4.3` candidate adds complete structured presentation frames and ordered
revisions for review; Orbit does not render a terminal.

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
`orb-bi4.3` candidate uses `RenderState::update` and its public row and cell
iterators to encode a complete versioned frame containing geometry, styled
graphemes, colors and palette, cursor state, active screen, title, working
directory, and hyperlinks. Version 1 explicitly advertises hyperlink support
and declares Kitty graphics unsupported.

Each frame has a 4 MiB and 100,000-cell bound. Nonblocking client output retains
the initial frame and at most one coalesced latest frame when no diagnostic
response separates them; total queued output is bounded. The co-located
real-PTY proof in [`src/presentation.rs`](src/presentation.rs) covers output before attach,
ordered revisions, alternate-screen rich state, restoration of an inactive
primary screen with pending wrap, split CSI, UTF-8, and APC input, a slow client,
background-only erased cells, and final-state convergence after reattach. The
candidate adds no dependency.

## PTY-lifetime proof

The Linux-only proof uses one foreground binary for both roles:

```sh
cargo run -- serve
cargo run -- client
```

The server owns the PTY, child process, terminal state, presentation extraction,
input encoding, resize, and terminal-generated replies on one thread. The
client sends a bounded diagnostic message set containing semantic key, mouse,
focus, paste, and resize events. It receives structured presentation frames,
acknowledgements, and limited lifecycle diagnostics, never raw PTY output.

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

The subprocess checks in [`tests/lifecycle.rs`](tests/lifecycle.rs) use protocol
responses and PTY state as synchronization. They prove resize through `stty`,
observe output produced by a blocked foreground command after the client has
disconnected, reconnect to the same shell PID, exercise second-client
rejection, and check aborted attachment, stale-socket identity, permissions,
child exit, startup and established SIGTERM cleanup, foreground-process reaping,
and replacement-socket ownership with bounded timeouts.

## Contract

Orbit keeps a real terminal process and the sole authoritative libghostty state
alive independently of its graphical client. One local client may attach at a
time. On attachment, the client receives a coherent complete structured
presentation frame at one revision followed by later frame revisions in order.
The client sends semantic input for Orbit to encode against authoritative
terminal state. The initial contract covers surviving client exits and
disconnections while Orbit continues running; it does not cover daemon or
machine restarts.

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
- one Rust package and binary through the `orb-bi4.3` headless convergence
  proof
- if that gate passes, create the private `luccahuguet/venus` repository before
  `orb-bi4.4`; graphical Venus code does not live in Orbit
- no multiplayer, remote transport, Zellij compatibility, layouts, tabs,
  plugins, or configuration framework
- Mars and Mars Next may inform behavior, but their code and architecture are
  not foundations for Orbit

## Development

```sh
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
git diff --check
br ready
```

Beads contain the dependency-ordered experiment plan.

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
| Rust source | 2,322 |
