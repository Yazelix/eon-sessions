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
    Frame, Rgb, Screen,
    session::{
        self, ClientMessage, FailureCode, FocusEvent, KeyAction, KeyEvent, Modifiers, PhysicalKey,
        SelectionAction, ServerMessage, SurfaceSize, ViewportCell, decode_server_message,
        encode_client_message,
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
        let hello = encode_client_message(&ClientMessage::Hello {
            minimum_version: session::VERSION,
            maximum_version: session::VERSION,
        })?;
        loop {
            let stream = connect_bounded(socket, deadline)?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.set_write_timeout(Some(Duration::from_secs(2)))?;
            let mut reader = BufReader::new(stream);
            if let Err(error) = reader.get_mut().write_all(&hello) {
                if Instant::now() >= deadline {
                    return Err(error.into());
                }
                thread::yield_now();
                continue;
            }
            let mut attached = false;
            loop {
                match read_message(&mut reader) {
                    Ok(ServerMessage::Attached { version }) if !attached => {
                        assert_eq!(version, session::VERSION);
                        attached = true;
                    }
                    Ok(ServerMessage::Frame(frame)) if attached => {
                        return Ok(Self {
                            reader,
                            frame: *frame,
                        });
                    }
                    Ok(ServerMessage::Busy) if !attached && Instant::now() < deadline => break,
                    Ok(message) => {
                        return Err(format!("unexpected attach response: {message:?}").into());
                    }
                    Err(error) if Instant::now() < deadline && error.is::<std::io::Error>() => {
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

    fn request_frame(&mut self, request: &ClientMessage) -> TestResult {
        let previous_revision = self.frame.revision;
        write_message(self.reader.get_mut(), request)?;
        let mut accepted = false;
        loop {
            match read_message(&mut self.reader)? {
                ServerMessage::Accepted => accepted = true,
                ServerMessage::Frame(frame) => {
                    assert!(frame.revision > self.frame.revision);
                    self.frame = *frame;
                    if accepted && self.frame.revision > previous_revision {
                        return Ok(());
                    }
                }
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

    fn key(&mut self, action: KeyAction, key: PhysicalKey, modifiers: Modifiers) -> TestResult {
        self.request(&ClientMessage::Key(KeyEvent {
            action,
            key,
            modifiers,
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: None,
            unshifted_codepoint: None,
        }))
    }

    fn enter(&mut self) -> TestResult {
        self.key(KeyAction::Press, PhysicalKey::ENTER, Modifiers::empty())
    }

    fn text(&mut self, text: impl Into<String>, consumed_modifiers: Modifiers) -> TestResult {
        self.request(&ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::A,
            modifiers: consumed_modifiers,
            consumed_modifiers,
            composing: false,
            text: Some(text.into()),
            unshifted_codepoint: Some('a'),
        }))
    }

    fn wheel(&mut self, button: session::MouseButton) -> TestResult {
        self.request_frame(&ClientMessage::Mouse(session::MouseEvent {
            action: session::MouseAction::Press,
            button: Some(button),
            modifiers: Modifiers::empty(),
            x: 1.0,
            y: 1.0,
        }))
    }

    fn select(&mut self, action: SelectionAction) -> TestResult {
        self.request_frame(&ClientMessage::Selection(action))
    }

    fn copy(&mut self) -> TestResult<Result<String, FailureCode>> {
        write_message(
            self.reader.get_mut(),
            &ClientMessage::Selection(SelectionAction::Copy),
        )?;
        loop {
            match read_message(&mut self.reader)? {
                ServerMessage::CopiedText(text) => return Ok(Ok(text)),
                ServerMessage::Frame(frame) => self.frame = *frame,
                ServerMessage::Failure(failure) => return Ok(Err(failure.code)),
                message => return Err(format!("unexpected copy response: {message:?}").into()),
            }
        }
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
    let binary = env!("CARGO_BIN_EXE_yazelix-orbit");
    let mut command = if let Some(valgrind) = std::env::var_os("ORBIT_MEMCHECK") {
        let mut command = Command::new(valgrind);
        command.args([
            "--tool=memcheck",
            "--leak-check=full",
            "--show-leak-kinds=all",
            "--errors-for-leak-kinds=definite,indirect",
            "--track-fds=yes",
            "--error-exitcode=97",
            "--log-file=target/orbit-memcheck.%p.log",
            binary,
        ]);
        command
    } else {
        Command::new(binary)
    };
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

fn process_cpu_ticks(pid: u32) -> TestResult<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let (_, fields) = stat
        .rsplit_once(") ")
        .ok_or("process stat is missing its command terminator")?;
    let mut fields = fields.split_whitespace();
    let user = fields.nth(11).ok_or("process stat is missing user CPU")?;
    let system = fields.next().ok_or("process stat is missing system CPU")?;
    Ok(user.parse::<u64>()? + system.parse::<u64>()?)
}

fn terminate(server: &Server) -> std::io::Result<()> {
    match unsafe { libc::kill(server.0.id() as libc::pid_t, libc::SIGTERM) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

fn frame_text(frame: &Frame) -> String {
    frame
        .rows
        .iter()
        .flat_map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.text.as_str())
                .chain(["\n"])
        })
        .collect()
}

#[test]
fn ansi_palette_launch_is_visible_and_survives_reattachment() -> TestResult {
    const PALETTE: &str = "000102,101112,202122,303132,404142,505152,606162,707172,808182,909192,a0a1a2,b0b1b2,c0c1c2,d0d1d2,e0e1e2,f0f1f2";
    const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb { r, g, b }
    }

    let dir = TestDir::new("ansi-palette")?;
    let socket = dir.0.join("orbit.sock");
    let server = Server(
        server_command()
            .arg(&socket)
            .arg("--ansi-palette-v1")
            .arg(PALETTE)
            .arg("--")
            .arg("/bin/sh")
            .spawn()?,
    );
    let first = Client::attach(&socket)?;
    assert_eq!(first.frame.colors.palette[0], rgb(0, 1, 2));
    assert_eq!(first.frame.colors.palette[15], rgb(0xf0, 0xf1, 0xf2));
    assert_eq!(first.frame.colors.palette[16], Rgb::BLACK);
    drop(first);

    let mut second = Client::attach(&socket)?;
    assert_eq!(second.frame.colors.palette[1], rgb(0x10, 0x11, 0x12));
    second.paste("printf '\\033]4;1;rgb:01/02/03\\033\\\\'; printf '\\033]2;override\\033\\\\'")?;
    second.enter()?;
    second.wait_title("override")?;
    assert_eq!(second.frame.colors.palette[1], rgb(1, 2, 3));
    second.paste("printf '\\033]104;1\\033\\\\'; printf '\\033]2;reset\\033\\\\'")?;
    second.enter()?;
    second.wait_title("reset")?;
    assert_eq!(second.frame.colors.palette[1], rgb(0x10, 0x11, 0x12));
    drop(second);
    assert!(server.shutdown()?.success());

    let invalid_socket = dir.0.join("invalid.sock");
    let child_marker = dir.0.join("child-started");
    let status = server_command()
        .arg(&invalid_socket)
        .arg("--ansi-palette-v1")
        .arg("incomplete")
        .arg("--")
        .arg("/bin/sh")
        .arg("-c")
        .arg(format!(": > {}", child_marker.display()))
        .status()?;
    assert!(!status.success());
    assert!(!invalid_socket.exists());
    assert!(!child_marker.exists());
    Ok(())
}

#[test]
fn attachment_negotiation_and_races_recover_for_canonical_client() -> TestResult {
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

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut first = connect_bounded(&socket, deadline)?;
    first.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut second = connect_bounded(&socket, deadline)?;
    second.set_read_timeout(Some(Duration::from_secs(2)))?;

    assert_eq!(read_message(&mut second)?, ServerMessage::Busy);
    drop(second);

    drop(Client::attach(&socket)?);
    assert_eq!(first.read(&mut [0])?, 0);
    assert!(server.shutdown()?.success());
    Ok(())
}

#[test]
fn pty_pressure_disconnect_ignores_stale_client_readiness() -> TestResult {
    let dir = TestDir::new("stale-client-readiness")?;
    let socket = dir.0.join("orbit.sock");
    let emitted = dir.0.join("emitted");

    // One more than the pending-effect bound disconnects during PTY processing.
    let effects = "\\033]52;c;eA==\\033\\\\".repeat(65);
    let server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg(
                "stty -echo; read orbit_trigger; printf \"$ORBIT_EFFECTS\"; \
                 printf emitted > \"$ORBIT_EMITTED\"; read orbit_stop",
            )
            .env("ORBIT_EFFECTS", effects)
            .env("ORBIT_EMITTED", &emitted)
            .spawn()?,
    );
    let mut first = Client::attach(&socket)?;

    let mut input = encode_client_message(&ClientMessage::Paste(b"trigger\n".to_vec()))?;
    // The complete first message releases the child; the incomplete second
    // message keeps client input ready without producing another reply.
    let incomplete =
        encode_client_message(&ClientMessage::Paste(vec![b'x'; session::MAX_PASTE_BYTES]))?;
    input.extend_from_slice(&incomplete[..incomplete.len() - 1]);
    let _ = first.reader.get_mut().write_all(&input);
    assert_eq!(wait_file_text(&emitted)?, "emitted");

    let _second = Client::attach(&socket)?;
    assert!(server.shutdown()?.success());
    Ok(())
}

#[test]
fn transient_pty_eio_recovers_when_the_live_child_reopens_the_terminal() -> TestResult {
    let dir = TestDir::new("transient-pty-eio")?;
    let socket = dir.0.join("orbit.sock");
    let marker = dir.0.join("lifecycle");
    let input = dir.0.join("input");
    let release = dir.0.join("release");
    let mut server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg(
                "exec 3>\"$ORBIT_MARKER\"; exec 0<&- 1>&- 2>&-; printf closed >&3; \
                 while [ ! -e \"$ORBIT_RELEASE\" ]; do sleep 0.01; done; sleep 1; \
                 exec 0<>/dev/tty 1>&0 2>&0; printf ',reopened' >&3; \
                 printf '\\033]2;reopened\\033\\\\'; IFS= read -r input; printf '%s' \"$input\" > \"$ORBIT_INPUT\"",
            )
            .env("ORBIT_MARKER", &marker)
            .env("ORBIT_INPUT", &input)
            .env("ORBIT_RELEASE", &release)
            .spawn()?,
    );

    assert_eq!(wait_file_text(&marker)?, "closed");
    let mut client = Client::attach(&socket)?;
    assert!(server.0.try_wait()?.is_none());
    let cpu_before = std::env::var_os("ORBIT_MEMCHECK")
        .is_none()
        .then(|| process_cpu_ticks(server.0.id()))
        .transpose()?;
    fs::write(&release, b"release")?;
    assert_eq!(
        wait_file_text_matching(&marker, |text| text == "closed,reopened")?,
        "closed,reopened"
    );
    if let Some(cpu_before) = cpu_before {
        let cpu_ticks = process_cpu_ticks(server.0.id())? - cpu_before;
        let clock_ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        assert!(clock_ticks > 0, "sysconf(_SC_CLK_TCK) failed");
        assert!(
            cpu_ticks.saturating_mul(5) < clock_ticks as u64,
            "Orbit consumed {cpu_ticks}/{clock_ticks} CPU ticks during a one-second PTY slave gap"
        );
    }
    client.wait_title("reopened")?;
    client.paste("after-reopen\n")?;
    assert_eq!(wait_file_text(&input)?, "after-reopen");
    assert!(wait_bounded(&mut server)?.success());
    Ok(())
}

#[test]
fn writing_descendant_cannot_hold_server_open_after_known_child_exit() -> TestResult {
    let dir = TestDir::new("writing-descendant")?;
    let socket = dir.0.join("orbit.sock");
    let descendant_pid_file = dir.0.join("descendant.pid");
    let mut server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg(
                "sh -c 'trap \"\" HUP; printf \"%s\\n\" \"$$\" > \"$ORBIT_DESCENDANT_PID\"; exec yes orbit' & while [ ! -s \"$ORBIT_DESCENDANT_PID\" ]; do :; done",
            )
            .env("ORBIT_DESCENDANT_PID", &descendant_pid_file)
            .spawn()?,
    );

    let descendant_pid = wait_file_text(&descendant_pid_file)?.trim().parse()?;
    let status = wait_bounded(&mut server);
    if status.is_err() {
        unsafe {
            libc::kill(descendant_pid as libc::pid_t, libc::SIGKILL);
        }
    }

    assert!(status?.success());
    assert!(!socket.exists());
    wait_process_gone(descendant_pid)?;
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

    first.text(
        format!("printf key > {}", key_file.display()),
        Modifiers::empty(),
    )?;
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
    assert_eq!(
        (second.frame.dimensions.cols, second.frame.dimensions.rows),
        (100, 40)
    );
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
fn selection_copy_is_authoritative_bounded_and_client_scoped() -> TestResult {
    let dir = TestDir::new("selection")?;
    let socket = dir.0.join("orbit.sock");
    let release = dir.0.join("release");
    let activity = dir.0.join("activity");
    let stop = dir.0.join("stop");
    let script = format!(
        "stty -echo; \
         while [ ! -e '{release}' ]; do sleep 0.01; done; \
         i=0; while [ \"$i\" -lt 6 ]; do printf 'history-%02d\\n' \"$i\"; i=$((i + 1)); done; \
         printf 'first-界-é-second'; printf '\\033]2;selection-ready\\033\\\\'; \
         while [ ! -e '{activity}' ]; do sleep 0.01; done; \
         printf '\\033[?1049hhidden\\033[?1049l\\033]2;selection-activity\\033\\\\'; \
         while [ ! -e '{stop}' ]; do sleep 0.01; done",
        release = release.display(),
        activity = activity.display(),
        stop = stop.display(),
    );
    let mut server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg(script)
            .spawn()?,
    );

    let mut first = Client::attach(&socket)?;
    let surface = SurfaceSize {
        cols: 20,
        rows: 4,
        cell_width: 8,
        cell_height: 16,
        screen_width: 160,
        screen_height: 64,
        padding_top: 0,
        padding_bottom: 0,
        padding_left: 0,
        padding_right: 0,
    };
    first.request_frame(&ClientMessage::Resize(surface))?;
    fs::write(&release, b"release")?;
    first.wait_title("selection-ready")?;
    let live = first.frame.clone();
    first.wheel(session::MouseButton::Four)?;
    assert_ne!(first.frame.rows, live.rows);
    first.wheel(session::MouseButton::Five)?;
    assert_eq!(first.frame.rows, live.rows);

    first.select(SelectionAction::Begin {
        frame_revision: first.frame.revision,
        cell: ViewportCell { x: 16, y: 3 },
    })?;
    first.select(SelectionAction::Update {
        cell: ViewportCell { x: 0, y: 3 },
    })?;
    first.select(SelectionAction::Finish {
        cell: ViewportCell { x: 0, y: 3 },
    })?;
    assert!(
        first
            .frame
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .any(|cell| cell.style.selected)
    );
    assert_eq!(first.copy()?, Ok("first-界-é-second".into()));

    let selected = first.frame.clone();
    drop(first);
    let mut second = Client::attach(&socket)?;
    assert!(second.frame.revision > selected.revision);
    assert_eq!(frame_text(&second.frame), frame_text(&selected));
    assert!(
        !second
            .frame
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .any(|cell| cell.style.selected)
    );
    assert_eq!(second.copy()?, Err(FailureCode::InvalidInput));

    second.select(SelectionAction::Begin {
        frame_revision: second.frame.revision,
        cell: ViewportCell { x: 16, y: 3 },
    })?;
    second.select(SelectionAction::Update {
        cell: ViewportCell { x: 0, y: 3 },
    })?;
    second.select(SelectionAction::Finish {
        cell: ViewportCell { x: 0, y: 3 },
    })?;
    fs::write(&activity, b"activity")?;
    second.wait_title("selection-activity")?;
    assert!(
        !second
            .frame
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .any(|cell| cell.style.selected)
    );
    assert_eq!(second.copy()?, Ok("first-界-é-second".into()));

    second.request_frame(&ClientMessage::Resize(SurfaceSize {
        cols: 21,
        screen_width: 168,
        ..surface
    }))?;
    assert_eq!(second.copy()?, Ok("first-界-é-second".into()));

    fs::write(&stop, b"stop")?;
    drop(second);
    assert!(wait_bounded(&mut server)?.success());
    assert!(!socket.exists());
    Ok(())
}

#[test]
fn authoritative_viewport_survives_detach_and_slow_reader_pressure() -> TestResult {
    let dir = TestDir::new("viewport")?;
    let socket = dir.0.join("orbit.sock");
    let release = dir.0.join("release");
    let clear = dir.0.join("clear");
    let pressure = dir.0.join("pressure");
    let stop = dir.0.join("stop");
    let script = format!(
        "stty -echo; \
         i=0; while [ \"$i\" -lt 48 ]; do printf 'history-%02d\\n' \"$i\"; i=$((i + 1)); done; \
         printf '\\033]2;history-ready\\033\\\\'; \
         while [ ! -e '{release}' ] && [ ! -e '{stop}' ]; do sleep 0.01; done; \
         [ -e '{stop}' ] && exit; \
         printf 'tail-after-pin\\n\\033]2;pinned-output\\033\\\\'; \
         while [ ! -e '{clear}' ] && [ ! -e '{stop}' ]; do sleep 0.01; done; \
         [ -e '{stop}' ] && exit; \
         printf '\\033[3J'; \
         i=0; while [ \"$i\" -lt 1100 ]; do printf 'prune-%04d-abcdefghijklmnopqrstuvwxyz-ABCDEFGHIJKLMNOPQRSTUVWXYZ-0123456789\\n' \"$i\"; i=$((i + 1)); done; \
         printf '\\033]2;pruned\\033\\\\'; \
         while [ ! -e '{pressure}' ] && [ ! -e '{stop}' ]; do sleep 0.01; done; \
         [ -e '{stop}' ] && exit; \
         printf '\\033]2;pressure-complete\\033\\\\'; \
         while [ ! -e '{stop}' ]; do sleep 0.01; done",
        release = release.display(),
        clear = clear.display(),
        pressure = pressure.display(),
        stop = stop.display(),
    );
    let mut server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg(script)
            .spawn()?,
    );

    let mut first = Client::attach(&socket)?;
    first.wait_title("history-ready")?;
    let live_text = frame_text(&first.frame);
    for _ in 0..6 {
        first.wheel(session::MouseButton::Four)?;
    }
    let pinned = first.frame.clone();
    assert_ne!(frame_text(&pinned), live_text);

    fs::write(&release, b"release")?;
    first.wait_title("pinned-output")?;
    assert_eq!(first.frame.rows, pinned.rows);
    assert!(!frame_text(&first.frame).contains("tail-after-pin"));
    let detached = first.frame.clone();
    drop(first);

    let mut second = Client::attach(&socket)?;
    assert_eq!(second.frame, detached);
    second.request_frame(&ClientMessage::Key(KeyEvent {
        action: KeyAction::Press,
        key: PhysicalKey::A,
        modifiers: Modifiers::empty(),
        consumed_modifiers: Modifiers::empty(),
        composing: false,
        text: Some("x".into()),
        unshifted_codepoint: Some('x'),
    }))?;
    assert!(frame_text(&second.frame).contains("tail-after-pin"));

    fs::write(&clear, b"clear")?;
    second.wait_title("pruned")?;
    second.request_frame(&ClientMessage::Resize(SurfaceSize {
        cols: 40,
        rows: 12,
        screen_width: 320,
        screen_height: 192,
        cell_width: 8,
        cell_height: 16,
        padding_top: 0,
        padding_bottom: 0,
        padding_left: 0,
        padding_right: 0,
    }))?;
    assert_eq!(
        (second.frame.dimensions.cols, second.frame.dimensions.rows),
        (40, 12)
    );

    let started = Instant::now();
    for _ in 0..32 {
        second.wheel(session::MouseButton::Four)?;
    }
    let wheel_elapsed = started.elapsed();
    let frame_bytes =
        session::encode_server_message(&ServerMessage::Frame(Box::new(second.frame.clone())))?
            .len();
    eprintln!(
        "viewport measurement: 32 wheel round trips in {wheel_elapsed:?}; complete ORBS frame {frame_bytes} bytes"
    );
    assert!(wheel_elapsed < Duration::from_secs(5));

    let wheel = encode_client_message(&ClientMessage::Mouse(session::MouseEvent {
        action: session::MouseAction::Press,
        button: Some(session::MouseButton::Four),
        modifiers: Modifiers::empty(),
        x: 1.0,
        y: 1.0,
    }))?;
    let _ = second.reader.get_mut().write_all(&wheel.repeat(4096));
    fs::write(&pressure, b"pressure")?;
    let mut third = Client::attach(&socket)?;
    drop(second);
    third.wait_title("pressure-complete")?;

    fs::write(&stop, b"stop")?;
    drop(third);
    assert!(wait_bounded(&mut server)?.success());
    assert!(!socket.exists());
    Ok(())
}

