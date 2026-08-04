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
