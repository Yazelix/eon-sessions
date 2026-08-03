# Orbit

Orbit is a clean-room Rust experiment for durable local terminal sessions in
Yazelix Nova. It is intended to become the session runtime beneath the Mars
graphical client if the experiment succeeds.

## Status

The repository contains a one-line executable skeleton and a dependency-ordered
experiment plan. It does not yet spawn a PTY, retain a session, or render a
terminal.

## Contract

Orbit keeps a real terminal process alive independently of its graphical
client. One local client may attach at a time. On attachment, the client
receives a coherent terminal snapshot followed by every subsequent byte in
order, without a gap or duplication. The initial contract covers surviving
client exits and disconnections while Orbit continues running; it does not
cover daemon or machine restarts.

## Initial boundaries

- Linux first
- local Unix socket transport
- one terminal session and one active client
- one Rust package and binary until another owner is demonstrably necessary
- no multiplayer, remote transport, Zellij compatibility, layouts, tabs,
  plugins, or configuration framework
- Mars and Mars Next may inform behavior, but their code and architecture are
  not foundations for Orbit

## Development

```sh
cargo check
br ready
```

Beads contain the dependency-ordered experiment plan.

The [design rationale](docs/RATIONALE.md) records where the idea came from, the
ownership hypothesis, relevant prior art, tradeoffs, and explicit graduation
and stop criteria.

## LOC scorecard

| Surface | Lines |
| --- | ---: |
| Rust source | 1 |
