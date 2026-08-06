#![cfg(target_os = "linux")]

mod platform;
mod presentation;

use libghostty_vt::{
    Terminal, TerminalOptions, focus,
    key::{
        Action as GhosttyKeyAction, Encoder as KeyEncoder, Event as GhosttyKeyEvent, Key,
        Mods as GhosttyModifiers,
    },
    mouse::{
        Action as GhosttyMouseAction, Button as GhosttyMouseButton, Encoder as MouseEncoder,
        EncoderSize as MouseEncoderSize, Event as GhosttyMouseEvent, Position as MousePosition,
    },
    paste,
    terminal::Mode,
};
use orbit_protocol::session::{
    self, ClientMessage, Failure, FailureCode, FocusEvent, KeyAction, KeyEvent, Modifiers,
    MouseAction, MouseButton, PhysicalKey, ServerMessage, SurfaceSize,
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

    let mut extractor = Extractor::new()?;
    let mut revision = 0;
    let mut client: Option<Client> = None;
    let mut pty_open = true;
    loop {
        if platform::termination_requested() {
            return Ok(0);
        }
        if let Some(status) = pty.try_wait()? {
            pty.stop_and_reap();
            let changed = drain_exited_pty(&mut pty, &mut terminal)?;
            fail_on_pty_write_overflow(&response_overflow)?;
            if changed {
                revision = next_revision(revision)?;
                publish_frame(&mut client, &mut extractor, revision, &terminal)?;
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
            client = None;
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
            let (open, changed) = read_pty_turn(&mut pty, &mut terminal)?;
            fail_on_pty_write_overflow(&response_overflow)?;
            pty_open = open;
            if changed {
                revision = next_revision(revision)?;
                publish_frame(&mut client, &mut extractor, revision, &terminal)?;
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
            )?
        {
            client = None;
        }
        if let Some(active) = &mut client {
            match active.output.flush(&mut active.stream) {
                Ok(true) if active.close_after_flush && active.output.is_empty() => client = None,
                Ok(true) => {}
                Ok(false) => client = None,
                Err(error) => return Err(error.into()),
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

fn read_client(
    client: &mut Client,
    terminal: &mut Terminal<'static, '_>,
    pty: Option<&Pty>,
    size: &mut SurfaceSize,
    writes: &RefCell<VecDeque<u8>>,
    extractor: &mut Extractor,
    revision: &mut u64,
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
            client, message, terminal, pty, size, writes, extractor, revision,
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
    match message {
        ClientMessage::Resize(surface) => {
            pty.resize(surface)?;
            terminal.resize(
                surface.cols,
                surface.rows,
                surface.cell_width,
                surface.cell_height,
            )?;
            *size = surface;
            if !client.output.push_message(&ServerMessage::Accepted)? {
                return Ok(false);
            }
            *revision = next_revision(*revision)?;
            queue_frame(client, extractor, *revision, terminal, false)
        }
        message => match encode_input(terminal, *size, message) {
            Ok(encoded) => {
                if queue_pty_write(&mut writes.borrow_mut(), &encoded) {
                    client.output.push_message(&ServerMessage::Accepted)
                } else {
                    let queued = queue_failure(
                        client,
                        FailureCode::Terminal,
                        "PTY input queue is full".into(),
                    )?;
                    client.close_when_flushed();
                    Ok(queued)
                }
            }
            Err(error) => queue_failure(client, FailureCode::Terminal, error.to_string()),
        },
    }
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
        ClientMessage::Hello { .. } | ClientMessage::Resize(_) => {
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

fn drain_exited_pty(pty: &mut Pty, terminal: &mut Terminal<'_, '_>) -> Result<bool> {
    let mut changed = false;
    for _ in 0..MAX_EXIT_PTY_READS {
        let (open, read) = read_pty_turn(pty, terminal)?;
        changed |= read;
        if !open || !read {
            break;
        }
    }
    Ok(changed)
}

fn read_pty_turn(pty: &mut Pty, terminal: &mut Terminal<'_, '_>) -> Result<(bool, bool)> {
    let mut bytes = [0; 8192];
    match pty.read(&mut bytes)? {
        PtyIo::Ready(0) | PtyIo::Closed => Ok((false, false)),
        PtyIo::Ready(read) => {
            terminal.vt_write(&bytes[..read]);
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

fn publish_frame(
    active: &mut Option<Client>,
    extractor: &mut Extractor,
    revision: u64,
    terminal: &Terminal<'static, '_>,
) -> Result {
    if let Some(client) = active
        .as_mut()
        .filter(|client| client.negotiation_deadline.is_none() && !client.close_after_flush)
        && !queue_frame(client, extractor, revision, terminal, false)?
    {
        *active = None;
    }
    Ok(())
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
        let size = INITIAL_SIZE;
        let mut terminal = Terminal::new(TerminalOptions {
            cols: size.cols,
            rows: size.rows,
            max_scrollback: 0,
        })?;
        terminal.resize(size.cols, size.rows, size.cell_width, size.cell_height)?;
        Ok(terminal)
    }

    fn key(key: PhysicalKey) -> ClientMessage {
        ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key,
            modifiers: Modifiers::empty(),
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: None,
            unshifted_codepoint: None,
        })
    }

    fn mouse() -> ClientMessage {
        ClientMessage::Mouse(session::MouseEvent {
            action: MouseAction::Press,
            button: Some(MouseButton::Left),
            modifiers: Modifiers::empty(),
            x: 1.0,
            y: 1.0,
        })
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

    #[test]
    fn authoritative_state_controls_all_semantic_input_encoding() -> Result {
        let size = INITIAL_SIZE;
        let normal = terminal()?;
        assert_eq!(
            encode_input(&normal, size, key(PhysicalKey::ARROW_UP))?,
            b"\x1b[A"
        );
        assert!(encode_input(&normal, size, mouse())?.is_empty());
        assert!(encode_input(&normal, size, ClientMessage::Focus(FocusEvent::Gained))?.is_empty());
        assert_eq!(
            encode_input(&normal, size, ClientMessage::Paste(b"a\nb".to_vec()))?,
            b"a\rb"
        );

        let mut modes = terminal()?;
        modes.vt_write(b"\x1b[?1h\x1b[?1000h\x1b[?1006h\x1b[?1004h\x1b[?2004h");
        assert_eq!(
            encode_input(&modes, size, key(PhysicalKey::ARROW_UP))?,
            b"\x1bOA"
        );
        assert_eq!(encode_input(&modes, size, mouse())?, b"\x1b[<0;1;1M");
        assert_eq!(
            encode_input(&modes, size, ClientMessage::Focus(FocusEvent::Gained))?,
            b"\x1b[I"
        );
        assert_eq!(
            encode_input(&modes, size, ClientMessage::Paste(b"a\nb".to_vec()))?,
            b"\x1b[200~a\nb\x1b[201~"
        );
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

        assert!(read_client(
            &mut client,
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
        )?);
        assert!(writes.borrow().is_empty());
        Ok(())
    }

    #[test]
    fn fatal_decode_releases_client_input_storage() -> Result {
        let (mut client, mut peer) = attached_client()?;
        let mut message = session::encode_client_message(&mouse())?;
        message[session::HEADER_BYTES + 4..session::HEADER_BYTES + 8]
            .copy_from_slice(&f32::MAX.to_bits().to_le_bytes());
        peer.write_all(&message)?;
        let mut terminal = terminal()?;
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut extractor = Extractor::new()?;
        let mut revision = 0;

        assert!(read_client(
            &mut client,
            &mut terminal,
            None,
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
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

        assert!(handle_client_message(
            &mut client,
            ClientMessage::Paste(b"rejected".to_vec()),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut extractor,
            &mut revision,
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
        let deadline = Instant::now() + std::time::Duration::from_secs(2);

        let status = loop {
            if let Some(status) = pty.try_wait()? {
                pty.stop_and_reap();
                drain_exited_pty(&mut pty, &mut terminal)?;
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