#[test]
fn authoritative_input_modes_and_resize_survive_detach() -> TestResult {
    let dir = TestDir::new("input-modes")?;
    let socket = dir.0.join("orbit.sock");
    let first_done = dir.0.join("first-done");
    let kitty_done = dir.0.join("kitty-done");
    let release = dir.0.join("release");
    let modes_off = dir.0.join("modes-off");
    let size = dir.0.join("size");
    let result = dir.0.join("result");
    let mut server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg(
                "stty -echo -icanon -icrnl min 1 time 0; \
                 printf '\\033[?1h\\033[?1000h\\033[?1006h\\033[?1004h\\033[?2004h\\033]2;input-modes-on\\033\\\\'; \
                 dd bs=1 count=34 of=\"$ORBIT_DIR/first\" 2>/dev/null; \
                 stty size > \"$ORBIT_SIZE\"; printf done > \"$ORBIT_FIRST_DONE\"; \
                 printf '\\033[>11u\\033]2;kitty-on\\033\\\\'; \
                 dd bs=1 count=9 of=\"$ORBIT_DIR/kitty\" 2>/dev/null; \
                 printf done > \"$ORBIT_KITTY_DONE\"; \
                 while [ ! -e \"$ORBIT_RELEASE\" ]; do sleep 0.01; done; \
                 printf '\\033[<u\\033[?1l\\033[?1000l\\033[?1006l\\033[?1004l\\033[?2004l\\033]2;input-modes-off\\033\\\\'; \
                 printf done > \"$ORBIT_MODES_OFF\"; \
                 dd bs=1 count=7 of=\"$ORBIT_DIR/second\" 2>/dev/null; \
                 stty min 0 time 1; dd bs=1 count=1 of=\"$ORBIT_DIR/extra\" 2>/dev/null; \
                 if [ -s \"$ORBIT_DIR/extra\" ]; then printf duplicate > \"$ORBIT_RESULT\"; exit 1; fi; \
                 printf ok > \"$ORBIT_RESULT\"",
            )
            .env("ORBIT_DIR", &dir.0)
            .env("ORBIT_FIRST_DONE", &first_done)
            .env("ORBIT_KITTY_DONE", &kitty_done)
            .env("ORBIT_RELEASE", &release)
            .env("ORBIT_MODES_OFF", &modes_off)
            .env("ORBIT_SIZE", &size)
            .env("ORBIT_RESULT", &result)
            .spawn()?,
    );

    let mut first = Client::attach(&socket)?;
    first.wait_title("input-modes-on")?;
    first.request(&ClientMessage::Resize(SurfaceSize {
        cols: 100,
        rows: 40,
        screen_width: 920,
        screen_height: 740,
        cell_width: 9,
        cell_height: 17,
        padding_top: 30,
        padding_bottom: 30,
        padding_left: 10,
        padding_right: 10,
    }))?;
    first.text("q", Modifiers::ALT)?;
    first.key(KeyAction::Press, PhysicalKey::ARROW_UP, Modifiers::empty())?;
    first.request(&ClientMessage::Mouse(session::MouseEvent {
        action: session::MouseAction::Press,
        button: Some(session::MouseButton::Left),
        modifiers: Modifiers::SHIFT,
        x: 915.0,
        y: 725.0,
    }))?;
    first.request(&ClientMessage::Focus(FocusEvent::Gained))?;
    first.paste(b"a\nb".to_vec())?;
    assert_eq!(wait_file_text(&first_done)?, "done");
    assert_eq!(
        fs::read(dir.0.join("first"))?,
        b"q\x1bOA\x1b[<4;100;40M\x1b[I\x1b[200~a\nb\x1b[201~"
    );
    assert_eq!(wait_file_text(&size)?.trim(), "40 100");

    first.wait_title("kitty-on")?;
    first.key(KeyAction::Release, PhysicalKey::ENTER, Modifiers::SHIFT)?;
    assert_eq!(wait_file_text(&kitty_done)?, "done");
    assert_eq!(fs::read(dir.0.join("kitty"))?, b"\x1b[13;2:3u");
    drop(first);

    fs::write(&release, b"release")?;
    assert_eq!(wait_file_text(&modes_off)?, "done");
    let mut second = Client::attach(&socket)?;
    second.wait_title("input-modes-off")?;
    assert_eq!(
        (second.frame.dimensions.cols, second.frame.dimensions.rows),
        (100, 40)
    );
    second.text("z", Modifiers::empty())?;
    second.key(KeyAction::Press, PhysicalKey::ARROW_UP, Modifiers::empty())?;
    second.request(&ClientMessage::Mouse(session::MouseEvent {
        action: session::MouseAction::Press,
        button: Some(session::MouseButton::Left),
        modifiers: Modifiers::empty(),
        x: 1.0,
        y: 1.0,
    }))?;
    second.request(&ClientMessage::Focus(FocusEvent::Gained))?;
    second.key(KeyAction::Release, PhysicalKey::ENTER, Modifiers::empty())?;
    second.paste(b"c\nd".to_vec())?;

    assert_eq!(wait_file_text(&result)?, "ok");
    assert_eq!(fs::read(dir.0.join("second"))?, b"z\x1b[Ac\rd");
    drop(second);
    assert!(wait_bounded(&mut server)?.success());
    assert!(!socket.exists());
    Ok(())
}

