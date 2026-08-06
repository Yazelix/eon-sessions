# Changelog

Orbit records accepted user- and consumer-visible changes here. Contracts
remain canonical in `docs/CONTRACTS.md`, dependency decisions in
`docs/CRATES.md`, and implementation evidence in Beads.

## Unreleased

- Orbit owns one real Linux PTY shell and the sole authoritative libghostty
  terminal state in a foreground process.
- The bounded local client can detach and reattach to the same live shell,
  while Orbit admits exactly one client and owns semantic input encoding.
- The private Unix socket and child process have explicit stale-path,
  permission, child-exit, foreground-process, and signal-shutdown behavior.
- Attachment begins with a bounded, versioned, complete structured
  presentation frame followed by ordered revisions. Frames carry rich terminal
  presentation state and explicitly declare Kitty graphics unsupported.
- ORBF v1 has one dependency-free canonical Rust package for Orbit and pinned
  consumers, with owned frame values, strict bounded decoding and encoding,
  and rejection of stale or duplicate complete-frame revisions.
- ORBS v1 adds the dependency-free canonical attachment, frame-envelope,
  lifecycle, semantic key, mouse, focus, arbitrary paste, and resize codec at
  `838b67652c4df1979e599b9c401ee664ffac66bd`, hardening `ORB-C3` and `ORB-C4`
  and changing `ORB-C5` to the reusable Venus boundary. This intentionally
  replaces the private unaccepted line-oriented diagnostic syntax without an
  adapter or compatibility window; Orbit's in-repository consumers moved
  atomically, and Venus pins the accepted revision. Role-incompatible kinds and
  message-specific invalid lengths, including a frame shorter than any
  canonical ORBF value, fail from the header before payload-sized buffering,
  and PTY resize preserves the accepted full surface pixel geometry
  instead of reconstructing only the cell grid. Key-associated text rejects C0,
  DEL, and macOS function-key PUA values before terminal encoding; clients send
  those inputs as semantic keys rather than text. Positional modifier bits are
  rejected unless their corresponding logical modifier is active, consistently
  on encoding and decoding. Buttonless mouse input is valid only for bare
  motion, wheel directions are valid only as momentary press events, and the
  terminal mapper distinguishes those ticks from held-button motion outside the
  viewport. Mouse axes and surface pixel dimensions are bounded to the selected
  terminal mapper's `u16` domain before native coordinate conversion. After a
  fatal negotiation or protocol response is queued, Orbit
  stops accepting input and publishing later frames or lifecycle messages to
  that connection while the bounded response drains, and immediately releases
  unread client bytes and their retained input storage. After Orbit observes
  PTY closure, later semantic requests receive a terminal failure instead of a
  false acknowledgement, and already queued undeliverable bytes and their
  retained queue storage are released.
- The authoritative owner loop yields after each productive PTY read and caps
  the shared terminal-response and semantic-input backlog. A whole semantic
  input that exceeds the remaining bound receives a terminal failure and closes
  that client without partial admission; terminal-response overflow fails the
  session instead of silently losing an authoritative reply. The accepted
  `fdd55e1e8bf6b932c2949ea3c897601ff63c0743` pressure proof hardens `ORB-C3`
  and `ORB-C4` while advancing the transport evidence for `ORB-C7`.
- Venus is the accepted first external ORBS v1 and ORBF v1 consumer. Its
  accepted relationship is recorded at
  `8149002c7275a00db1c08fded171e09f249dd977`, and it pins Orbit
  `c905bf9610581747f1b07565814b501ca66cfaa6`; the later Orbit proof
  `838b67652c4df1979e599b9c401ee664ffac66bd` is compatible without a manifest
  migration because the protocol package and Cargo metadata are byte-identical.
