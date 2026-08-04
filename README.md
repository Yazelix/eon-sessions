# Orbit

Orbit is a clean-room Rust experiment for durable local terminal sessions in
Yazelix Nova. The experiment tests Orbit as the session runtime beneath Venus,
the greenfield graphical client. Mars remains the current graphical product
during the experiment.

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
cargo check
br ready
```

Beads contain the dependency-ordered experiment plan.

The [design rationale](docs/RATIONALE.md) records where the idea came from, the
ownership hypothesis, relevant prior art, tradeoffs, and explicit graduation
and stop criteria. The [reference map](docs/REFERENCES.md) connects existing
projects and crates to the implementation slice where their evidence is useful.
References are studied when that slice begins; they do not authorize additional
features or dependencies.

## LOC scorecard

| Surface | Lines |
| --- | ---: |
| Rust source | 1 |
