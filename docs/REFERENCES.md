# Orbit reference map

Orbit studies existing work at the implementation boundary where it can answer
a concrete question. A reference is evidence, not an architectural foundation,
dependency decision, or authorization to inherit its feature set.

When claiming an implementation bead, follow the evidence-first gate in
`AGENTS.md`. The bead classifies each source as required before code,
conditional on a named failure, comparison only, or rejected scope. Planning
routing does not count as implementation evidence.

1. Inspect the current source and documentation for every required reference,
   recording a release or commit when behavior matters.
2. Extract the smallest relevant contract, check, failure mode, or ownership
   lesson.
3. Record what Orbit uses, rejects, or still cannot prove in an append-only
   pre-implementation `Reference gate` comment.
4. Name the affected `ORB-C*` contracts and state how the evidence constrains
   ownership, dependencies, code shape, and the first contract check before
   editing code, tests, or manifests.
5. Ask before adding an unplanned dependency, compatibility surface, fork, or
   feature.

When a slice may change Cargo dependencies, follow the separate crate gate in
`AGENTS.md` and record the durable outcome in [the crate decision
index](CRATES.md). Compare credible implementation shapes rather than choosing
the first crate encountered in reference research. Normally include the
no-new-crate or existing-dependency path and at least two viable crate-backed
alternatives, with a smaller set only when the recorded search finds fewer
credible choices. Architecture fit, ownership, owned LOC, transitive cost,
maintenance, platform and build consequences, and focused measurements decide
the recommendation.

The implementation remains clean-room. Orbit may reproduce user-visible
behavior and independently derive checks, but it does not copy source from
Mars, Mars Next, or another project by default.

## Platform portability

- Inspect the exact `libc` release's Linux and Apple target definitions plus
  recorded official Darwin or macOS documentation for `openpty`, controlling
  terminals, process groups, signals, polling, nonblocking I/O, Unix sockets,
  permissions, and EOF or error behavior before defining the platform seam.
- Use `portable-pty`, `rustix`, and `rustix-openpty` as crate and API-shape
  comparisons, not automatic portability choices. The crate gate decides
  whether their abstraction and dependency cost improve Orbit.
- Keep terminal authority, semantic input, presentation framing, ordering, and
  client compatibility independent of the host OS. Do not treat building a
  macOS backend or CI as part of the initial portability audit.

## Attachment and terminal state

