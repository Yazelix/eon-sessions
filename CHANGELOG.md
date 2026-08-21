# Changelog

Orbit records accepted user- and consumer-visible changes here. Contracts
remain canonical in `docs/CONTRACTS.md`, dependency decisions in
`docs/CRATES.md`, and implementation evidence in Beads.

## Unreleased

- The accepted `2518512758c7848bbd87f907b105c8bcf0fc4e1b` owner proof hardens
  `ORB-C13`: an optional exact empty record claim serializes the Ready boundary.
  A launcher-held claim prevents publication; an Orbit-held claim is marked
  before Live replacement so later record removal cannot restore pre-Ready
  local-child authority. Eon and Eonova adopt the owner revision separately.
- Prove `ORB-C13` through exact Eon consumer acceptance at
  `0bf0b165d06b4a8162be497011070f61f6c2000a`: same-boot supervisor loss,
  multi-Session and EonTerm recovery, contending replacements, and owner-routed
  Stop preserve Orbit authority without fallback or duplicate ownership.
- The accepted `7f067b30e97d0b4787a7c6c0bbe3dd8a80a61c2c` proof adds
  `ORB-C12`: explicit Linux Session shutdown reaps the direct PTY child and
  empties the exact Orbit-owned cgroup before reporting success. Processes
  deliberately handed to another lifecycle scope are outside Orbit ownership;
  graphical-client disconnection remains non-destructive under `ORB-C1`.
- The accepted `2e41997cbec202afc5222f084ffb98973a01b0f0` proof hardens
  `ORB-C4` and `ORB-C6`: canonical ORBF v1 rejects malformed wide-cell,
  spacer-tail, and wrapped-edge topology before encoder, decoder, or reducer
  acceptance. Valid ORBF v1 and ORBS v4 bytes are unchanged.
- The accepted `9c0617a97612cdd045ed67b5cd7c87244eb888e6` proof changes
  `ORB-C8` and replaces ORBS v3 with exact-version ORBS v4. A client may preview
  one revision-bound adjacent canonical row without moving Orbit's viewport or
  sending PTY input. Each vertical wheel returns a typed terminal-routed result
  or one atomic applied-row count and newer complete frame. Venus and the pinned
  Eon/Eonova runtime require separate owner-first adoption; no adapter, dual
  decoder, feature probe, or compatibility window is provided. ORBF remains v1.
- The accepted `6de95296d252c119d4fdba2d9b03cec1a09355ae` proof hardens
  `ORB-C6`: canonical ORBF v1 rejects capability values that contradict its
  advertised hyperlink support or explicit lack of Kitty graphics before
  encoder, decoder, or reducer acceptance. Valid ORBF v1 and ORBS v3 bytes are
  unchanged.
- The accepted `9bc87191dd90fd3d7db939127f1ebedfccd2b48d` proof hardens
  `ORB-C1`: Orbit serializes stale-socket inspection, removal, bind, and setup
  through the validated private parent directory. Simultaneous launches leave
  one reachable owner and one explicit loser, without allowing the loser to
  unlink the winner's socket. ORBF v1 and ORBS v3 are unchanged.
- The accepted `cb0703010c3a4944980404b665abff795be09fb4` proof hardens
  `ORB-C1`: Linux PTY-master `EIO` remains retryable while Orbit's direct child
  is alive, so a temporary zero-slave interval no longer closes the Session.
  Recovery is bounded to avoid CPU spin, and later presentation and semantic
  input continue through the same authoritative terminal. ORBF v1 and ORBS v3
  are unchanged.
- The accepted `83d700882ee156bac0de0112b3f1daade049b9fb` proof hardens
  `ORB-C1` and `ORB-C7`: PTY-side output pressure may disconnect a concurrently
  ready client without letting the stale poll result panic Orbit. The live PTY
  session remains available for later attachment. ORBF v1 and ORBS v3 are
  unchanged.
- The accepted `26b4b9465b3f0e74f091a0aae93fddc61412b893` proof hardens
  `ORB-C4` and `ORB-C6`: Orbit honors authoritative DEC 2026 synchronized
  output, continuing terminal parsing, replies, and ordered effects while
  holding intermediate frames. Normal completion publishes the latest complete
  frame; a one-second watchdog releases abandoned holds, and a new client's
  initial frame waits for completion or timeout. ORBF v1 and ORBS v3 are
  unchanged.
