#![cfg(target_os = "linux")]

mod platform;

use libghostty_vt::{
    Terminal, TerminalOptions, focus,
    key::{Action as KeyAction, Encoder as KeyEncoder, Event as KeyEvent, Key},
    mouse::{
        Action as MouseAction, Button as MouseButton, Encoder as MouseEncoder,
        EncoderSize as MouseEncoderSize, Event as MouseEvent, Position as MousePosition,
    },
    paste,
    terminal::Mode,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    env,
    error::Error,
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    rc::Rc,
};

use platform::{Pty, PtyIo};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const MAX_MESSAGE: usize = 4096;
const MAX_DIAGNOSTIC_TITLE: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Size {
    cols: u16,
    rows: u16,
    cell_width: u32,
    cell_height: u32,
}

impl Default for Size {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            cell_width: 8,
            cell_height: 16,
        }
    }
}

impl Size {
    fn mouse(self) -> MouseEncoderSize {
        MouseEncoderSize {
            screen_width: u32::from(self.cols) * self.cell_width,
            screen_height: u32::from(self.rows) * self.cell_height,
            cell_width: self.cell_width,
            cell_height: self.cell_height,
            padding_top: 0,
            padding_bottom: 0,
            padding_right: 0,
            padding_left: 0,
        }
    }
}

struct Client {
    stream: UnixStream,
    input: Vec<u8>,
}

enum SemanticInput {
    Key(Key),
    Mouse {
        action: MouseAction,
        button: MouseButton,
        x: f32,
        y: f32,
    },
    Focus(focus::Event),
    Paste(Vec<u8>),
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
    let mut size = Size::default();
    let mut pty = Pty::spawn(command, size)?;
    platform::ignore_broken_pipe()?;

    let writes = Rc::new(RefCell::new(VecDeque::<u8>::new()));
    let response_sink = Rc::clone(&writes);
    let mut terminal = Terminal::new(TerminalOptions {
        cols: size.cols,
        rows: size.rows,
        max_scrollback: 1_000,
    })?;
    terminal.resize(size.cols, size.rows, size.cell_width, size.cell_height)?;
    terminal.on_pty_write(move |_, bytes| {
        response_sink.borrow_mut().extend(bytes);
    })?;

    let mut client: Option<Client> = None;
    let mut pty_open = true;
    loop {
        if platform::termination_requested() {
            return Ok(0);
        }
        if let Some(status) = pty.try_wait()? {
            read_pty(&mut pty, &mut terminal)?;
            let code = status.code().unwrap_or(1);
            if let Some(client) = &mut client {
                let _ = send_line(&mut client.stream, &format!("EXIT {code}"));
            }
            return Ok(code);
        }

        let readiness = platform::poll(
            &listener,
            pty_open.then(|| (&pty, !writes.borrow().is_empty())),
            client.as_ref().map(|client| &client.stream),
        )?;

        if readiness.listener {
            accept_clients(&listener, &mut client)?;
        }
        if readiness.pty_read {
            pty_open = read_pty(&mut pty, &mut terminal)?;
        }
        if readiness.pty_write {
            pty_open &= flush_pty(&mut pty, &mut writes.borrow_mut())?;
        }
        if readiness.client
            && !read_client(
                client.as_mut().expect("client existed when poll began"),
                &mut terminal,
                &pty,
                &mut size,
                &writes,
            )?
        {
            client = None;
        }
    }
}

fn run_client(socket: &Path) -> Result<i32> {
    let stream = UnixStream::connect(socket)?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    print!("{response}");
    if response.trim() == "BUSY" {
        return Ok(2);
    }

    for line in io::stdin().lock().lines() {
        writeln!(writer, "{}", line?)?;
        response.clear();
        if reader.read_line(&mut response)? == 0 {
            break;
        }
        print!("{response}");
    }
    Ok(0)
}

