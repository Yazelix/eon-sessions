# Orbit rationale

Orbit is a pre-implementation architecture experiment. This file records its
origin, hypothesis, and stop conditions. The README and `AGENTS.md` define the
current contract and boundaries; Beads define authorized work.

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
  discussion and
  [libghostty roadmap](https://mitchellh.com/writing/libghostty-is-coming)
  led us to the ownership inversion: terminal multiplexers already need
  terminal-emulation state, and that state can come from the same reusable core
  as a graphical terminal.

We combine those observations for Yazelix. Superlogical's unpublished design
may differ from Orbit.

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
                     snapshot + ordered tail
                              v
                   Orbit/Mars libghostty client -> pixels

client input and resize -----------------------> Orbit -> PTY
```

Orbit owns process lifetime, the PTY, and authoritative terminal state. The
client owns presentation and user input. On attachment, Orbit establishes one
atomic boundary: everything before it is represented by a coherent snapshot,
and everything after it is delivered once in the raw PTY tail. Both sides use
libghostty so convergence does not depend on two unrelated terminal parsers.

We start with one local session and one active client, giving input and resize
one unambiguous owner and isolating state transfer from workspace policy.

## Why it may be better

- A client can disappear without taking the shell or its foreground process
  with it.
- Mars gains persistence without Zellij's composited terminal stream or
  compatibility surface.
- The server and client share one terminal-semantics implementation.
- Snapshot-and-tail convergence is a precise contract that can be tested with
  real PTYs.
- A successful design gives Mars a small session owner instead of inheriting a
  general-purpose multiplexer architecture.
- Starting clean permits ownership and tests to be stricter than Mars Next
  before compatibility makes mistakes expensive.

## Why it may fail

The visible grid is not complete terminal state. Correct continuation can also
depend on cursor and keyboard modes, alternate-screen state, tab stops,
character sets, palette changes, hyperlinks, images, scrollback, and a parser
partway through an escape sequence. A snapshot that merely looks correct can
diverge on the next byte.

Other material risks are:

- libghostty's embeddable APIs may not expose the state required by a headless
  owner or an attachment snapshot;
- obtaining that state may require a large fork or unstable private API;
- switching atomically from snapshot to live bytes may introduce a gap,
  duplication, or unbounded buffering;
- two live libghostty states cost memory and must remain version-compatible;
- the Orbit process becomes a failure boundary for every PTY it owns;
- a one-session experiment is not yet a replacement for Zellij's workspace
  features;
- native-window and rendering integration may outweigh the simplicity gained
  in the session layer.

The feasibility and convergence work address these risks before UI polish.

## Why this is uncommon

Mature multiplexers have kept PTYs alive for decades. Orbit adds the harder
requirement of reconstructing a graphical client from complete terminal
semantics and continuing from an ordered raw byte stream.

Historically, terminal-emulation cores were embedded inside applications rather
than exposed as stable libraries. Existing multiplexers also gain portability
by presenting a conventional terminal to many unrelated clients, so their
architecture favors broad compatibility, remote attachment, and shared
sessions over a native client coupled to the same state engine. Even with a
shared engine, serialization, parser state, and an atomic handoff remain hard.

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

Orbit replaces Zellij's process, PTY, and session-lifetime role. Mars remains
the graphical terminal client; `yzx` and the wider Yazelix environment remain
above both components.

The desired client shares Orbit's terminal core, while the current Mars
application uses a different one. The experiment includes the smallest fresh
client needed to prove the shared-engine design. If the experiment succeeds,
that client inherits the Mars product name and Orbit remains the headless
runtime. Orbit begins with both sides, then settles into the Zellij side of the
current chain.

## Naming and eventual ownership

During the experiment, Orbit contains both the headless runtime and the minimum
client required to prove it. If it graduates, the product boundary becomes:

```text
Yazelix Nova
|-- yzx: launcher and integration
|-- Mars: graphical terminal client
`-- Orbit: durable local session runtime
```

Orbit survives graduation as the runtime name. A failed experiment leaves Mars
unchanged. A successful one keeps processes in Orbit while clients come and go.

## Evidence sequence

Beads order the work:

1. Prove the libghostty attachment model with deterministic state.
2. Own one real PTY and survive diagnostic-client detachment.
3. Prove a race-free snapshot boundary and convergence after arbitrary tails.
4. Add one minimal native client.
5. Harden the one-client boundary.
6. Dogfood through an opt-in Yazelix path.
7. Decide later ownership from evidence.
8. Decide whether Orbit graduates beneath Mars.

A failed contract returns the project to planning before the next step.

## Graduation criteria

Graduate Orbit only if all of these are true:

- the server and client converge after detach/reattach with hidden terminal
  modes and simple text output;
- the PTY survives client failure and has explicit exit semantics;
- rapid output, alternate screens, Unicode, partial escape sequences, and
  resize boundaries behave deterministically;
- a native client can use the design without a broad adapter or libghostty
  fork;
- fresh Yazelix dogfood is useful with shells, Neovim, Yazi, full-screen TUIs,
  and long-running processes;
- the resulting ownership and LOC are smaller than continuing the current
  Mars/Zellij chain or Mars Next;
- no known P0 or P1 correctness risk remains.

## Stop criteria

Stop and return to planning if any of these become true:

- a sufficient snapshot requires maintaining a large libghostty fork;
- hidden terminal state cannot be transferred or reconstructed reliably;
- correctness requires replaying unbounded PTY history;
- snapshot-to-tail handoff cannot be made race-free without disproportionate
  protocol machinery;
- the client needs a second terminal-semantics implementation;
- the smallest usable slice grows into layout, compatibility, multiplayer, or
  remote-session work before proving detach/reattach;
- dogfood shows no meaningful advantage over the simpler existing options.

If we stop, we keep the evidence and avoid another untrusted multiplexer
implementation.
