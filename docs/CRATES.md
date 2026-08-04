# Orbit crate decisions

This is the durable index of direct and architecture-shaping crate decisions.
The [reference map](REFERENCES.md) records projects worth studying; appearance
there does not approve a dependency. `AGENTS.md` defines the crate gate, and
Beads retain the full candidate evidence.

## Current decisions

| Boundary | Selected shape | Status | Credible alternatives | Why | Evidence |
| --- | --- | --- | --- | --- | --- |
| Authoritative terminal semantics | `libghostty-vt` 0.2.1, with `libghostty-vt-sys` 0.2.1 and pinned Ghostty `a887df42c56f6de86c0fe6da9c4eeca37931e083` | Selected for the experiment | Formatter reconstruction with the same crate; a binding extension or fork; replacement by `rio-vt` | The published crate supplies the chosen sole terminal authority and state-aware input. Its Formatter cannot provide exact client reconstruction, so Orbit keeps authority server-side and tests the public presentation surface before considering expansion or replacement. | `orb-bi4.1`; `orb-bi4.3` owns the next decision boundary |
| Platform-specific PTY and local-runtime mechanics | One concrete `src/platform.rs` seam around direct `libc` 0.2.189 calls | Selected for the Linux-first proof; no macOS implementation or support claim | Handwritten C ABI with no crate; `pty-process` 0.5.3; `nix` 0.31.3 with narrow `fs`, `poll`, `process`, `signal`, and `term` features; `portable-pty` 0.9.0; `rustix` 1.1.4 with `rustix-openpty` 0.2.0 | The seam owns PTY setup, process groups, descriptors, polling, signals, socket paths and permissions, and platform EOF classification while the owner loop sees concrete platform-neutral outcomes. Ghostty confirms the physical PTY split; portable-pty's trait model and eleven-package normal dependency graph are broader than Orbit needs. The measured `nix` shape adds `nix` and `cfg-if` plus a build dependency without reducing net owned LOC; the rustix shape adds `rustix-openpty`, `rustix`, and `linux-raw-sys`, with more packages needed for signals. The direct shape keeps Cargo unchanged and occupies 336 lines in `src/platform.rs`; `src/main.rs` is 579 lines, and total owned Rust is 1,554 lines. A focused `pty-process` scratch candidate passed all 13 tests and the canonical checks, removed 44 owned lines, and added `pty-process`, `rustix`, and `linux-raw-sys` to the normal Linux graph. The accepted proof retains the no-manifest candidate; adopting `pty-process` remains a separate user-approved crate gate. Darwin's zero-length master-read closure can map to the existing platform-neutral `Closed` outcome, but target compilation and runtime behavior remain unproved. | Retroactive crate gate in `orb-bi4.2`; exact Apple, Ghostty, portable-pty, and libc sources plus the portability disposition are recorded in `orb-pmp`; post-candidate `pty-process` evidence is recorded there for a future decision |
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
