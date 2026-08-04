# Orbit crate decisions

This is the durable index of direct and architecture-shaping crate decisions.
The [reference map](REFERENCES.md) records projects worth studying; appearance
there does not approve a dependency. `AGENTS.md` defines the crate gate, and
Beads retain the full candidate evidence.

## Current decisions

| Boundary | Selected shape | Status | Credible alternatives | Why | Evidence |
| --- | --- | --- | --- | --- | --- |
| Authoritative terminal semantics | `libghostty-vt` 0.2.1, with `libghostty-vt-sys` 0.2.1 and pinned Ghostty `a887df42c56f6de86c0fe6da9c4eeca37931e083` | Selected for the experiment | Formatter reconstruction with the same crate; a binding extension or fork; replacement by `rio-vt` | The published crate supplies the chosen sole terminal authority and state-aware input. Its Formatter cannot provide exact client reconstruction, so Orbit keeps authority server-side and tests the public presentation surface before considering expansion or replacement. | `orb-bi4.1`; `orb-bi4.3` owns the next decision boundary |
| Linux PTY and process lifecycle | Direct `libc` 0.2.189 calls | Selected for the experiment; `orb-pmp` owns the platform-seam decision | Handwritten C ABI with no crate; `nix` 0.31.3 with narrow `fs`, `poll`, `process`, `signal`, and `term` features; `portable-pty` 0.9.0; `rustix` 1.1.4 with `rustix-openpty` 0.2.0 | The small explicit Linux syscall boundary keeps one owner and avoids a cross-platform abstraction. The measured `nix` shape adds `nix` and `cfg-if` plus a build dependency without reducing net owned LOC, while its `openpty` path still needs explicit close-on-exec handling and low-level ioctl details. The rustix shape adds `rustix-openpty`, `rustix`, and `linux-raw-sys`; completing its signal boundary adds further packages. `portable-pty` introduces a broader trait model and eleven normal packages. The accepted implementation has 847 lines in `src/main.rs`, including its in-file checks and the complete owner loop rather than only PTY calls. | Retroactive crate gate in `orb-bi4.2`; proof revision `ac0a291ebe7fcebe4d7cace914b89363856c4903`; `orb-pmp` owns portability acceptance |
| Structured presentation extraction | Existing libghostty dependencies and public read-only APIs first | Planned | A small approved binding extension; libghostty fork; replacement by `rio-vt`; secondary oracle using `vt100` or `termwiz` | No additional crate is justified until the public surface is measured against `ORB-C4` and `ORB-C6`. Engine replacement and binding expansion remain user decision gates. | `orb-bi4.3` |
| Native Venus window, font, and renderer | Undecided | Planned after the Orbit convergence proof | Ghostling and `libghostty-rs` ownership patterns; compatible renderer crates such as Sugarloaf if required; no unnecessary `librio`, Ratatui, or Qwertty dependency | Venus needs a measured crate gate against the exact Orbit frame contract. Reference implementations are not automatically dependencies. | `orb-bi4.4` |

## Recording rules

- Record direct and architecture-shaping choices, not every transitive package.
- Include the no-new-crate or existing-dependency shape whenever it is credible.
- Exact releases or commits, accepted and rejected alternatives, dependency
  footprint, owned LOC effect, build implications, and evidence Bead belong in
  each completed decision.
- Record whether each candidate preserves a credible macOS implementation path
  and the platform seam it would require. This does not make portability more
  important than contract fit, ownership, LOC, or dependency cost.
- `Selected for the experiment` is not a permanent endorsement. A later Bead
  may preserve, replace, or remove the choice through a fresh crate gate and
  explicit user approval.
- Candidate code does not become an accepted crate decision merely because it
  builds or passes tests.
