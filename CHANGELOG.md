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
  `814a5dbcfdd3284769f8fb982ffff21110618754`, hardening `ORB-C3` and `ORB-C4`
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
  viewport. After a fatal negotiation or protocol response is queued, Orbit
  stops accepting input and publishing later frames or lifecycle messages to
  that connection while the bounded response drains. After Orbit observes PTY
  closure, later semantic requests receive a terminal failure instead of a
  false acknowledgement, and already queued undeliverable bytes are released.