- Orbit accepts one optional, versioned 16-color ANSI palette from its
  component launcher before PTY startup. The supplied defaults appear in the
  existing ORBS frames and remain the target of OSC 104 resets; standalone
  launches retain libghostty defaults.
- Orbit PTY children advertise `TERM=eon`. The repository-owned terminfo entry
  inherits the conservative `xterm-256color` baseline and is installed with
  standard `tic` before source-run sessions.
- Orbit owns one real Linux PTY shell and the sole authoritative libghostty
  terminal state in a foreground process.
- The bounded local client can detach and reattach to the same live shell,
  while Orbit admits exactly one client and owns semantic input encoding.
- The private Unix socket and child process have explicit stale-path,
  permission, child-exit, foreground-process, and signal-shutdown behavior.
- Attachment begins with a bounded, versioned, complete structured
  presentation frame followed by ordered revisions. Frames carry rich terminal
  presentation state and explicitly declare Kitty graphics unsupported.
- The accepted `840a67c0cb32b334ed54888321d5ca77e58117b0` proof adds
  `ORB-C8`: Orbit owns the libghostty viewport across client disconnection,
  routes vertical wheel input through authoritative mouse and alternate-screen
  modes or retained history, and returns scrolled primary output to the live
  area after terminal-producing key input.
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
- The accepted `9d6d2bb37f20ab4ad9e186c7bc715eabef43e757` proof replaces
  ORBS v1 with ORBS v2 and adds `ORB-C9`: revision-bound current-viewport
  selection, authoritative selected presentation, and bounded plain-text copy.
  A successful Finish freezes one immutable client-scoped value that survives
  later terminal output, resize or reflow, and active-screen transitions; a
  newer selection, client loss, or exit clears it. Venus consumes ORBS v2
  through `ven-4sn` without a v1 adapter or compatibility window.
- The accepted `3ee7c80005f3d2bbe81e539799327803716f6174` proof replaces
  the Orbit producer boundary with ORBS v3 and adds `ORB-C11`: bounded,
  normalized terminal clipboard writes are delivered once and in order to the
  attached client before the frame from the same PTY turn. Invalid, unsupported,
  detached, or pressured writes fail closed. Venus native delivery updates
  separately without a v2 adapter or compatibility window.
- The authoritative owner loop yields after each productive PTY read and caps
  the shared terminal-response and semantic-input backlog. A whole semantic
  input that exceeds the remaining bound receives a terminal failure and closes
  that client without partial admission; terminal-response overflow fails the
  session instead of silently losing an authoritative reply. The accepted
  `fdd55e1e8bf6b932c2949ea3c897601ff63c0743` pressure proof hardens `ORB-C3`
  and `ORB-C4` while advancing the transport evidence for `ORB-C7`.
- The accepted `9e9b136969dc583d365daa75402ec03c265080f1`
  lifecycle proof hardens `ORB-C1` and `ORB-C3` and proves `ORB-C7` for the
  local one-client boundary. Silent pre-attachment peers release the only slot
  after a bounded deadline, simultaneous peers receive deterministic Busy,
  reattachment receives the retained authoritative dimensions, and known-child
  exit stops owned process groups before a fixed final PTY-read budget. A
  continuously writing descendant cannot hold Orbit or its socket open.
- The accepted `8e3ba0beeb18157dc5a48c68c38812fa8d8fb779`
  terminal-authority proof hardens `ORB-C2`. Partial private-mode parser state
  survives client failure without export or reconstruction; Orbit alone sends
  the ordered terminal reply while detached, and canonical attachment cannot
  duplicate it. Ordered client-visible effects remain a separate transport gap.
- Venus is the accepted first external ORBS v1 and ORBF v1 consumer. Its
  accepted relationship is recorded at
  `8149002c7275a00db1c08fded171e09f249dd977`, and it pins Orbit
  `c905bf9610581747f1b07565814b501ca66cfaa6`; the later Orbit proof
  `838b67652c4df1979e599b9c401ee664ffac66bd` is compatible without a manifest
  migration because the protocol package and Cargo metadata are byte-identical.
