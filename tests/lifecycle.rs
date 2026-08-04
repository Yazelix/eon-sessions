#![cfg(target_os = "linux")]

use std::{
    fs::{self, OpenOptions},
    io::{BufReader, Read, Write},
    net::Shutdown,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use orbit_protocol::{
    Frame,
    session::{
        self, ClientMessage, FailureCode, FocusEvent, KeyAction, KeyEvent, Modifiers, PhysicalKey,
        ServerMessage, SurfaceSize, decode_server_message, encode_client_message,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> TestResult<Self> {
        let path = std::env::temp_dir().join(format!(
            "orbit-{name}-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        Ok(Self(path))
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Server(Child);

impl Server {
    fn shutdown(mut self) -> TestResult<ExitStatus> {
        terminate(&self)?;
        wait_bounded(&mut self)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = terminate(self);
            let _ = wait_bounded(self);
        }
    }
}

struct Client {
    reader: BufReader<UnixStream>,
    frame: Frame,
}

impl Client {
    fn attach(socket: &Path) -> TestResult<Self> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let stream = connect_bounded(socket, deadline)?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.set_write_timeout(Some(Duration::from_secs(2)))?;
            let mut reader = BufReader::new(stream);
            write_message(
                reader.get_mut(),
                &ClientMessage::Hello {
                    minimum_version: session::VERSION,
                    maximum_version: session::VERSION,
                },
            )?;
            let mut attached = false;
            loop {
                match read_message(&mut reader) {
                    Ok(ServerMessage::Attached { version }) => {
                        assert_eq!(version, session::VERSION);
                        attached = true;
                    }
                    Ok(ServerMessage::Frame(frame)) if attached => {
                        return Ok(Self {
                            reader,
                            frame: *frame,
                        });
                    }
                    Ok(ServerMessage::Busy) if Instant::now() < deadline => break,
                    Ok(message) => {
                        return Err(format!("unexpected attach response: {message:?}").into());
                    }
                    Err(error) if Instant::now() < deadline => {
                        let _ = error;
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
            thread::yield_now();
        }
    }

    fn request(&mut self, request: &ClientMessage) -> TestResult {
        write_message(self.reader.get_mut(), request)?;
        loop {
            match read_message(&mut self.reader)? {
                ServerMessage::Accepted => return Ok(()),
                ServerMessage::Frame(frame) => self.frame = *frame,
                ServerMessage::Failure(failure) => {
                    return Err(format!("request failed: {failure:?}").into());
                }
                message => return Err(format!("unexpected response: {message:?}").into()),
            }
        }
    }

    fn paste(&mut self, bytes: impl Into<Vec<u8>>) -> TestResult {
        self.request(&ClientMessage::Paste(bytes.into()))
    }

    fn enter(&mut self) -> TestResult {
        self.request(&ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::ENTER,
            modifiers: Modifiers::empty(),
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: None,
            unshifted_codepoint: None,
        }))
    }

    fn text(&mut self, text: impl Into<String>) -> TestResult {
        self.request(&ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::A,
            modifiers: Modifiers::empty(),
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: Some(text.into()),
            unshifted_codepoint: Some('a'),
        }))
    }

    fn wait_title(&mut self, expected: &str) -> TestResult {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.frame.title == expected {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "title never became {expected:?}; last title: {:?}",
                    self.frame.title
                )
                .into());
            }
            match read_message(&mut self.reader)? {
                ServerMessage::Frame(frame) => self.frame = *frame,
                ServerMessage::Exited { code } => {
                    return Err(format!("shell exited with {code}").into());
                }
                _ => {}
            }
        }
    }
}

fn write_message(writer: &mut impl Write, message: &ClientMessage) -> TestResult {
    writer.write_all(&encode_client_message(message)?)?;
    Ok(())
}

fn read_message(reader: &mut impl Read) -> TestResult<ServerMessage> {
    let mut header = [0; session::HEADER_BYTES];
    reader.read_exact(&mut header)?;
    let length = session::server_message_len(&header)?
        .expect("complete header declares its server-message length");
    let mut framed = Vec::with_capacity(length);
    framed.extend_from_slice(&header);
    framed.resize(length, 0);
    reader.read_exact(&mut framed[session::HEADER_BYTES..])?;
    Ok(decode_server_message(&framed)?)
}

