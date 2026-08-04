# Orbit rationale

Orbit is an architecture experiment. This file records its origin, hypothesis,
and stop conditions. The README and `AGENTS.md` define the current boundaries,
the [contract index](CONTRACTS.md) records accepted behavior and proof status,
and Beads define authorized work.

## Where the idea came from

Yazelix composes three independently evolved layers:

```text
yzx -> Mars -> Yazelix Zellij fork
```

Mars provides the graphical terminal, while Zellij owns durable PTYs, terminal
state, panes, and session behavior. The multiplexer and graphical terminal each
interpret terminal behavior and follow different ownership models.

We arrived at the Orbit hypothesis after examining three projects:

- [TUIOS](https://tuios.gaurav.zip/) treats the terminal as an entire workspace.
  Its feature set also shows how tiling, workspaces, themes, scripting, and
  window-management policy can overtake the core session problem.
- [Herdr](https://github.com/ogulcancelik/herdr) uses a Rust client/server
  multiplexer to keep real agent terminals alive across detach/reattach. It is
  close prior art for the use case. Herdr also covers agent state, layouts,
  remote access, and multiplayer concerns that Orbit excludes.
- Mitchell Hashimoto's public [Superlogical](https://www.superlogical.com/)
  discussion, [multiplexer video](https://www.youtube.com/watch?v=o-qtso47ECk),
  and
  [libghostty roadmap](https://mitchellh.com/writing/libghostty-is-coming)
  led us to the ownership inversion: terminal multiplexers already need
  terminal-emulation state, and that state can come from the same reusable core
  as a graphical terminal.

We combine those observations for Yazelix. Superlogical's unpublished design
may differ from Orbit. `Logimux` is this repository's shorthand for
Superlogical's currently unnamed terminal multiplexer, not an official product
name.

## The hypothesis

A conventional graphical-terminal and multiplexer chain resembles:

```text
application -> PTY -> multiplexer terminal model -> composed VT stream
            -> graphical terminal model -> pixels
```

We are testing a narrower ownership model:

```text
application -> PTY -> Orbit authoritative libghostty state
                              |
                  structured presentation frames
                              v
                         Venus renderer -> pixels

semantic client input and resize --------------> Orbit -> PTY
```

Orbit owns process lifetime, the PTY, and authoritative terminal state. The
client owns presentation and user interaction. On attachment, Orbit establishes
one atomic boundary: the client receives a coherent complete structured frame
at one revision followed by later frame revisions in order. Hidden terminal
state stays in Orbit, and semantic client input is encoded against that state.
The client does not parse a replicated PTY stream or run another authoritative
terminal emulator.

We start with one local session and one active client, giving input and resize
one unambiguous owner and isolating presentation transfer from workspace policy.
The boundary must account explicitly for rich presentation state instead of
assuming that a plain-text cell grid is the final product contract.

## Why it may be better

- A client can disappear without taking the shell or its foreground process
  with it.
- Venus gains persistence without Zellij's composited terminal stream or
  compatibility surface.
- Orbit is the only terminal-semantics owner; clients can use native, web, or
  mobile rendering stacks without reproducing hidden terminal state.
- Complete frames and monotonic revisions form a precise attachment contract
  that can be tested with real PTYs.
- A successful design gives Venus a small session owner instead of inheriting a
  general-purpose multiplexer architecture.
- Starting clean permits ownership and tests to be stricter than Mars Next
  before compatibility makes mistakes expensive.

## Why it may fail

The visible grid is not complete terminal state. Correct continuation can also
depend on cursor and keyboard modes, alternate-screen state, tab stops,
character sets, palette changes, hyperlinks, images, scrollback, and a parser
partway through an escape sequence. Orbit retains that hidden state, but a
presentation frame must still expose every user-visible consequence without
silently flattening styled text, metadata, or graphics.

Other material risks are:

- libghostty's embeddable APIs may not expose the read-only presentation state
  required by a headless owner;
- obtaining that presentation state may require a large fork or unstable
  private API;
- extracting and publishing frames atomically may introduce stale revisions,
  excessive copying, or unbounded slow-client buffering;
- a client-neutral rich presentation schema may become a maintenance and
  compatibility surface of its own;
- the Orbit process becomes a failure boundary for every PTY it owns;
- a one-session experiment is not yet a replacement for Zellij's workspace
  features;
- native-window and rendering integration may outweigh the simplicity gained
  in the session layer.

The feasibility and convergence work address these risks before UI polish.

## How references are used

Orbit studies prior art just in time rather than choosing an entire stack in
advance. Each implementation bead names the projects and crates that can answer
its immediate questions. The
[reference map](REFERENCES.md) records what to inspect, what each source can
teach Orbit, and which surrounding features remain outside the experiment.

Reference study is part of the evidence for completing a bead. It records a
release or commit where behavior matters, extracts a relevant contract or
failure mode, and states what Orbit accepts or rejects. It does not authorize a
dependency, fork, compatibility layer, or scope expansion. Those choices still
require the user. When a dependency choice is in scope, the separate
[crate decision index](CRATES.md) and crate gate compare credible
implementation shapes, including using no new crate, before the manifest
changes.

## Why this is uncommon

Mature multiplexers have kept PTYs alive for decades. Orbit adds the requirement
of reconnecting graphical clients to a host-owned rich presentation without
nesting another terminal emulator in the path.

Historically, terminal-emulation cores were embedded inside applications rather
than exposed as stable libraries. Existing multiplexers also gain portability
by presenting a conventional terminal to many unrelated clients, so their
architecture favors broad compatibility, remote attachment, and shared
sessions over a native presentation protocol. A structured boundary avoids
transferring parser state, but rich cell, metadata, graphics, ordering, and
backpressure semantics remain hard.

Libghostty gives us a plausible way to test this arrangement. Orbit determines
whether its APIs make the design small enough for the narrower Yazelix use
case.

## Why multiplayer is excluded

Multiple attached clients introduce shared input arbitration, focus, resize
authority, slow-client backpressure, clipboard and terminal-side effects,
authorization, and observable concurrency semantics. Those concerns would
shape the protocol before Orbit proves its basic value.

Yazelix serves one local user, and normal Git and agent workflows cover the
collaboration this project needs. We exclude multiplayer from the protocol and
from postponed scaffolding.

## Why Mars Next is not the base

Mars Next demonstrated that a Rust/libghostty direction could compile and
support small isolated checks. Its live-session work did not establish the
real-PTY, detach/reattach, socket, or terminal-convergence contracts on which
Orbit depends, and its default verification could omit Ghostty-backed paths.

Because the code is still small and unused as a compatibility surface, a clean
start is cheaper than trying to infer and repair its ownership model. Mars Next
remains a behavioral reference. Reuse requires a specific contract and an
explicit user decision.

## What Orbit would replace

Orbit would replace Zellij's process, PTY, and session-lifetime role in the
Nova path. Venus would provide graphical presentation; `yzx` and the wider
Yazelix environment remain above both components. Mars and Zellij remain the
current runtime until separate promotion and retirement decisions replace
them.

Venus consumes Orbit's structured presentation boundary, while the current Mars
application owns its terminal state directly. The experiment includes the
smallest Venus client needed to prove the one-authority design. Venus keeps its
product name if the experiment succeeds; it does not inherit Mars. Orbit owns
the headless proof through `orb-bi4.3`. Passing that convergence gate authorizes
creation of the private `luccahuguet/venus` repository before `orb-bi4.4`.
Graphical Venus code does not live in Orbit; the proven presentation contract
is the repository boundary.

## Naming and eventual ownership

During the first three experiment beads, Orbit contains the headless runtime
and diagnostic client required to prove it. The minimum graphical client begins
in the separate Venus repository after the convergence gate passes. If Venus
and Orbit graduate, the product boundary becomes:

```text
Yazelix Nova
|-- yzx: launcher and integration
|-- Venus: graphical terminal client
`-- Orbit: durable local session runtime
```

Orbit survives graduation as the runtime name. Venus remains distinct from
Mars during coexistence and after any eventual Mars retirement. A failed
experiment leaves the current Mars and Zellij path unchanged. A successful one
keeps processes in Orbit while Venus clients come and go.

## Evidence sequence

Beads order the work:

1. Prove the libghostty attachment model with deterministic state.
2. Own one real PTY and survive diagnostic-client detachment.
3. Complete `orb-pmp` to audit and isolate the Linux platform seam without
   implementing macOS.
4. Prove race-free structured presentation convergence on reattach.
5. Complete `orb-9o6` to automate the machine-checkable governance invariants
   after the protocol has two implementation slices of evidence.
6. Create the private Venus repository and add one minimum native client.
7. Harden the one-client boundary.
8. Dogfood Venus and Orbit through an opt-in Yazelix path.
9. Decide later Venus and Orbit ownership from evidence.
10. Decide whether Venus and Orbit graduate into Yazelix Nova.

A failed contract returns the project to planning before the next step.

## Graduation criteria

Graduate Venus and Orbit only if all of these are true:

- reattaching clients receive coherent rich presentation while hidden terminal
  state remains authoritative in Orbit;
- the PTY survives client failure and has explicit exit semantics;
- rapid output, alternate screens, Unicode, partial escape sequences, and
  resize boundaries behave deterministically;
- a native Venus client can use the structured boundary without a second
  terminal emulator, broad adapter, or libghostty fork;
- fresh Yazelix dogfood is useful with shells, Neovim, Yazi, full-screen TUIs,
  and long-running processes;
- the resulting ownership and LOC are smaller than continuing the current
  Mars/Zellij chain or Mars Next;
- direct and architecture-shaping crate choices have evidence-backed reasons,
  recorded alternatives, and acceptable owned-LOC, build, and maintenance
  cost;
- Linux-specific runtime mechanics remain behind a narrow platform seam, while
  core ownership and cross-repository contracts have no unrecorded macOS
  blocker;
- no known P0 or P1 correctness risk remains.

## Stop criteria

Stop and return to planning if any of these become true:

- a sufficient read-only presentation surface requires maintaining a large
  libghostty fork;
- rich visible terminal state cannot cross the presentation boundary without
  silent loss or a text-grid ceiling;
- correct attachment requires retaining unbounded frame history;
- atomic frame extraction and publication cannot be made race-free without
  disproportionate protocol machinery;
- the client needs another terminal-semantics implementation or PTY parser;
- the smallest usable slice grows into layout, compatibility, multiplayer, or
  remote-session work before proving detach/reattach;
- macOS would require redesigning core ownership, presentation or input
  contracts, or durable state rather than adding a bounded platform backend;
- dogfood shows no meaningful advantage over the simpler existing options.

If we stop, we keep the evidence and avoid another untrusted multiplexer
implementation.