- [libghostty-vt](https://docs.rs/libghostty-vt/latest/libghostty_vt/) is the
  terminal-semantics owner under test. Study `Terminal`, `RenderState`, effects,
  input encoding, thread ownership, and the
  [VT formatter options](https://docs.rs/libghostty-vt/latest/libghostty_vt/fmt/struct.FormatterOptions.html).
  The Formatter proof records why hidden terminal state cannot form an exact
  checkpoint. Study the public read-only surface for dimensions, styled cells
  and graphemes, cursor, title, working directory, hyperlinks, images, and
  explicit capability handling while inactive screens, parser continuation,
  scrollback semantics, and PTY effects remain authoritative in Orbit.
- [zmx](https://github.com/neurosnap/zmx) is the closest implementation
  reference for Orbit's headless half. It owns persistent PTYs, communicates
  over Unix sockets, feeds output into libghostty, and emits a VT bootstrap when
  a client attaches. Study its formatter use, socket and process lifecycle,
  tests, and documented restoration failures. Independently prove Orbit's
  single-owner attach ordering, authoritative handling of alternate screens and
  split sequences, and one-client backpressure. Do not inherit its bootstrap
  plus raw-tail contract, multiple clients, SSH workflow, labels, commands, or
  session-management surface.
- [vt100](https://docs.rs/vt100/latest/vt100/) provides a small example of
  formatted screen state and state diffs. It is useful as a test oracle or
  counterexample, not as Orbit's production terminal engine.
- [termwiz](https://docs.rs/termwiz/latest/termwiz/) models screen changes and
  terminal capabilities. Study its change representation only if Orbit needs a
  comparison for state-diff testing; adopting its broader terminal stack is not
  part of the initial plan.

## PTY and session lifetime

- [zmx](https://zmx.sh/) demonstrates the closest end-to-end lifecycle and
  explicitly separates session persistence from window management.
- [shpool](https://github.com/shell-pool/shpool) and
  [abduco](https://github.com/martanne/abduco) are smaller prior art for durable
  shell sessions and detach/reattach ownership. Study process reaping, signals,
  socket cleanup, and failure semantics rather than their command surfaces.
- [portable-pty](https://docs.rs/portable-pty/latest/portable_pty/) and
  [rustix](https://docs.rs/rustix/latest/rustix/) are candidate implementation
  levels for the Linux PTY slice. Compare their owned LOC, lifecycle control,
  dependency footprint, and ability to express the required checks before
  proposing either dependency.
- [tastty](https://docs.rs/tastty/latest/tastty/) is a compact Rust vertical
  slice joining PTY I/O, a background parser, and optional Ratatui rendering.
  Study its API boundaries and tests without inheriting its terminal engine,
  Tokio requirement, or drop-based session lifetime.

## Native client and rendering

- [Ghostling](https://github.com/ghostty-org/ghostling) and
  [libghostty-rs](https://github.com/Uzaaft/libghostty-rs) show small clients
  built around libghostty. Study how they bridge `RenderState`, glyphs, input,
  resize, callbacks, and a native render loop. Determine which rendering pieces
  can consume Orbit-authored presentation state without giving Venus a second
  terminal authority. Use measured evidence to choose Venus's window, font,
  and renderer owners.
- [rio-vt](https://github.com/raphamorim/rio/tree/main/rio-vt) is the strongest
  Rust-native alternative terminal engine. Compare complete-state access,
  parser ownership, modern protocol support, API maturity, and dependency cost
  if libghostty cannot satisfy Orbit's contract. It replaces libghostty in that
  comparison; it is not an additional terminal engine.
- [Sugarloaf](https://github.com/raphamorim/rio/tree/main/sugarloaf) is Rio's
  renderer and is relevant only when evaluating a compatible rendering stack.
  [librio](https://github.com/raphamorim/rio/tree/main/librio) is the C ABI for
  non-Rust consumers and is unnecessary for Venus's Rust implementation.
- [justerm-core](https://github.com/kihyun1998/justerm) is a reference for a
  server-authoritative design that sends structured grid and damage frames to
  thin native or web renderers. Study its frame ownership, damage model, web
  boundary, and protocol costs while independently keeping Orbit's initial
  implementation to bounded complete frames. Orbit accepts responsibility for
  a presentation boundary but must not copy a text-grid ceiling or speculative
  general protocol framework.

## Terminal-facing utilities

- [Qwertty](https://github.com/joshka/qwertty) is useful for studying ordered
  terminal I/O, raw-mode restoration, query correlation, typed input, and
  policy gates for sensitive terminal features. It targets applications that
  own a controlling terminal, so it is not part of Venus's native client or
  Orbit's PTY core by default.
- [Ratatui](https://github.com/ratatui/ratatui) can support an optional
  diagnostic or administration TUI after the core contract is proven. It does
  not provide PTY ownership, terminal-emulator state, native windowing, or the
  graphical renderer required by Venus.

## Product and scope references

- [Amp Orbs](https://ampcode.com/manual/orbs) is a product-layer reference for
  client-independent work that can continue on a remote machine and be resumed
  from web, terminal, or mobile clients. An Orb combines a repository, agent
  thread, tools, terminal, services, and artifacts; its shared terminal
  currently runs inside tmux. Study its user model, setup and resume hooks,
  agent-readable logs and readiness surfaces, service portals, and separation
  between a durable environment and transient clients. It does not answer
  Orbit's PTY ownership, terminal-state convergence, or native-rendering
  questions.
- Amp's remote-machine provisioning, repository synchronization, secrets and
  identity, billing, wake policy, event system, multiplayer, and workspace
  orchestration belong above Orbit if Yazelix ever chooses that product scope.
  They remain excluded from the initial experiment. If Orbit grows from an
  internal runtime into a remote workspace product, revisit the naming
  proximity between Orbit and Orbs.
- [TUIOS](https://tuios.gaurav.zip/) is a warning and inspiration for
  terminal-as-workspace scope. Orbit takes no tiling, workspace, theme, or
  scripting features from it during the initial experiment.
- [Herdr](https://github.com/ogulcancelik/herdr) demonstrates durable agent
  terminals in a Rust client/server multiplexer. Orbit studies its user-visible
  detach and reattach behavior while excluding layouts, remote access,
  multiplayer, and agent-management policy.
- `Logimux` is this repository's shorthand for Superlogical's currently unnamed
  terminal multiplexer, not an official product name.
  [Superlogical](https://www.superlogical.com/), Mitchell Hashimoto's
  [multiplexer video](https://www.youtube.com/watch?v=o-qtso47ECk), and the
  [libghostty roadmap](https://mitchellh.com/writing/libghostty-is-coming)
  motivate the shared terminal-core ownership hypothesis. The video also
  motivates reconnecting transient native clients to durable work, while its
  remote, Tailscale, window-restoration, and split-restoration ideas remain
  outside the initial Orbit contract. Orbit relies only on published behavior
  and independently verified APIs.
- Zellij, Mars, and Mars Next remain comparison and behavioral references for
  dogfood and graduation. Their existing ownership or compatibility surfaces do
  not define Venus or Orbit.