fn connect_bounded(socket: &Path, deadline: Instant) -> TestResult<UnixStream> {
    loop {
        match UnixStream::connect(socket) {
            Ok(stream) => return Ok(stream),
            Err(error)
                if Instant::now() < deadline
                    && matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) =>
            {
                thread::yield_now();
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn wait_file_text(path: &Path) -> TestResult<String> {
    wait_file_text_matching(path, |text| !text.trim().is_empty())
}

fn wait_file_text_matching(path: &Path, ready: impl Fn(&str) -> bool) -> TestResult<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = fs::read_to_string(path)
            && ready(&text)
        {
            return Ok(text);
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for content in {}", path.display()).into());
        }
        thread::yield_now();
    }
}

fn server_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yazelix-orbit"));
    command
        .arg("serve")
        .env("PS1", "")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn spawn_server(socket: &Path) -> TestResult<Server> {
    Ok(Server(
        server_command()
            .arg(socket)
            .arg("--")
            .arg("/bin/sh")
            .spawn()?,
    ))
}

fn spawn_default_server(runtime: &Path) -> TestResult<Server> {
    Ok(Server(
        server_command()
            .arg("--")
            .arg("/bin/sh")
            .env("XDG_RUNTIME_DIR", runtime)
            .spawn()?,
    ))
}

fn wait_bounded(server: &mut Server) -> TestResult<ExitStatus> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = server.0.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            server.0.kill()?;
            let _ = server.0.wait();
            return Err("server did not exit within five seconds".into());
        }
        thread::yield_now();
    }
}

fn wait_process_gone(pid: u32) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
        if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
            return Err(format!("PTY child {pid} remained alive").into());
        }
        thread::yield_now();
    }
}