fn read_client(
    client: &mut Client,
    terminal: &mut Terminal<'_, '_>,
    pty: &Pty,
    size: &mut Size,
    writes: &RefCell<VecDeque<u8>>,
) -> Result<bool> {
    let mut bytes = [0; 1024];
    let read = match client.stream.read(&mut bytes) {
        Ok(0) => return Ok(false),
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(true),
        Err(error) if is_disconnect(&error) => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    client.input.extend_from_slice(&bytes[..read]);
    if client.input.len() > MAX_MESSAGE {
        let _ = send_line(&mut client.stream, "ERROR message too large");
        return Ok(false);
    }

    while let Some(end) = client.input.iter().position(|byte| *byte == b'\n') {
        let mut message: Vec<u8> = client.input.drain(..=end).collect();
        message.pop();
        if message.last() == Some(&b'\r') {
            message.pop();
        }
        let response = match std::str::from_utf8(&message) {
            Ok(message) => handle_message(message, terminal, pty, size, writes)
                .unwrap_or_else(|error| format!("ERROR {error}")),
            Err(_) => "ERROR message is not UTF-8".into(),
        };
        if send_line(&mut client.stream, &response).is_err() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn handle_message(
    message: &str,
    terminal: &mut Terminal<'_, '_>,
    pty: &Pty,
    size: &mut Size,
    writes: &RefCell<VecDeque<u8>>,
) -> Result<String> {
    if message == "PING" {
        return Ok("PONG".into());
    }
    if message == "PID" {
        return Ok(format!("PID {}", pty.id()));
    }
    if message == "TITLE" {
        let title = terminal.title()?.as_bytes();
        if title.len() > MAX_DIAGNOSTIC_TITLE {
            return Err("title exceeds the diagnostic bound".into());
        }
        return Ok(format!("TITLE {}", hex(title)));
    }
    if message == "SIZE" {
        return Ok(format!(
            "SIZE {} {} {} {}",
            size.cols, size.rows, size.cell_width, size.cell_height
        ));
    }
    if let Some(arguments) = message.strip_prefix("RESIZE ") {
        let values: Vec<_> = arguments.split_whitespace().collect();
        if values.len() != 4 {
            return Err("RESIZE requires cols rows cell-width cell-height".into());
        }
        let next = Size {
            cols: values[0].parse()?,
            rows: values[1].parse()?,
            cell_width: values[2].parse()?,
            cell_height: values[3].parse()?,
        };
        if next.cols == 0
            || next.rows == 0
            || next.cell_width == 0
            || next.cell_height == 0
            || next.cols > 1_000
            || next.rows > 1_000
            || next.cell_width > 1_000
            || next.cell_height > 1_000
        {
            return Err("RESIZE values are outside the diagnostic bounds".into());
        }
        pty.resize(next)?;
        terminal.resize(next.cols, next.rows, next.cell_width, next.cell_height)?;
        *size = next;
        return Ok("OK".into());
    }

    let input = parse_input(message)?;
    let encoded = encode_input(terminal, *size, input)?;
    writes.borrow_mut().extend(encoded);
    Ok("OK".into())
}

fn parse_input(message: &str) -> Result<SemanticInput> {
    if let Some(text) = message.strip_prefix("PASTE ") {
        return Ok(SemanticInput::Paste(text.as_bytes().to_vec()));
    }
    match message {
        "PASTE" => Ok(SemanticInput::Paste(Vec::new())),
        "KEY UP" => Ok(SemanticInput::Key(Key::ArrowUp)),
        "KEY ENTER" => Ok(SemanticInput::Key(Key::Enter)),
        "FOCUS IN" => Ok(SemanticInput::Focus(focus::Event::Gained)),
        "FOCUS OUT" => Ok(SemanticInput::Focus(focus::Event::Lost)),
        _ if message.starts_with("MOUSE ") => parse_mouse(message),
        _ => Err("unknown diagnostic message".into()),
    }
}

fn parse_mouse(message: &str) -> Result<SemanticInput> {
    let values: Vec<_> = message.split_whitespace().collect();
    if values.len() != 5 {
        return Err("MOUSE requires action button x y".into());
    }
    let action = match values[1] {
        "PRESS" => MouseAction::Press,
        "RELEASE" => MouseAction::Release,
        "MOTION" => MouseAction::Motion,
        _ => return Err("unknown mouse action".into()),
    };
    let button = match values[2] {
        "LEFT" => MouseButton::Left,
        "RIGHT" => MouseButton::Right,
        "MIDDLE" => MouseButton::Middle,
        _ => return Err("unknown mouse button".into()),
    };
    let x = values[3].parse::<f32>()?;
    let y = values[4].parse::<f32>()?;
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
        return Err("mouse coordinates must be finite and non-negative".into());
    }
    Ok(SemanticInput::Mouse {
        action,
        button,
        x,
        y,
    })
}

fn encode_input(terminal: &Terminal<'_, '_>, size: Size, input: SemanticInput) -> Result<Vec<u8>> {
    match input {
        SemanticInput::Key(key) => {
            let mut event = KeyEvent::new()?;
            event
                .set_action(KeyAction::Press)
                .set_key(key)
                .set_utf8::<String>(None);
            let mut encoder = KeyEncoder::new()?;
            encoder.set_options_from_terminal(terminal);
            let mut output = Vec::new();
            encoder.encode_to_vec(&event, &mut output)?;
            Ok(output)
        }
        SemanticInput::Mouse {
            action,
            button,
            x,
            y,
        } => {
            let mut event = MouseEvent::new()?;
            event
                .set_action(action)
                .set_button(Some(button))
                .set_position(MousePosition { x, y });
            let mut encoder = MouseEncoder::new()?;
            encoder
                .set_options_from_terminal(terminal)
                .set_size(size.mouse());
            let mut output = Vec::new();
            encoder.encode_to_vec(&event, &mut output)?;
            Ok(output)
        }
        SemanticInput::Focus(event) => {
            if !terminal.mode(Mode::FOCUS_EVENT)? {
                return Ok(Vec::new());
            }
            let mut output = vec![0; 8];
            let written = event.encode(&mut output)?;
            output.truncate(written);
            Ok(output)
        }
        SemanticInput::Paste(mut data) => {
            let bracketed = terminal.mode(Mode::BRACKETED_PASTE)?;
            let mut output = vec![0; data.len() + 16];
            let written = paste::encode(&mut data, bracketed, &mut output)?;
            output.truncate(written);
            Ok(output)
        }
    }
}

fn read_pty(pty: &mut Pty, terminal: &mut Terminal<'_, '_>) -> Result<bool> {
    let mut bytes = [0; 8192];
    loop {
        match pty.read(&mut bytes)? {
            PtyIo::Ready(read) => terminal.vt_write(&bytes[..read]),
            PtyIo::Blocked => return Ok(true),
            PtyIo::Closed => return Ok(false),
        }
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
                let _ = send_line(&mut stream, "BUSY");
            }
            Ok((mut stream, _)) => {
                if send_line(&mut stream, "ATTACHED").is_ok() {
                    *active = Some(Client {
                        stream,
                        input: Vec::new(),
                    });
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

fn send_line(stream: &mut UnixStream, line: &str) -> io::Result<()> {
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")
}

fn is_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
    )
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Instant};

    fn terminal() -> Result<Terminal<'static, 'static>> {
        let size = Size::default();
        let mut terminal = Terminal::new(TerminalOptions {
            cols: size.cols,
            rows: size.rows,
            max_scrollback: 0,
        })?;
        terminal.resize(size.cols, size.rows, size.cell_width, size.cell_height)?;
        Ok(terminal)
    }

    #[test]
    fn authoritative_state_controls_all_semantic_input_encoding() -> Result {
        let size = Size::default();
        let normal = terminal()?;
        assert_eq!(
            encode_input(&normal, size, SemanticInput::Key(Key::ArrowUp))?,
            b"\x1b[A"
        );
        assert!(
            encode_input(
                &normal,
                size,
                SemanticInput::Mouse {
                    action: MouseAction::Press,
                    button: MouseButton::Left,
                    x: 1.0,
                    y: 1.0,
                }
            )?
            .is_empty()
        );
        assert!(
            encode_input(&normal, size, SemanticInput::Focus(focus::Event::Gained))?.is_empty()
        );
        assert_eq!(
            encode_input(&normal, size, SemanticInput::Paste(b"a\nb".to_vec()))?,
            b"a\rb"
        );

        let mut modes = terminal()?;
        modes.vt_write(b"\x1b[?1h\x1b[?1000h\x1b[?1006h\x1b[?1004h\x1b[?2004h");
        assert_eq!(
            encode_input(&modes, size, SemanticInput::Key(Key::ArrowUp))?,
            b"\x1bOA"
        );
        assert_eq!(
            encode_input(
                &modes,
                size,
                SemanticInput::Mouse {
                    action: MouseAction::Press,
                    button: MouseButton::Left,
                    x: 1.0,
                    y: 1.0,
                }
            )?,
            b"\x1b[<0;1;1M"
        );
        assert_eq!(
            encode_input(&modes, size, SemanticInput::Focus(focus::Event::Gained))?,
            b"\x1b[I"
        );
        assert_eq!(
            encode_input(&modes, size, SemanticInput::Paste(b"a\nb".to_vec()))?,
            b"\x1b[200~a\nb\x1b[201~"
        );
        Ok(())
    }

    #[test]
    fn exited_child_output_is_drained_into_authoritative_state() -> Result {
        let size = Size::default();
        let command = [
            "/bin/sh".into(),
            "-c".into(),
            "printf '\\033]2;final-title\\033\\\\'".into(),
        ];
        let mut pty = Pty::spawn(&command, size)?;
        let mut terminal = terminal()?;
        let deadline = Instant::now() + std::time::Duration::from_secs(2);

        let status = loop {
            if let Some(status) = pty.try_wait()? {
                read_pty(&mut pty, &mut terminal)?;
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
