#![cfg(target_os = "linux")]

mod platform;
mod presentation;

use libghostty_vt::{
    Terminal, TerminalOptions, ffi,
    fmt::Format,
    focus,
    key::{
        Action as GhosttyKeyAction, Encoder as KeyEncoder, Event as GhosttyKeyEvent, Key,
        Mods as GhosttyModifiers,
    },
    mouse::{
        Action as GhosttyMouseAction, Button as GhosttyMouseButton, Encoder as MouseEncoder,
        EncoderSize as MouseEncoderSize, Event as GhosttyMouseEvent, Position as MousePosition,
    },
    paste,
    screen::Screen,
    selection::{FormatOptions, Selection},
    terminal::{ClipboardWrite, ClipboardWriteError, Mode, Point, PointCoordinate, ScrollViewport},
};
use orbit_protocol::session::{
    self, ClientMessage, ClipboardLocation, Failure, FailureCode, FocusEvent, KeyAction, KeyEvent,
    Modifiers, MouseAction, MouseButton, PhysicalKey, SelectionAction, ServerMessage, SurfaceSize,
    ViewportCell,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    env,
    error::Error,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

use platform::{Pty, PtyIo};
use presentation::{Extractor, OutputQueue, is_disconnect};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const MAX_PTY_WRITE_BYTES: usize = session::MAX_PASTE_BYTES + 16;
const MAX_EXIT_PTY_READS: usize = 4;
const MAX_PENDING_CLIPBOARD_WRITES: usize = 64;
const CLIENT_NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(1);

const INITIAL_SIZE: SurfaceSize = SurfaceSize {
    cols: 80,
    rows: 24,
    cell_width: 8,
    cell_height: 16,
    screen_width: 640,
    screen_height: 384,
    padding_top: 0,
    padding_bottom: 0,
    padding_left: 0,
    padding_right: 0,
};

struct Client {
    stream: UnixStream,
    input: Vec<u8>,
    output: OutputQueue,
    close_after_flush: bool,
    negotiation_deadline: Option<Instant>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SelectionState {
    anchor: Option<ViewportCell>,
    copied: Option<String>,
    visible: bool,
}

#[derive(Default)]
struct PendingClipboardWrites {
    enabled: bool,
    writes: VecDeque<(ClipboardLocation, String)>,
    overflowed: bool,
}

impl PendingClipboardWrites {
    fn capture(
        &mut self,
        write: ClipboardWrite<'_>,
    ) -> std::result::Result<(), ClipboardWriteError> {
        if !self.enabled {
            return Err(ClipboardWriteError::Denied);
        }
        if self.overflowed {
            return Err(ClipboardWriteError::Busy);
        }
        let write = raw_clipboard_write(&write);
        let location = clipboard_location_raw(write.location)?;
        if write.contents_len != 1 || write.contents.is_null() {
            return Err(ClipboardWriteError::Unsupported);
        }
        // SAFETY: libghostty guarantees one live entry for the callback when
        // contents_len is one.
        let content = unsafe { &*write.contents };
        if content.mime.len != b"text/plain".len() || content.mime.ptr.is_null() {
            return Err(ClipboardWriteError::Unsupported);
        }
        // SAFETY: libghostty owns this bounded callback-borrowed MIME string.
        let mime = unsafe { std::slice::from_raw_parts(content.mime.ptr, content.mime.len) };
        if mime != b"text/plain" {
            return Err(ClipboardWriteError::Unsupported);
        }
        if content.data.len == 0
            || content.data.len > session::MAX_COPY_BYTES
            || content.data.ptr.is_null()
        {
            return Err(ClipboardWriteError::InvalidData);
        }
        // SAFETY: libghostty owns this bounded callback-borrowed data string.
        let data = unsafe { std::slice::from_raw_parts(content.data.ptr, content.data.len) };
        let text = std::str::from_utf8(data).map_err(|_| ClipboardWriteError::InvalidData)?;
        if text.contains('\0') {
            return Err(ClipboardWriteError::InvalidData);
        }
        let bytes = self
            .writes
            .iter()
            .map(|(_, text)| text.len())
            .sum::<usize>();
        if self.writes.len() >= MAX_PENDING_CLIPBOARD_WRITES
            || bytes + text.len() > session::MAX_COPY_BYTES
        {
            self.overflowed = true;
            return Err(ClipboardWriteError::Busy);
        }
        self.writes.push_back((location, text.to_owned()));
        Ok(())
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

fn raw_clipboard_write<'a>(write: &'a ClipboardWrite<'_>) -> &'a ffi::ClipboardWrite {
    // SAFETY: In pinned libghostty-vt 0.2.1 ClipboardWrite is exactly one
    // raw-pointer field plus PhantomData. Transmute enforces pointer size, and
    // the callback guarantees that the pointed-to C request is live.
    let raw: *const ffi::ClipboardWrite = unsafe { std::mem::transmute(write.clone()) };
    // SAFETY: The callback owns a valid request for its full duration.
    unsafe { &*raw }
}

impl Client {
    fn close_when_flushed(&mut self) {
        self.input = Vec::new();
        self.close_after_flush = true;
    }

    fn finish_session(&mut self, code: i32) -> Result {
        if self.negotiation_deadline.is_none() {
            if !self.close_after_flush {
                let _ = self.output.push_message(&ServerMessage::Exited { code })?;
            }
            let _ = self.output.flush(&mut self.stream);
        }
        Ok(())
    }
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("orbit: {error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let mut arguments = env::args().skip(1);
    match arguments.next().as_deref() {
        Some("serve") => {
            let mut arguments: Vec<String> = arguments.collect();
            let socket = if arguments.first().is_some_and(|value| value != "--") {
                PathBuf::from(arguments.remove(0))
            } else {
                platform::default_socket_path()?
            };
            if arguments.first().is_some_and(|value| value == "--") {
                arguments.remove(0);
            }
            if arguments.is_empty() {
                arguments.push(env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()));
            }
            run_server(&socket, &arguments)
        }
        Some("client") => {
            let socket = arguments
                .next()
                .map(PathBuf::from)
                .map_or_else(|| platform::default_socket_path(), Ok)?;
            run_client(&socket)
        }
        _ => Err("usage: yazelix-orbit serve [SOCKET] [-- COMMAND ...] | client [SOCKET]".into()),
    }
}

fn run_server(socket: &Path, command: &[String]) -> Result<i32> {
    platform::install_shutdown_signals()?;
    let (listener, _socket_guard) = platform::create_listener(socket)?;
    let mut size = INITIAL_SIZE;
    let mut pty = Pty::spawn(command, size)?;
    platform::ignore_broken_pipe()?;

    let writes = Rc::new(RefCell::new(VecDeque::<u8>::new()));
    let response_sink = Rc::clone(&writes);
    let response_overflow = Rc::new(Cell::new(false));
    let overflow_sink = Rc::clone(&response_overflow);
    let mut terminal = Terminal::new(TerminalOptions {
        cols: size.cols,
        rows: size.rows,
        max_scrollback: 1_000,
    })?;
    terminal.resize(size.cols, size.rows, size.cell_width, size.cell_height)?;
    terminal.on_pty_write(move |_, bytes| {
        if !queue_pty_write(&mut response_sink.borrow_mut(), bytes) {
            overflow_sink.set(true);
        }
    })?;
    let clipboard_writes = Rc::new(RefCell::new(PendingClipboardWrites::default()));
    let clipboard_sink = Rc::clone(&clipboard_writes);
    terminal.on_clipboard_write(move |_, write| clipboard_sink.borrow_mut().capture(write))?;

    let mut extractor = Extractor::new()?;
    let mut revision = 0;
    let mut client: Option<Client> = None;
    let mut selection = SelectionState::default();
    let mut pty_open = true;
    loop {
        clipboard_writes.borrow_mut().enabled = client.as_ref().is_some_and(|client| {
            client.negotiation_deadline.is_none() && !client.close_after_flush
        });
        if platform::termination_requested() {
            return Ok(0);
        }
        if let Some(status) = pty.try_wait()? {
            pty.stop_and_reap();
            let changed = clear_selection(&terminal, &mut selection, true)?
                | drain_exited_pty(&mut pty, &mut terminal, &mut selection)?;
            fail_on_pty_write_overflow(&response_overflow)?;
            if !publish_clipboard_writes(&mut client, &clipboard_writes)? {
                disconnect_client(&mut client, &terminal, &mut selection, &mut revision)?;
            }
            if changed {
                revision = next_revision(revision)?;
                if !publish_frame(&mut client, &mut extractor, revision, &terminal)? {
                    disconnect_client(&mut client, &terminal, &mut selection, &mut revision)?;
                }
            }
            let code = status.code().unwrap_or(1);
            if let Some(client) = &mut client {
                client.finish_session(code)?;
            }
            return Ok(code);
        }
        if client
            .as_ref()
            .and_then(|client| client.negotiation_deadline)
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            disconnect_client(&mut client, &terminal, &mut selection, &mut revision)?;
        }

        let readiness = platform::poll(
            &listener,
            pty_open.then(|| (&pty, !writes.borrow().is_empty())),
            client
                .as_ref()
                .filter(|client| !client.close_after_flush)
                .map(|client| &client.stream),
        )?;
        let pty_was_open = pty_open;

        if readiness.listener {
            accept_clients(&listener, &mut client)?;
        }
        if readiness.pty_read {
            let (open, changed) = read_pty_turn(&mut pty, &mut terminal, &mut selection)?;
            fail_on_pty_write_overflow(&response_overflow)?;
            pty_open = open;
            if !publish_clipboard_writes(&mut client, &clipboard_writes)? {
                disconnect_client(&mut client, &terminal, &mut selection, &mut revision)?;
            }
            if changed {
                revision = next_revision(revision)?;
                if !publish_frame(&mut client, &mut extractor, revision, &terminal)? {
                    disconnect_client(&mut client, &terminal, &mut selection, &mut revision)?;
                }
            }
        }
        if readiness.pty_write && pty_open {
            pty_open = flush_pty(&mut pty, &mut writes.borrow_mut())?;
        }
        if pty_was_open && !pty_open {
            discard_pty_writes(&writes);
        }
        if readiness.client
            && !read_client(
                client.as_mut().expect("client existed when poll began"),
                &mut terminal,
                pty_open.then_some(&pty),
                &mut size,
                &writes,
                &mut extractor,
                &mut revision,
                &mut selection,
            )?
        {
            disconnect_client(&mut client, &terminal, &mut selection, &mut revision)?;
        }
        if let Some(active) = &mut client {
            let disconnect = !active.output.flush(&mut active.stream)?
                || active.close_after_flush && active.output.is_empty();
            if disconnect {
                disconnect_client(&mut client, &terminal, &mut selection, &mut revision)?;
            }
        }
    }
}

fn discard_pty_writes(writes: &RefCell<VecDeque<u8>>) {
    drop(writes.take());
}

fn queue_pty_write(writes: &mut VecDeque<u8>, bytes: &[u8]) -> bool {
    if writes.len().saturating_add(bytes.len()) > MAX_PTY_WRITE_BYTES {
        return false;
    }
    writes.extend(bytes);
    true
}

fn fail_on_pty_write_overflow(overflow: &Cell<bool>) -> Result {
    if overflow.get() {
        return Err("PTY response exceeded the bounded write queue".into());
    }
    Ok(())
}

fn clipboard_location_raw(
    location: ffi::ClipboardLocation::Type,
) -> std::result::Result<ClipboardLocation, ClipboardWriteError> {
    match location {
        ffi::ClipboardLocation::STANDARD => Ok(ClipboardLocation::Standard),
        ffi::ClipboardLocation::SELECTION => Ok(ClipboardLocation::Selection),
        ffi::ClipboardLocation::PRIMARY => Ok(ClipboardLocation::Primary),
        _ => Err(ClipboardWriteError::Unsupported),
    }
}

fn publish_clipboard_writes(
    active: &mut Option<Client>,
    pending: &RefCell<PendingClipboardWrites>,
) -> Result<bool> {
    let mut pending = pending.borrow_mut();
    let Some(client) = active
        .as_mut()
        .filter(|client| client.negotiation_deadline.is_none() && !client.close_after_flush)
    else {
        pending.clear();
        return Ok(true);
    };
    if pending.overflowed {
        pending.clear();
        return Ok(false);
    }
    while let Some((location, text)) = pending.writes.pop_front() {
        if !client
            .output
            .push_message(&ServerMessage::ClipboardWrite { location, text })?
        {
            pending.clear();
            return Ok(false);
        }
    }
    Ok(true)
}

fn run_client(socket: &Path) -> Result<i32> {
    let stream = UnixStream::connect(socket)?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    write_client_message(
        &mut writer,
        &ClientMessage::Hello {
            minimum_version: session::VERSION,
            maximum_version: session::VERSION,
        },
    )?;
    loop {
        let message = read_server_message(&mut reader)?.ok_or("server closed during attachment")?;
        print_server_message(&message);
        match message {
            ServerMessage::Attached { .. } => break,
            ServerMessage::Busy => return Ok(2),
            ServerMessage::Incompatible { .. } => return Ok(3),
            ServerMessage::Failure(_) => return Ok(4),
            _ => {}
        }
    }

    for line in io::stdin().lock().lines() {
        write_client_message(&mut writer, &ClientMessage::Paste(line?.into_bytes()))?;
        write_client_message(
            &mut writer,
            &ClientMessage::Key(KeyEvent {
                action: KeyAction::Press,
                key: PhysicalKey::ENTER,
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: None,
                unshifted_codepoint: None,
            }),
        )?;
        let mut accepted = 0;
        while accepted < 2 {
            let Some(message) = read_server_message(&mut reader)? else {
                return Ok(0);
            };
            print_server_message(&message);
            match message {
                ServerMessage::Accepted => accepted += 1,
                ServerMessage::Failure(_) => return Ok(4),
                ServerMessage::Exited { code } => return Ok(code),
                _ => {}
            }
        }
    }
    Ok(0)
}

fn write_client_message(writer: &mut impl Write, message: &ClientMessage) -> Result {
    writer.write_all(&session::encode_client_message(message)?)?;
    Ok(())
}

fn read_server_message(reader: &mut impl Read) -> Result<Option<ServerMessage>> {
    let mut header = [0; session::HEADER_BYTES];
    if reader.read(&mut header[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..])?;
    let length =
        session::server_message_len(&header)?.expect("complete header has a server-message length");
    let mut message = Vec::with_capacity(length);
    message.extend_from_slice(&header);
    message.resize(length, 0);
    reader.read_exact(&mut message[session::HEADER_BYTES..])?;
    Ok(Some(session::decode_server_message(&message)?))
}

fn print_server_message(message: &ServerMessage) {
    match message {
        ServerMessage::Frame(frame) => println!(
            "FRAME revision={} size={}x{} title={:?}",
            frame.revision, frame.dimensions.cols, frame.dimensions.rows, frame.title
        ),
        message => println!("{message:?}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn read_client(
    client: &mut Client,
    terminal: &mut Terminal<'static, '_>,
    pty: Option<&Pty>,
    size: &mut SurfaceSize,
    writes: &RefCell<VecDeque<u8>>,
    extractor: &mut Extractor,
    revision: &mut u64,
    selection: &mut SelectionState,
) -> Result<bool> {
    if client.close_after_flush {
        return Ok(true);
    }
    let mut bytes = [0; 1024];
    let read = match client.stream.read(&mut bytes) {
        Ok(0) => return Ok(false),
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(true),
        Err(error) if is_disconnect(&error) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    client.input.extend_from_slice(&bytes[..read]);
    loop {
        let length = match session::client_message_len(&client.input) {
            Ok(Some(length)) if client.input.len() >= length => length,
            Ok(_) => return Ok(true),
            Err(error) => {
                if !queue_failure(client, FailureCode::Protocol, error.to_string())? {
                    return Ok(false);
                }
                client.close_when_flushed();
                return Ok(true);
            }
        };
        let message = match session::decode_client_message(&client.input[..length]) {
            Ok(message) => message,
            Err(error) => {
                if !queue_failure(client, FailureCode::Protocol, error.to_string())? {
                    return Ok(false);
                }
                client.close_when_flushed();
                return Ok(true);
            }
        };
        drop(client.input.drain(..length));
        let keep = handle_client_message(
            client, message, terminal, pty, size, writes, extractor, revision, selection,
        )?;
        if !keep {
            return Ok(false);
        }
        if client.close_after_flush {
            return Ok(true);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_client_message(
    client: &mut Client,
    message: ClientMessage,
    terminal: &mut Terminal<'static, '_>,
    pty: Option<&Pty>,
    size: &mut SurfaceSize,
    writes: &RefCell<VecDeque<u8>>,
    extractor: &mut Extractor,
    revision: &mut u64,
    selection: &mut SelectionState,
) -> Result<bool> {
    if client.negotiation_deadline.is_some() {
        return match message {
            ClientMessage::Hello {
                minimum_version,
                maximum_version,
            } if minimum_version <= session::VERSION && maximum_version >= session::VERSION => {
                client.negotiation_deadline = None;
                if !client.output.push_message(&ServerMessage::Attached {
                    version: session::VERSION,
                })? {
                    return Ok(false);
                }
                queue_frame(client, extractor, *revision, terminal, true)
            }
            ClientMessage::Hello { .. } => {
                let queued = client.output.push_message(&ServerMessage::Incompatible {
                    minimum_version: session::VERSION,
                    maximum_version: session::VERSION,
                })?;
                client.close_when_flushed();
                Ok(queued)
            }
            _ => {
                let queued = queue_failure(
                    client,
                    FailureCode::Protocol,
                    "first client message must be Hello".into(),
                )?;
                client.close_when_flushed();
                Ok(queued)
            }
        };
    }

    if matches!(&message, ClientMessage::Hello { .. }) {
        let queued = queue_failure(
            client,
            FailureCode::Protocol,
            "Hello may only be sent once".into(),
        )?;
        client.close_when_flushed();
        return Ok(queued);
    }
    let Some(pty) = pty else {
        return queue_failure(client, FailureCode::Terminal, "PTY is closed".into());
    };
    if let ClientMessage::Selection(action) = message {
        return handle_selection(
            client, action, terminal, *size, extractor, revision, selection,
        );
    }
    let message = match message {
        ClientMessage::Resize(surface) => {
            clear_selection(terminal, selection, false)?;
            pty.resize(surface)?;
            terminal.resize(
                surface.cols,
                surface.rows,
                surface.cell_width,
                surface.cell_height,
            )?;
            *size = surface;
            *revision = next_revision(*revision)?;
            if !client.output.push_message(&ServerMessage::Accepted)? {
                return Ok(false);
            }
            return queue_frame(client, extractor, *revision, terminal, false);
        }
        message => message,
    };
    let return_live = matches!(&message, ClientMessage::Key(_));
    let message = match message {
        ClientMessage::Mouse(input)
            if matches!(input.button, Some(MouseButton::Four | MouseButton::Five))
                && !terminal.is_mouse_tracking()? =>
        {
            let delta = if input.button == Some(MouseButton::Four) {
                -1
            } else {
                1
            };
            if terminal.active_screen()? == Screen::Alternate && terminal.mode(Mode::ALT_SCROLL)? {
                ClientMessage::Key(KeyEvent {
                    action: KeyAction::Press,
                    key: if delta < 0 {
                        PhysicalKey::ARROW_UP
                    } else {
                        PhysicalKey::ARROW_DOWN
                    },
                    modifiers: Modifiers::empty(),
                    consumed_modifiers: Modifiers::empty(),
                    composing: false,
                    text: None,
                    unshifted_codepoint: None,
                })
            } else {
                clear_selection(terminal, selection, false)?;
                terminal.scroll_viewport(ScrollViewport::Delta(delta));
                *revision = next_revision(*revision)?;
                if !client.output.push_message(&ServerMessage::Accepted)? {
                    return Ok(false);
                }
                return queue_frame(client, extractor, *revision, terminal, false);
            }
        }
        message => message,
    };
    let encoded = match encode_input(terminal, *size, message) {
        Ok(encoded) => encoded,
        Err(error) => {
            return queue_failure(client, FailureCode::Terminal, error.to_string());
        }
    };
    if !queue_pty_write(&mut writes.borrow_mut(), &encoded) {
        let queued = queue_failure(
            client,
            FailureCode::Terminal,
            "PTY input queue is full".into(),
        )?;
        client.close_when_flushed();
        return Ok(queued);
    }
    let returned_live =
        if return_live && !encoded.is_empty() && terminal.active_screen()? == Screen::Primary {
            let scrollbar = terminal.scrollbar()?;
            if scrollbar.offset.saturating_add(scrollbar.len) < scrollbar.total {
                clear_selection(terminal, selection, false)?;
                terminal.scroll_viewport(ScrollViewport::Bottom);
                *revision = next_revision(*revision)?;
                true
            } else {
                false
            }
        } else {
            false
        };
    if !client.output.push_message(&ServerMessage::Accepted)? {
        return Ok(false);
    }
    if returned_live {
        return queue_frame(client, extractor, *revision, terminal, false);
    }
    Ok(true)
}

fn handle_selection(
    client: &mut Client,
    action: SelectionAction,
    terminal: &Terminal<'static, '_>,
    size: SurfaceSize,
    extractor: &mut Extractor,
    revision: &mut u64,
    state: &mut SelectionState,
) -> Result<bool> {
    let (anchor, cell, finish) = match action {
        SelectionAction::Begin {
            frame_revision,
            cell,
        } => {
            if frame_revision != *revision {
                return queue_failure(
                    client,
                    FailureCode::InvalidInput,
                    "selection frame revision is stale".into(),
                );
            }
            (cell, cell, false)
        }
        SelectionAction::Update { cell } => {
            let Some(anchor) = state.anchor else {
                return queue_failure(
                    client,
                    FailureCode::InvalidInput,
                    "selection update has no active selection".into(),
                );
            };
            (anchor, cell, false)
        }
        SelectionAction::Finish { cell } => {
            let Some(anchor) = state.anchor else {
                return queue_failure(
                    client,
                    FailureCode::InvalidInput,
                    "selection finish has no active selection".into(),
                );
            };
            (anchor, cell, true)
        }
        SelectionAction::Copy => {
            return match &state.copied {
                Some(text) => client
                    .output
                    .push_message(&ServerMessage::CopiedText(text.clone())),
                None => queue_failure(
                    client,
                    FailureCode::InvalidInput,
                    "no finished selection to copy".into(),
                ),
            };
        }
    };
    if cell.x >= size.cols || cell.y >= size.rows {
        return queue_failure(
            client,
            FailureCode::InvalidInput,
            "selection cell is outside the current viewport".into(),
        );
    }
    if !client.output.can_push_result_frame() {
        return queue_failure(
            client,
            FailureCode::Terminal,
            "client output queue cannot admit selection frame".into(),
        );
    }
    let next = match next_revision(*revision) {
        Ok(next) => next,
        Err(error) => return queue_failure(client, FailureCode::Terminal, error.to_string()),
    };

    let selected = match viewport_selection(terminal, anchor, cell) {
        Ok(selected) => selected,
        Err(error) => return queue_failure(client, FailureCode::Terminal, error.to_string()),
    };
    let copied = if finish {
        match format_selection(terminal, &selected) {
            Ok(text) => Some(text),
            Err(error) => {
                return queue_failure(client, FailureCode::Terminal, error.to_string());
            }
        }
    } else {
        None
    };
    if let Err(error) = terminal.set_selection(Some(&selected)) {
        return queue_failure(client, FailureCode::Terminal, error.to_string());
    }

    state.anchor = (!finish).then_some(anchor);
    if finish || matches!(action, SelectionAction::Begin { .. }) {
        state.copied = copied;
    }
    state.visible = true;
    *revision = next;
    if !client.output.push_message(&ServerMessage::Accepted)? {
        return Ok(false);
    }
    queue_frame(client, extractor, *revision, terminal, false)
}

fn viewport_selection<'terminal>(
    terminal: &'terminal Terminal<'_, '_>,
    start: ViewportCell,
    end: ViewportCell,
) -> Result<Selection<'terminal>> {
    let point = |cell: ViewportCell| {
        terminal.grid_ref(Point::Viewport(PointCoordinate {
            x: cell.x,
            y: u32::from(cell.y),
        }))
    };
    Ok(Selection::new(point(start)?, point(end)?, false))
}

fn format_selection(terminal: &Terminal<'_, '_>, selected: &Selection<'_>) -> Result<String> {
    let options = || {
        FormatOptions::new()
            .with_emit_format(Format::Plain)
            .with_unwrap(true)
            .with_trim(true)
            .with_selection(selected)
    };
    let required = match terminal.format_selection_buf(options(), &mut []) {
        Ok(Some(written)) => written,
        Ok(None) => return Err("terminal returned no selection text".into()),
        Err(libghostty_vt::error::Error::OutOfSpace { required }) => required,
        Err(error) => return Err(error.into()),
    };
    if required > session::MAX_COPY_BYTES {
        return Err("selection text exceeds the copy limit".into());
    }
    let mut bytes = vec![0; required];
    let Some(written) = terminal.format_selection_buf(options(), &mut bytes)? else {
        return Err("terminal returned no selection text".into());
    };
    bytes.truncate(written);
    Ok(String::from_utf8(bytes)?)
}

fn clear_selection(
    terminal: &Terminal<'_, '_>,
    state: &mut SelectionState,
    forget_copy: bool,
) -> Result<bool> {
    let changed = state.visible;
    if changed {
        terminal.set_selection(None)?;
    }
    state.anchor = None;
    state.visible = false;
    if forget_copy {
        state.copied = None;
    }
    Ok(changed)
}

fn apply_pty_output(
    terminal: &mut Terminal<'_, '_>,
    state: &mut SelectionState,
    bytes: &[u8],
) -> Result {
    clear_selection(terminal, state, false)?;
    terminal.vt_write(bytes);
    Ok(())
}

fn queue_failure(client: &mut Client, code: FailureCode, mut detail: String) -> Result<bool> {
    while detail.len() > session::MAX_FAILURE_BYTES {
        detail.pop();
    }
    client
        .output
        .push_message(&ServerMessage::Failure(Failure { code, detail }))
}

fn encode_input(
    terminal: &Terminal<'_, '_>,
    size: SurfaceSize,
    input: ClientMessage,
) -> Result<Vec<u8>> {
    match input {
        ClientMessage::Key(input) => {
            let key = Key::try_from(u32::from(input.key.raw()))
                .map_err(|value| format!("unsupported physical key {value}"))?;
            let mut event = GhosttyKeyEvent::new()?;
            event
                .set_action(match input.action {
                    KeyAction::Press => GhosttyKeyAction::Press,
                    KeyAction::Release => GhosttyKeyAction::Release,
                    KeyAction::Repeat => GhosttyKeyAction::Repeat,
                })
                .set_key(key)
                .set_mods(ghostty_modifiers(input.modifiers)?)
                .set_consumed_mods(ghostty_modifiers(input.consumed_modifiers)?)
                .set_composing(input.composing)
                .set_utf8(input.text);
            if let Some(codepoint) = input.unshifted_codepoint {
                event.set_unshifted_codepoint(codepoint);
            }
            let mut encoder = KeyEncoder::new()?;
            encoder.set_options_from_terminal(terminal);
            let mut output = Vec::new();
            encoder.encode_to_vec(&event, &mut output)?;
            Ok(output)
        }
        ClientMessage::Mouse(input) => {
            let is_wheel = matches!(
                input.button,
                Some(MouseButton::Four | MouseButton::Five | MouseButton::Six | MouseButton::Seven)
            );
            let mut event = GhosttyMouseEvent::new()?;
            event
                .set_action(match input.action {
                    MouseAction::Press => GhosttyMouseAction::Press,
                    MouseAction::Release => GhosttyMouseAction::Release,
                    MouseAction::Motion => GhosttyMouseAction::Motion,
                })
                .set_button(input.button.map(ghostty_mouse_button))
                .set_mods(ghostty_modifiers(input.modifiers)?)
                .set_position(MousePosition {
                    x: input.x,
                    y: input.y,
                });
            let mut encoder = MouseEncoder::new()?;
            encoder
                .set_options_from_terminal(terminal)
                .set_size(mouse_size(size))
                .set_any_button_pressed(
                    input.button.is_some() && input.action != MouseAction::Release && !is_wheel,
                );
            let mut output = Vec::new();
            encoder.encode_to_vec(&event, &mut output)?;
            Ok(output)
        }
        ClientMessage::Focus(event) => {
            if !terminal.mode(Mode::FOCUS_EVENT)? {
                return Ok(Vec::new());
            }
            let event = match event {
                FocusEvent::Gained => focus::Event::Gained,
                FocusEvent::Lost => focus::Event::Lost,
            };
            let mut output = vec![0; 8];
            let written = event.encode(&mut output)?;
            output.truncate(written);
            Ok(output)
        }
        ClientMessage::Paste(mut data) => {
            let bracketed = terminal.mode(Mode::BRACKETED_PASTE)?;
            let mut output = vec![0; data.len() + 16];
            let written = paste::encode(&mut data, bracketed, &mut output)?;
            output.truncate(written);
            Ok(output)
        }
        ClientMessage::Hello { .. } | ClientMessage::Resize(_) | ClientMessage::Selection(_) => {
            Err("message is not terminal input".into())
        }
    }
}

fn mouse_size(size: SurfaceSize) -> MouseEncoderSize {
    MouseEncoderSize {
        screen_width: size.screen_width,
        screen_height: size.screen_height,
        cell_width: size.cell_width,
        cell_height: size.cell_height,
        padding_top: size.padding_top,
        padding_bottom: size.padding_bottom,
        padding_right: size.padding_right,
        padding_left: size.padding_left,
    }
}

fn ghostty_modifiers(modifiers: Modifiers) -> Result<GhosttyModifiers> {
    GhosttyModifiers::from_bits(modifiers.bits()).ok_or_else(|| "invalid key modifiers".into())
}

fn ghostty_mouse_button(button: MouseButton) -> GhosttyMouseButton {
    match button {
        MouseButton::Unknown => GhosttyMouseButton::Unknown,
        MouseButton::Left => GhosttyMouseButton::Left,
        MouseButton::Middle => GhosttyMouseButton::Middle,
        MouseButton::Right => GhosttyMouseButton::Right,
        MouseButton::Four => GhosttyMouseButton::Four,
        MouseButton::Five => GhosttyMouseButton::Five,
        MouseButton::Six => GhosttyMouseButton::Six,
        MouseButton::Seven => GhosttyMouseButton::Seven,
        MouseButton::Eight => GhosttyMouseButton::Eight,
        MouseButton::Nine => GhosttyMouseButton::Nine,
        MouseButton::Ten => GhosttyMouseButton::Ten,
        MouseButton::Eleven => GhosttyMouseButton::Eleven,
    }
}

fn drain_exited_pty(
    pty: &mut Pty,
    terminal: &mut Terminal<'_, '_>,
    selection: &mut SelectionState,
) -> Result<bool> {
    let mut changed = false;
    for _ in 0..MAX_EXIT_PTY_READS {
        let (open, read) = read_pty_turn(pty, terminal, selection)?;
        changed |= read;
        if !open || !read {
            break;
        }
    }
    Ok(changed)
}

fn read_pty_turn(
    pty: &mut Pty,
    terminal: &mut Terminal<'_, '_>,
    selection: &mut SelectionState,
) -> Result<(bool, bool)> {
    let mut bytes = [0; 8192];
    match pty.read(&mut bytes)? {
        PtyIo::Ready(0) | PtyIo::Closed => Ok((false, false)),
        PtyIo::Ready(read) => {
            apply_pty_output(terminal, selection, &bytes[..read])?;
            Ok((true, true))
        }
        PtyIo::Blocked => Ok((true, false)),
    }
}

fn flush_pty(pty: &mut Pty, writes: &mut VecDeque<u8>) -> Result<bool> {
    while !writes.is_empty() {
        let bytes = {
            let (first, second) = writes.as_slices();
            if first.is_empty() { second } else { first }
        };
        match pty.write(bytes)? {
            PtyIo::Ready(written) => {
                drop(writes.drain(..written));
            }
            PtyIo::Blocked => return Ok(true),
            PtyIo::Closed => return Ok(false),
        }
    }
    Ok(true)
}

fn accept_clients(listener: &UnixListener, active: &mut Option<Client>) -> Result {
    loop {
        match listener.accept() {
            Ok((mut stream, _)) if active.is_some() => {
                let busy = session::encode_server_message(&ServerMessage::Busy)?;
                let _ = stream.write_all(&busy);
            }
            Ok((stream, _)) => {
                stream.set_nonblocking(true)?;
                *active = Some(Client {
                    stream,
                    input: Vec::new(),
                    output: OutputQueue::default(),
                    close_after_flush: false,
                    negotiation_deadline: Some(Instant::now() + CLIENT_NEGOTIATION_TIMEOUT),
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

fn next_revision(revision: u64) -> Result<u64> {
    revision
        .checked_add(1)
        .ok_or_else(|| "presentation revision exhausted".into())
}

fn disconnect_client(
    client: &mut Option<Client>,
    terminal: &Terminal<'_, '_>,
    selection: &mut SelectionState,
    revision: &mut u64,
) -> Result {
    *client = None;
    if clear_selection(terminal, selection, true)? {
        *revision = next_revision(*revision)?;
    }
    Ok(())
}

fn publish_frame(
    active: &mut Option<Client>,
    extractor: &mut Extractor,
    revision: u64,
    terminal: &Terminal<'static, '_>,
) -> Result<bool> {
    if let Some(client) = active
        .as_mut()
        .filter(|client| client.negotiation_deadline.is_none() && !client.close_after_flush)
        && !queue_frame(client, extractor, revision, terminal, false)?
    {
        return Ok(false);
    }
    Ok(true)
}

fn queue_frame(
    client: &mut Client,
    extractor: &mut Extractor,
    revision: u64,
    terminal: &Terminal<'static, '_>,
    initial: bool,
) -> Result<bool> {
    let frame = match extractor.frame(revision, terminal) {
        Ok(frame) => frame,
        Err(error) => {
            eprintln!("orbit: presentation client disconnected: {error}");
            return Ok(false);
        }
    };
    if initial {
        client.output.push_initial_frame(frame)
    } else {
        client.output.push_frame(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Instant};

    fn terminal() -> Result<Terminal<'static, 'static>> {
        terminal_with_scrollback(0)
    }

    fn terminal_with_scrollback(max_scrollback: usize) -> Result<Terminal<'static, 'static>> {
        let size = INITIAL_SIZE;
        let mut terminal = Terminal::new(TerminalOptions {
            cols: size.cols,
            rows: size.rows,
            max_scrollback,
        })?;
        terminal.resize(size.cols, size.rows, size.cell_width, size.cell_height)?;
        Ok(terminal)
    }

    fn attached_client() -> Result<(Client, UnixStream)> {
        let (stream, peer) = UnixStream::pair()?;
        stream.set_nonblocking(true)?;
        Ok((
            Client {
                stream,
                input: Vec::new(),
                output: OutputQueue::default(),
                close_after_flush: false,
                negotiation_deadline: None,
            },
            peer,
        ))
    }

    fn closing_client() -> Result<(Client, UnixStream, Vec<u8>)> {
        let (mut client, peer) = attached_client()?;
        let failure = ServerMessage::Failure(Failure {
            code: FailureCode::Protocol,
            detail: "terminal".into(),
        });
        let expected = session::encode_server_message(&failure)?;
        assert!(client.output.push_message(&failure)?);
        client.close_when_flushed();
        Ok((client, peer, expected))
    }

    fn wheel(button: MouseButton) -> ClientMessage {
        ClientMessage::Mouse(session::MouseEvent {
            action: MouseAction::Press,
            button: Some(button),
            modifiers: Modifiers::empty(),
            x: 1.0,
            y: 1.0,
        })
    }

    fn flush_message(client: &mut Client, peer: &mut UnixStream) -> Result<ServerMessage> {
        assert!(client.output.flush(&mut client.stream)?);
        read_server_message(peer)?.ok_or_else(|| "client output closed".into())
    }

    fn fill_output(output: &mut OutputQueue) -> Result {
        let failure = ServerMessage::Failure(Failure {
            code: FailureCode::Terminal,
            detail: "x".repeat(session::MAX_FAILURE_BYTES),
        });
        while output.push_message(&failure)? {}
        while output.push_message(&ServerMessage::Accepted)? {}
        Ok(())
    }

    #[test]
    fn clipboard_capture_is_attached_typed_and_bounded() -> Result {
        let pending = Rc::new(RefCell::new(PendingClipboardWrites::default()));
        let sink = Rc::clone(&pending);
        let last_error = Rc::new(Cell::new(None));
        let error_sink = Rc::clone(&last_error);
        let mut terminal = terminal()?;
        terminal.on_clipboard_write(move |_, write| {
            let result = sink.borrow_mut().capture(write);
            error_sink.set(result.err());
            result
        })?;

        terminal.vt_write(b"\x1b]52;c;ZGV0YWNoZWQ=\x1b\\");
        assert!(pending.borrow().writes.is_empty());

        pending.borrow_mut().enabled = true;
        terminal.vt_write(
            b"\x1b]52;c;\x1b\\\x1b]52;c;YQBi\x1b\\\x1b]52;c;/w==\x1b\\\x1b]52;c;b25l\x1b\\\x1b]52;s;dHdv\x1b\\\x1b]52;p;dGhyZWU=\x1b\\",
        );
        assert_eq!(
            pending.borrow().writes,
            VecDeque::from([
                (ClipboardLocation::Standard, "one".into()),
                (ClipboardLocation::Selection, "two".into()),
                (ClipboardLocation::Primary, "three".into()),
            ])
        );

        let mut oversized = b"\x1b]52;c;".to_vec();
        oversized.extend_from_slice(&b"eHh4".repeat(session::MAX_COPY_BYTES / 3));
        oversized.extend_from_slice(b"eHg=\x1b\\");
        terminal.vt_write(&oversized);
        assert_eq!(last_error.get(), Some(ClipboardWriteError::InvalidData));
        assert_eq!(pending.borrow().writes.len(), 3);
        assert_eq!(
            clipboard_location_raw(u32::MAX),
            Err(ClipboardWriteError::Unsupported)
        );

        let captured = pending.borrow().writes.len();
        for _ in captured..=MAX_PENDING_CLIPBOARD_WRITES {
            terminal.vt_write(b"\x1b]52;c;eA==\x1b\\");
        }
        assert!(pending.borrow().overflowed);

        pending.borrow_mut().clear();
        {
            let mut pending = pending.borrow_mut();
            pending.enabled = true;
            pending.writes.push_back((
                ClipboardLocation::Standard,
                "x".repeat(session::MAX_COPY_BYTES - 1),
            ));
        }
        terminal.vt_write(b"\x1b]52;c;eHg=\x1b\\\x1b]52;c;eA==\x1b\\");
        assert_eq!(last_error.get(), Some(ClipboardWriteError::Busy));
        assert_eq!(pending.borrow().writes.len(), 1);
        assert!(pending.borrow().overflowed);

        pending.borrow_mut().clear();
        terminal.vt_write(b"\x1b]52;p;bm90LXJlcGxheWVk\x1b\\");
        assert_eq!(last_error.get(), Some(ClipboardWriteError::Denied));
        assert!(pending.borrow().writes.is_empty());
        Ok(())
    }

    #[test]
    fn clipboard_output_pressure_disconnects_without_retaining_effects() -> Result {
        let (mut client, _peer) = attached_client()?;
        fill_output(&mut client.output)?;
        let pending = RefCell::new(PendingClipboardWrites {
            enabled: true,
            writes: VecDeque::from([(ClipboardLocation::Standard, "copy".into())]),
            overflowed: false,
        });
        let mut active = Some(client);

        assert!(!publish_clipboard_writes(&mut active, &pending)?);
        assert!(pending.borrow().writes.is_empty());
        Ok(())
    }

    #[test]
    fn authoritative_selection_rejects_stale_input_and_freezes_copy() -> Result {
        let mut text = terminal_with_scrollback(100)?;
        text.vt_write(b"A\r\n\r\nZ\r\nB\x1b[3;1H\x1b[X");
        let cell = |(x, y)| ViewportCell { x, y };
        for (start, end, expected) in [
            ((0, 0), (0, 0), "A"),
            ((0, 0), (0, 3), "A\n\n\nB"),
            ((0, 3), (0, 0), "A\n\n\nB"),
            ((0, 1), (0, 1), ""),
            ((0, 2), (0, 2), ""),
        ] {
            let selected = viewport_selection(&text, cell(start), cell(end))?;
            assert_eq!(format_selection(&text, &selected)?, expected);
        }

        for line in 0..30 {
            text.vt_write(format!("history-{line:02}\r\n").as_bytes());
        }
        text.scroll_viewport(ScrollViewport::Top);
        let history = text.scrollbar()?;
        assert_eq!(history.offset, 0);
        assert!(history.offset + history.len < history.total);
        let retained = viewport_selection(&text, cell((0, 0)), cell((0, 0)))?;
        assert_eq!(format_selection(&text, &retained)?, "A");

        let (mut client, mut peer) = attached_client()?;
        let pty = Pty::spawn(&["/bin/sh".into()], INITIAL_SIZE)?;
        let mut terminal = terminal()?;
        terminal.vt_write("alpha 界\r\n".as_bytes());
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut extractor = Extractor::new()?;
        let mut revision = 1;
        let mut selection = SelectionState::default();

        macro_rules! reject {
            ($action:expr) => {{
                let unchanged = revision;
                assert!(handle_client_message(
                    &mut client,
                    ClientMessage::Selection($action),
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut extractor,
                    &mut revision,
                    &mut selection,
                )?);
                assert_eq!(revision, unchanged);
                assert_eq!(selection, SelectionState::default());
                assert!(matches!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::Failure(Failure {
                        code: FailureCode::InvalidInput,
                        ..
                    })
                ));
            }};
        }
        macro_rules! select {
            ($action:expr) => {{
                assert!(handle_client_message(
                    &mut client,
                    ClientMessage::Selection($action),
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut extractor,
                    &mut revision,
                    &mut selection,
                )?);
                assert_eq!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::Accepted
                );
                let ServerMessage::Frame(frame) = flush_message(&mut client, &mut peer)? else {
                    return Err("selection did not publish a frame".into());
                };
                assert_eq!(frame.revision, revision);
                assert!(frame.rows[0].cells[0].style.selected);
            }};
        }
        reject!(session::SelectionAction::Begin {
            frame_revision: 0,
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        reject!(session::SelectionAction::Update {
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        reject!(session::SelectionAction::Begin {
            frame_revision: revision,
            cell: session::ViewportCell { x: size.cols, y: 0 },
        });

        select!(session::SelectionAction::Begin {
            frame_revision: revision,
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        apply_pty_output(&mut terminal, &mut selection, b"!")?;
        assert_eq!(selection, SelectionState::default());
        reject!(session::SelectionAction::Finish {
            cell: session::ViewportCell { x: 6, y: 0 },
        });

        select!(session::SelectionAction::Begin {
            frame_revision: revision,
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        select!(session::SelectionAction::Update {
            cell: session::ViewportCell { x: 6, y: 0 },
        });
        select!(session::SelectionAction::Finish {
            cell: session::ViewportCell { x: 6, y: 0 },
        });

        assert!(handle_client_message(
            &mut client,
            ClientMessage::Selection(session::SelectionAction::Copy),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert_eq!(
            flush_message(&mut client, &mut peer)?,
            ServerMessage::CopiedText("alpha 界".into())
        );

        apply_pty_output(&mut terminal, &mut selection, b"later")?;
        assert!(
            !Extractor::new()?.frame(revision + 1, &terminal)?.rows[0]
                .cells
                .iter()
                .any(|cell| cell.style.selected)
        );
        assert_eq!(selection.copied.as_deref(), Some("alpha 界"));

        let (mut blocked, _) = attached_client()?;
        fill_output(&mut blocked.output)?;
        let stable_revision = revision;
        assert!(!handle_client_message(
            &mut blocked,
            ClientMessage::Selection(session::SelectionAction::Begin {
                frame_revision: revision,
                cell: session::ViewportCell { x: 1, y: 0 },
            }),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert_eq!(revision, stable_revision);
        assert_eq!(selection.copied.as_deref(), Some("alpha 界"));

        apply_pty_output(&mut terminal, &mut selection, b"\x1b[?1049h")?;
        assert_eq!(terminal.active_screen()?, Screen::Alternate);
        assert_eq!(selection.copied.as_deref(), Some("alpha 界"));
        assert!(selection.anchor.is_none());
        assert!(!selection.visible);
        Ok(())
    }

    #[test]
    fn authoritative_viewport_routes_wheel_and_key_from_terminal_state() -> Result {
        let (mut client, mut peer) = attached_client()?;
        let pty = Pty::spawn(&["/bin/sh".into()], INITIAL_SIZE)?;
        let mut terminal = terminal_with_scrollback(100)?;
        for line in 0..32 {
            terminal.vt_write(format!("history-{line:02}\r\n").as_bytes());
        }
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut extractor = Extractor::new()?;
        let mut revision = 0;
        let mut selection = SelectionState::default();
        macro_rules! accept {
            ($message:expr, $frame:expr) => {{
                let previous_revision = revision;
                assert!(handle_client_message(
                    &mut client,
                    $message,
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut extractor,
                    &mut revision,
                    &mut selection,
                )?);
                assert_eq!(revision, previous_revision + u64::from($frame));
                assert_eq!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::Accepted
                );
                if $frame {
                    assert!(matches!(
                        flush_message(&mut client, &mut peer)?,
                        ServerMessage::Frame(frame) if frame.revision == revision
                    ));
                }
            }};
        }

        let live = terminal.scrollbar()?;
        assert_eq!(live.offset + live.len, live.total);
        accept!(wheel(MouseButton::Four), true);
        let scrolled = terminal.scrollbar()?;
        assert_eq!(scrolled.offset + 1, live.offset);

        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Top);
        accept!(wheel(MouseButton::Four), true);
        assert_eq!(terminal.scrollbar()?.offset, 0);

        accept!(wheel(MouseButton::Five), true);
        assert_eq!(terminal.scrollbar()?.offset, 1);

        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Bottom);
        accept!(wheel(MouseButton::Five), true);
        let live = terminal.scrollbar()?;
        assert_eq!(live.offset + live.len, live.total);

        terminal.vt_write(b"\x1b[?1000h\x1b[?1006h");
        for (button, expected) in [
            (MouseButton::Four, b"\x1b[<64;1;1M".as_slice()),
            (MouseButton::Six, b"\x1b[<66;1;1M".as_slice()),
        ] {
            accept!(wheel(button), false);
            assert_eq!(writes.take().into_iter().collect::<Vec<_>>(), expected);
        }

        terminal.vt_write(b"\x1b[?1000l\x1b[?1006l\x1b[?1049h\x1b[?1007h");
        for (modes, button, expected) in [
            (
                b"\x1b[?1l".as_slice(),
                MouseButton::Four,
                b"\x1b[A".as_slice(),
            ),
            (
                b"\x1b[?1h".as_slice(),
                MouseButton::Five,
                b"\x1bOB".as_slice(),
            ),
        ] {
            terminal.vt_write(modes);
            accept!(wheel(button), false);
            assert_eq!(writes.take().into_iter().collect::<Vec<_>>(), expected);
        }

        terminal.vt_write(b"\x1b[?1007l");
        accept!(wheel(MouseButton::Four), true);
        assert!(writes.borrow().is_empty());

        terminal.vt_write(b"\x1b[?1049l");
        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(-1));
        let key = ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::A,
            modifiers: Modifiers::empty(),
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: Some("x".into()),
            unshifted_codepoint: Some('x'),
        });
        accept!(key, true);
        assert_eq!(writes.take().into_iter().collect::<Vec<_>>(), b"x");
        let live = terminal.scrollbar()?;
        assert_eq!(live.offset + live.len, live.total);

        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(-1));
        let before_empty = terminal.scrollbar()?;
        accept!(
            ClientMessage::Key(KeyEvent {
                action: KeyAction::Release,
                key: PhysicalKey::ENTER,
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: None,
                unshifted_codepoint: None,
            }),
            false
        );
        assert!(writes.borrow().is_empty());
        assert_eq!(terminal.scrollbar()?.offset, before_empty.offset);

        assert!(queue_pty_write(
            &mut writes.borrow_mut(),
            &vec![b'q'; MAX_PTY_WRITE_BYTES],
        ));
        assert!(handle_client_message(
            &mut client,
            ClientMessage::Key(KeyEvent {
                action: KeyAction::Press,
                key: PhysicalKey::A,
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: Some("rejected".into()),
                unshifted_codepoint: Some('r'),
            }),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert_eq!(terminal.scrollbar()?.offset, before_empty.offset);
        assert_eq!(revision, 6);
        assert!(client.close_after_flush);
        assert!(matches!(
            flush_message(&mut client, &mut peer)?,
            ServerMessage::Failure(Failure {
                code: FailureCode::Terminal,
                ..
            })
        ));

        let (mut blocked_client, _) = attached_client()?;
        fill_output(&mut blocked_client.output)?;
        let resized = SurfaceSize {
            rows: 25,
            screen_height: 400,
            ..INITIAL_SIZE
        };
        assert!(!handle_client_message(
            &mut blocked_client,
            ClientMessage::Resize(resized),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert_eq!(size, resized);
        assert_eq!(revision, 7);
        Ok(())
    }

    #[test]
    fn outside_mouse_reporting_uses_held_button_semantics() -> Result {
        let mut terminal = terminal()?;
        terminal.vt_write(b"\x1b[?1003h\x1b[?1016h");
        let wheel = ClientMessage::Mouse(session::MouseEvent {
            action: MouseAction::Press,
            button: Some(MouseButton::Four),
            modifiers: Modifiers::empty(),
            x: 641.0,
            y: 1.0,
        });
        assert!(encode_input(&terminal, INITIAL_SIZE, wheel)?.is_empty());

        let motion = ClientMessage::Mouse(session::MouseEvent {
            action: MouseAction::Motion,
            button: Some(MouseButton::Left),
            modifiers: Modifiers::empty(),
            x: 641.0,
            y: 1.0,
        });

        assert_eq!(
            encode_input(&terminal, INITIAL_SIZE, motion)?,
            b"\x1b[<32;641;1M"
        );
        Ok(())
    }

    #[test]
    fn every_protocol_physical_key_reaches_the_terminal_mapper() {
        for raw in 0..=PhysicalKey::MAX_RAW {
            assert!(
                Key::try_from(u32::from(raw)).is_ok(),
                "protocol physical key {raw} has no terminal mapping"
            );
        }
    }

    #[test]
    fn terminal_closing_client_cannot_apply_later_input() -> Result {
        let (mut client, mut peer) = attached_client()?;
        write_client_message(&mut peer, &ClientMessage::Paste(b"ignored".to_vec()))?;
        client.close_when_flushed();
        let pty = Pty::spawn(&["/bin/sh".into()], INITIAL_SIZE)?;
        let mut terminal = terminal()?;
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut extractor = Extractor::new()?;
        let mut revision = 0;
        let mut selection = SelectionState::default();

        assert!(read_client(
            &mut client,
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert!(writes.borrow().is_empty());
        Ok(())
    }

    #[test]
    fn fatal_decode_releases_client_input_storage() -> Result {
        let (mut client, mut peer) = attached_client()?;
        let mut message =
            session::encode_client_message(&ClientMessage::Mouse(session::MouseEvent {
                action: MouseAction::Press,
                button: Some(MouseButton::Left),
                modifiers: Modifiers::empty(),
                x: 1.0,
                y: 1.0,
            }))?;
        message[session::HEADER_BYTES + 4..session::HEADER_BYTES + 8]
            .copy_from_slice(&f32::MAX.to_bits().to_le_bytes());
        peer.write_all(&message)?;
        let mut terminal = terminal()?;
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut extractor = Extractor::new()?;
        let mut revision = 0;
        let mut selection = SelectionState::default();

        assert!(read_client(
            &mut client,
            &mut terminal,
            None,
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert!(client.close_after_flush);
        assert!(client.input.is_empty());
        assert_eq!(client.input.capacity(), 0);
        assert!(client.output.flush(&mut client.stream)?);
        assert_eq!(
            read_server_message(&mut peer)?,
            Some(ServerMessage::Failure(Failure {
                code: FailureCode::Protocol,
                detail: "invalid mouse coordinates".into(),
            }))
        );
        Ok(())
    }

    #[test]
    fn pty_write_pressure_rejects_whole_input_and_closes_client() -> Result {
        let (mut client, mut peer) = attached_client()?;
        let pty = Pty::spawn(&["/bin/sh".into()], INITIAL_SIZE)?;
        let mut terminal = terminal()?;
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        assert!(queue_pty_write(
            &mut writes.borrow_mut(),
            &vec![b'x'; MAX_PTY_WRITE_BYTES],
        ));
        let mut extractor = Extractor::new()?;
        let mut revision = 0;
        let mut selection = SelectionState::default();

        assert!(handle_client_message(
            &mut client,
            ClientMessage::Paste(b"rejected".to_vec()),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert_eq!(writes.borrow().len(), MAX_PTY_WRITE_BYTES);
        assert!(client.close_after_flush);
        assert!(client.input.is_empty());
        assert_eq!(client.input.capacity(), 0);
        assert!(client.output.flush(&mut client.stream)?);
        assert_eq!(
            read_server_message(&mut peer)?,
            Some(ServerMessage::Failure(Failure {
                code: FailureCode::Terminal,
                detail: "PTY input queue is full".into(),
            }))
        );

        discard_pty_writes(&writes);
        assert!(writes.borrow().is_empty());
        assert_eq!(writes.borrow().capacity(), 0);
        Ok(())
    }

    #[test]
    fn closed_pty_discards_queued_storage_and_rejects_input() -> Result {
        let (mut client, mut peer) = attached_client()?;
        let mut terminal = terminal()?;
        let mut size = INITIAL_SIZE;
        let mut queued = VecDeque::with_capacity(1_024);
        queued.extend(b"queued before closure");
        let writes = RefCell::new(queued);
        let mut extractor = Extractor::new()?;
        let mut revision = 0;
        let mut selection = SelectionState::default();

        discard_pty_writes(&writes);
        assert!(writes.borrow().is_empty());
        assert_eq!(writes.borrow().capacity(), 0);
        assert!(handle_client_message(
            &mut client,
            ClientMessage::Paste(b"undeliverable".to_vec()),
            &mut terminal,
            None,
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
            &mut selection,
        )?);
        assert!(writes.borrow().is_empty());
        assert!(client.output.flush(&mut client.stream)?);
        assert_eq!(
            read_server_message(&mut peer)?,
            Some(ServerMessage::Failure(Failure {
                code: FailureCode::Terminal,
                detail: "PTY is closed".into(),
            }))
        );
        Ok(())
    }

    #[test]
    fn terminal_closing_client_cannot_receive_later_frames() -> Result {
        let (client, mut peer, expected) = closing_client()?;
        let mut active = Some(client);
        let terminal = terminal()?;
        let mut extractor = Extractor::new()?;

        publish_frame(&mut active, &mut extractor, 1, &terminal)?;
        let client = active
            .as_mut()
            .expect("closing client remains while draining");
        assert!(client.output.flush(&mut client.stream)?);
        assert!(client.output.is_empty());
        drop(active);

        let mut actual = Vec::new();
        peer.read_to_end(&mut actual)?;
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn terminal_closing_client_finishes_with_queued_failure_only() -> Result {
        let (mut client, mut peer, expected) = closing_client()?;

        client.finish_session(17)?;
        assert!(client.output.is_empty());
        drop(client);

        let mut actual = Vec::new();
        peer.read_to_end(&mut actual)?;
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn exited_child_output_is_drained_into_authoritative_state() -> Result {
        let size = INITIAL_SIZE;
        let command = [
            "/bin/sh".into(),
            "-c".into(),
            "printf '%5000s' x; printf '\\033]2;final-title\\033\\\\'".into(),
        ];
        let mut pty = Pty::spawn(&command, size)?;
        let mut terminal = terminal()?;
        let mut selection = SelectionState::default();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);

        let status = loop {
            if let Some(status) = pty.try_wait()? {
                pty.stop_and_reap();
                drain_exited_pty(&mut pty, &mut terminal, &mut selection)?;
                break status;
            }
            if Instant::now() >= deadline {
                return Err("PTY child did not exit within two seconds".into());
            }
            thread::yield_now();
        };

        assert!(status.success());
        assert_eq!(terminal.title()?, "final-title");
        Ok(())
    }
}