#[test]
fn parser_state_and_terminal_replies_survive_client_failure() -> TestResult {
    let dir = TestDir::new("terminal-authority")?;
    let socket = dir.0.join("orbit.sock");
    let begin = dir.0.join("begin");
    let partial = dir.0.join("partial");
    let release = dir.0.join("release");
    let answered = dir.0.join("answered");
    let check_duplicate = dir.0.join("check-duplicate");
    let result = dir.0.join("result");
    let stop = dir.0.join("stop");
    let mut server = Server(
        server_command()
            .arg(&socket)
            .arg("--")
            .arg("/bin/sh")
            .arg("-c")
            .arg(
                "stty -echo -icanon min 1 time 0; \
                 while [ ! -e \"$ORBIT_BEGIN\" ]; do sleep 0.01; done; \
                 printf '\\033[?104'; printf partial > \"$ORBIT_PARTIAL\"; \
                 while [ ! -e \"$ORBIT_RELEASE\" ]; do sleep 0.01; done; \
                 printf '9h\\033[?25l\\033[H\\033[6n'; \
                 reply=$(dd bs=1 count=6 2>/dev/null); \
                 if [ \"$reply\" != \"$(printf '\\033[1;1R')\" ]; then printf wrong > \"$ORBIT_RESULT\"; exit 1; fi; \
                 printf answered > \"$ORBIT_ANSWERED\"; printf '\\033]2;authoritative-reply\\033\\\\'; \
                 while [ ! -e \"$ORBIT_CHECK_DUPLICATE\" ]; do sleep 0.01; done; \
                 stty min 0 time 1; extra=$(dd bs=1 count=1 2>/dev/null); stty sane; \
                 if [ -n \"$extra\" ]; then printf duplicate > \"$ORBIT_RESULT\"; exit 1; fi; \
                 printf once > \"$ORBIT_RESULT\"; \
                 while [ ! -e \"$ORBIT_STOP\" ]; do sleep 0.01; done; \
                 printf '\\033[?1049l'",
            )
            .env("ORBIT_BEGIN", &begin)
            .env("ORBIT_PARTIAL", &partial)
            .env("ORBIT_RELEASE", &release)
            .env("ORBIT_ANSWERED", &answered)
            .env("ORBIT_CHECK_DUPLICATE", &check_duplicate)
            .env("ORBIT_RESULT", &result)
            .env("ORBIT_STOP", &stop)
            .spawn()?,
    );

    let mut first = Client::attach(&socket)?;
    let initial_revision = first.frame.revision;
    fs::write(&begin, b"begin")?;
    assert_eq!(wait_file_text(&partial)?, "partial");
    while first.frame.revision == initial_revision {
        if let ServerMessage::Frame(frame) = read_message(&mut first.reader)? {
            first.frame = *frame;
        }
    }
    drop(first);

    fs::write(&release, b"release")?;
    assert_eq!(wait_file_text(&answered)?, "answered");
    let mut second = Client::attach(&socket)?;
    second.wait_title("authoritative-reply")?;
    assert_eq!(second.frame.screen, Screen::Alternate);
    assert!(!second.frame.cursor.visible);
    fs::write(&check_duplicate, b"check")?;
    assert_eq!(wait_file_text(&result)?, "once");

    fs::write(&stop, b"stop")?;
    let exit_code = loop {
        match read_message(&mut second.reader)? {
            ServerMessage::Frame(frame) => second.frame = *frame,
            ServerMessage::Exited { code } => break code,
            _ => {}
        }
    };
    assert_eq!(exit_code, 0);
    assert_eq!(second.frame.screen, Screen::Primary);
    drop(second);
    assert!(wait_bounded(&mut server)?.success());
    assert!(!socket.exists());
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
