# Changelog

Orbit records accepted user- and consumer-visible changes here. Contracts
remain canonical in `docs/CONTRACTS.md`, dependency decisions in
`docs/CRATES.md`, and implementation evidence in Beads.

## Unreleased

- Orbit owns one real Linux PTY shell and the sole authoritative libghostty
  terminal state in a foreground process.
- The bounded local diagnostic client can detach and reattach to the same live
  shell, while Orbit admits exactly one client and owns semantic input
  encoding.
- The private Unix socket and child process have explicit stale-path,
  permission, child-exit, foreground-process, and signal-shutdown behavior.
- Attachment begins with a bounded, versioned, complete structured
  presentation frame followed by ordered revisions. Frames carry rich terminal
  presentation state and explicitly declare Kitty graphics unsupported.
- ORBF v1 has one dependency-free canonical Rust package for Orbit and pinned
  consumers, with owned frame values, strict bounded decoding and encoding,
  and rejection of stale or duplicate complete-frame revisions.