fn terminate(server: &Server) -> std::io::Result<()> {
    match unsafe { libc::kill(server.0.id() as libc::pid_t, libc::SIGTERM) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

#[test]
fn attachment_negotiation_rejects_incompatible_and_unordered_clients() -> TestResult {
    let dir = TestDir::new("negotiation")?;
    let socket = dir.0.join("orbit.sock");
    let server = spawn_server(&socket)?;

    let mut incompatible = connect_bounded(&socket, Instant::now() + Duration::from_secs(5))?;
    incompatible.set_read_timeout(Some(Duration::from_secs(2)))?;
    write_message(
        &mut incompatible,
        &ClientMessage::Hello {
            minimum_version: session::VERSION + 1,
            maximum_version: session::VERSION + 1,
        },
    )?;
    assert_eq!(
        read_message(&mut incompatible)?,
        ServerMessage::Incompatible {
            minimum_version: session::VERSION,
            maximum_version: session::VERSION,
        }
    );
    drop(incompatible);

    let mut unordered = connect_bounded(&socket, Instant::now() + Duration::from_secs(5))?;
    unordered.set_read_timeout(Some(Duration::from_secs(2)))?;
    write_message(&mut unordered, &ClientMessage::Focus(FocusEvent::Gained))?;
    match read_message(&mut unordered)? {
        ServerMessage::Failure(failure) => assert_eq!(failure.code, FailureCode::Protocol),
        message => return Err(format!("unexpected unordered response: {message:?}").into()),
    }
    drop(unordered);

    drop(Client::attach(&socket)?);
    assert!(server.shutdown()?.success());
    Ok(())
}

#[test]
fn closed_pty_rejects_input_while_child_remains_alive() -> TestResult {
    let dir = TestDir::new("closed-pty")?;
    let socket = dir.0.join("orbit.sock");
    let marker = dir.0.join("closed");
    let mut server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg("exec 3>\"$ORBIT_MARKER\"; exec 0<&- 1>&- 2>&-; printf ready >&3; sleep 60")
            .env("ORBIT_MARKER", &marker)
            .spawn()?,
    );

    assert_eq!(wait_file_text(&marker)?, "ready");
    let mut client = Client::attach(&socket)?;
    assert!(server.0.try_wait()?.is_none());

    write_message(
        client.reader.get_mut(),
        &ClientMessage::Paste(b"undeliverable".to_vec()),
    )?;
    match read_message(&mut client.reader)? {
        ServerMessage::Failure(failure) => {
            assert_eq!(failure.code, FailureCode::Terminal);
            assert_eq!(failure.detail, "PTY is closed");
        }
        message => return Err(format!("unexpected closed-PTY response: {message:?}").into()),
    }
    assert!(server.0.try_wait()?.is_none());
    assert!(server.shutdown()?.success());
    Ok(())
}

#[test]
fn shell_survives_detach_and_one_client_reattaches() -> TestResult {
    let dir = TestDir::new("lifecycle")?;
    let socket = dir.0.join("orbit.sock");
    let fifo = dir.0.join("release.fifo");
    let first_pid_file = dir.0.join("first-shell.pid");
    let second_pid_file = dir.0.join("second-shell.pid");
    let key_file = dir.0.join("key-input");
    let multiline_file = dir.0.join("multiline-paste");
    let status = Command::new("mkfifo").arg(&fifo).status()?;
    assert!(status.success());

    let mut server = spawn_server(&socket)?;
    let mut first = Client::attach(&socket)?;
    first.paste(format!(
        "printf '%s\\n' \"$$\" > {}",
        first_pid_file.display()
    ))?;
    first.enter()?;
    let shell_pid = wait_file_text(&first_pid_file)?.trim().parse()?;

    first.text(format!("printf key > {}", key_file.display()))?;
    first.enter()?;
    wait_file_text_matching(&key_file, |text| text == "key")?;
    first.paste(format!(
        "printf first > {}\nprintf second >> {}",
        multiline_file.display(),
        multiline_file.display()
    ))?;
    first.enter()?;
    wait_file_text_matching(&multiline_file, |text| text == "firstsecond")?;
    first.request(&ClientMessage::Mouse(session::MouseEvent {
        action: session::MouseAction::Motion,
        button: Some(session::MouseButton::Left),
        modifiers: Modifiers::empty(),
        x: 1.0,
        y: 1.0,
    }))?;

    first.request(&ClientMessage::Resize(SurfaceSize {
        cols: 100,
        rows: 40,
        screen_width: 920,
        screen_height: 740,
        cell_width: 9,
        cell_height: 18,
        padding_top: 10,
        padding_bottom: 10,
        padding_left: 10,
        padding_right: 10,
    }))?;
    first.paste("printf '\\033]2;size-%s\\033\\\\' \"$(stty size)\"")?;
    first.enter()?;
    first.wait_title("size-40 100")?;
    assert_eq!(
        (first.frame.dimensions.cols, first.frame.dimensions.rows),
        (100, 40)
    );

    let mut rejected = UnixStream::connect(&socket)?;
    rejected.set_read_timeout(Some(Duration::from_secs(2)))?;
    assert_eq!(read_message(&mut rejected)?, ServerMessage::Busy);
    first.request(&ClientMessage::Focus(FocusEvent::Gained))?;

    first.paste(format!(
        "printf '\\033]2;ready\\033\\\\'; read orbit_release < {}; printf '\\033]2;detached\\033\\\\'",
        fifo.display()
    ))?;
    first.enter()?;
    first.wait_title("ready")?;
    drop(first);

    writeln!(OpenOptions::new().write(true).open(&fifo)?, "go")?;

    let mut second = Client::attach(&socket)?;
    second.paste(format!(
        "printf '%s\\n' \"$$\" > {}",
        second_pid_file.display()
    ))?;
    second.enter()?;
    assert_eq!(
        wait_file_text(&second_pid_file)?.trim().parse::<u32>()?,
        shell_pid
    );
    second.wait_title("detached")?;
    second.paste("exit")?;
    second.enter()?;
    drop(second);

    assert!(wait_bounded(&mut server)?.success());
    assert!(!socket.exists());
    wait_process_gone(shell_pid)?;
    Ok(())
}

#[test]
fn stale_socket_and_signal_shutdown_are_clean() -> TestResult {
    let dir = TestDir::new("cleanup")?;
    let runtime = dir.0.join("yazelix-orbit");
    fs::create_dir(&runtime)?;
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))?;
    let socket = runtime.join("orbit.sock");
    drop(UnixListener::bind(&socket)?);

    let server = spawn_default_server(&dir.0)?;
    let aborted = connect_bounded(&socket, Instant::now() + Duration::from_secs(5))?;
    aborted.shutdown(Shutdown::Read)?;
    drop(aborted);
    let mut client = Client::attach(&socket)?;
    let shell_pid_file = dir.0.join("shell.pid");
    client.paste(format!(
        "printf '%s\\n' \"$$\" > {}",
        shell_pid_file.display()
    ))?;
    client.enter()?;
    let shell_pid = wait_file_text(&shell_pid_file)?.trim().parse()?;
    let foreground_pid_file = dir.0.join("foreground.pid");
    client.paste(format!(
        "sh -c 'trap \"\" HUP; echo $$ > {}; printf \"\\033]2;foreground-ready\\033\\\\\"; exec sleep 60'",
        foreground_pid_file.display()
    ))?;
    client.enter()?;
    client.wait_title("foreground-ready")?;
    let foreground_pid = fs::read_to_string(&foreground_pid_file)?.trim().parse()?;
    let mode = fs::metadata(&socket)?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);

    assert!(server.shutdown()?.success());
    assert!(!socket.exists());
    wait_process_gone(shell_pid)?;
    wait_process_gone(foreground_pid)?;

    fs::write(&socket, b"do not remove")?;
    let mut refused = spawn_default_server(&dir.0)?;
    assert!(!wait_bounded(&mut refused)?.success());
    assert_eq!(fs::read(&socket)?, b"do not remove");

    fs::remove_file(&socket)?;
    let replaced = spawn_default_server(&dir.0)?;
    drop(Client::attach(&socket)?);
    fs::remove_file(&socket)?;
    let _replacement = UnixListener::bind(&socket)?;
    assert!(replaced.shutdown()?.success());
    assert!(socket.exists(), "server removed a replacement socket");
    Ok(())
}

#[test]
fn signal_during_startup_removes_socket() -> TestResult {
    let dir = TestDir::new("startup-signal")?;
    let socket = dir.0.join("orbit.sock");
    let server = spawn_server(&socket)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !socket.exists() {
        if Instant::now() >= deadline {
            return Err("server socket did not appear within five seconds".into());
        }
        thread::yield_now();
    }

    assert!(server.shutdown()?.success());
    assert!(!socket.exists());
    Ok(())
}
