#![cfg(target_os = "linux")]

use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
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
}

impl Client {
    fn attach(socket: &Path) -> TestResult<Self> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let stream = connect_bounded(socket, deadline)?;
            stream.set_read_timeout(Some(Duration::from_secs(2)))?;
            stream.set_write_timeout(Some(Duration::from_secs(2)))?;
            let mut reader = BufReader::new(stream);
            let mut response = String::new();
            match reader.read_line(&mut response) {
                Ok(read) if read > 0 && response.trim_end_matches(['\r', '\n']) == "ATTACHED" => {
                    return Ok(Self { reader });
                }
                Ok(read)
                    if Instant::now() < deadline && (read == 0 || response.trim() == "BUSY") => {}
                Err(error)
                    if Instant::now() < deadline
                        && matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionReset
                                | std::io::ErrorKind::ConnectionAborted
                                | std::io::ErrorKind::BrokenPipe
                                | std::io::ErrorKind::TimedOut
                        ) => {}
                Ok(_) => return Err(format!("unexpected attach response: {response:?}").into()),
                Err(error) => return Err(error.into()),
            }
            thread::yield_now();
        }
    }

    fn request(&mut self, request: &str) -> TestResult<String> {
        writeln!(self.reader.get_mut(), "{request}")?;
        self.read_line()
    }

    fn read_line(&mut self) -> TestResult<String> {
        let mut line = String::new();
        let read = self.reader.read_line(&mut line)?;
        if read == 0 {
            return Err("diagnostic connection closed".into());
        }
        Ok(line.trim_end_matches(['\r', '\n']).to_owned())
    }

    fn pid(&mut self) -> TestResult<u32> {
        Ok(self
            .request("PID")?
            .strip_prefix("PID ")
            .ok_or("missing PID response")?
            .parse()?)
    }

    fn wait_title(&mut self, expected: &str) -> TestResult {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let response = self.request("TITLE")?;
            let encoded = response
                .strip_prefix("TITLE ")
                .ok_or("missing TITLE response")?;
            if decode_hex(encoded)? == expected.as_bytes() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(
                    format!("title never became {expected:?}; last response: {response}").into(),
                );
            }
            thread::yield_now();
        }
    }
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

fn decode_hex(encoded: &str) -> TestResult<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        return Err("odd hexadecimal response".into());
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(pair, 16)?)
        })
        .collect()
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
fn shell_survives_detach_and_one_client_reattaches() -> TestResult {
    let dir = TestDir::new("lifecycle")?;
    let socket = dir.0.join("orbit.sock");
    let fifo = dir.0.join("release.fifo");
    let status = Command::new("mkfifo").arg(&fifo).status()?;
    assert!(status.success());

    let mut server = spawn_server(&socket)?;
    let mut first = Client::attach(&socket)?;
    let shell_pid = first.pid()?;

    assert_eq!(first.request("RESIZE 100 40 9 18")?, "OK");
    assert_eq!(
        first.request("PASTE printf '\\033]2;size-%s\\033\\\\' \"$(stty size)\"")?,
        "OK"
    );
    assert_eq!(first.request("KEY ENTER")?, "OK");
    first.wait_title("size-40 100")?;

    let mut rejected = UnixStream::connect(&socket)?;
    rejected.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut rejection = String::new();
    BufReader::new(&mut rejected).read_line(&mut rejection)?;
    assert_eq!(rejection.trim(), "BUSY");
    assert_eq!(first.request("PING")?, "PONG");

    assert_eq!(
        first.request(&format!(
            "PASTE printf '\\033]2;ready\\033\\\\'; read orbit_release < {}; printf '\\033]2;detached\\033\\\\'",
            fifo.display()
        ))?,
        "OK"
    );
    assert_eq!(first.request("KEY ENTER")?, "OK");
    first.wait_title("ready")?;
    drop(first);

    writeln!(OpenOptions::new().write(true).open(&fifo)?, "go")?;

    let mut second = Client::attach(&socket)?;
    assert_eq!(second.pid()?, shell_pid);
    second.wait_title("detached")?;
    assert_eq!(second.request("PASTE exit")?, "OK");
    assert_eq!(second.request("KEY ENTER")?, "OK");
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
    let shell_pid = client.pid()?;
    let foreground_pid_file = dir.0.join("foreground.pid");
    assert_eq!(
        client.request(&format!(
            "PASTE sh -c 'trap \"\" HUP; echo $$ > {}; printf \"\\033]2;foreground-ready\\033\\\\\"; exec sleep 60'",
            foreground_pid_file.display()
        ))?,
        "OK"
    );
    assert_eq!(client.request("KEY ENTER")?, "OK");
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
