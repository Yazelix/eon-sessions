use crate::Result;
use orbit_protocol::session::SurfaceSize;
use std::{
    env,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
            net::{UnixListener, UnixStream},
            process::CommandExt,
        },
    },
    path::{Component, Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static TERMINATE: AtomicBool = AtomicBool::new(false);
static NEXT_CONTAINMENT: AtomicU64 = AtomicU64::new(0);
const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const SHUTDOWN_GRACE: Duration = Duration::from_millis(500);
const SHUTDOWN_LIMIT: Duration = Duration::from_secs(2);
const SHUTDOWN_POLL: Duration = Duration::from_millis(10);
const PTY_EIO_RETRY_DELAY: Duration = Duration::from_millis(10);

pub(crate) enum PtyIo {
    Ready(usize),
    Blocked,
    Closed,
}

pub(crate) struct Pty {
    master: File,
    child: Option<Child>,
    containment: SessionContainment,
}

impl Pty {
    pub(crate) fn spawn(command: &[String], size: SurfaceSize) -> Result<Self> {
        if command.is_empty() {
            return Err("PTY command cannot be empty".into());
        }
        let (containment, membership) = SessionContainment::create()?;

        let mut master_fd = -1;
        let mut slave_fd = -1;
        let winsize = winsize(size);
        let result = unsafe {
            libc::openpty(
                &raw mut master_fd,
                &raw mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null(),
                &winsize,
            )
        };
        if result == -1 {
            return Err(io::Error::last_os_error().into());
        }

        let master = unsafe { File::from_raw_fd(master_fd) };
        let slave = unsafe { File::from_raw_fd(slave_fd) };
        set_fd_flags(master.as_raw_fd(), libc::O_NONBLOCK)?;
        set_fd_flags(slave.as_raw_fd(), 0)?;

        let stdin = slave.try_clone()?;
        let stdout = slave.try_clone()?;
        let mut child_command = Command::new(&command[0]);
        child_command
            .args(&command[1..])
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(slave))
            .env("TERM", "eon")
            .env("COLORTERM", "truecolor");
        let membership_fd = membership.as_raw_fd();
        unsafe {
            child_command.pre_exec(move || {
                let member = b"0";
                if libc::write(membership_fd, member.as_ptr().cast(), member.len())
                    != member.len() as isize
                {
                    return Err(io::Error::last_os_error());
                }
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::ioctl(libc::STDIN_FILENO, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = child_command.spawn()?;
        drop(membership);
        Ok(Self {
            master,
            child: Some(child),
            containment,
        })
    }

    pub(crate) fn resize(&self, size: SurfaceSize) -> Result {
        let winsize = winsize(size);
        let result = unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) };
        if result == -1 {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child
            .as_mut()
            .expect("PTY child already reaped")
            .try_wait()
    }

    pub(crate) fn read(&mut self, bytes: &mut [u8]) -> Result<PtyIo> {
        nonblocking(|| self.master.read(bytes), self.child.is_some())
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<PtyIo> {
        nonblocking(|| self.master.write(bytes), false)
    }

    pub(crate) fn stop_and_reap(&mut self) -> Result {
        let Some(child) = self.child.as_mut() else {
            return self.containment.remove();
        };
        let child_pid = child.id() as libc::pid_t;
        let child_exited = child.try_wait()?.is_some();
        if !child_exited {
            unsafe {
                libc::kill(child_pid, libc::SIGHUP);
            }
        }

        if !self
            .containment
            .wait_empty(Instant::now() + SHUTDOWN_GRACE)?
        {
            self.containment.kill_all()?;
            if !self
                .containment
                .wait_empty(Instant::now() + SHUTDOWN_LIMIT)?
            {
                return Err("PTY Session containment did not become empty".into());
            }
        }

        if !child_exited {
            let deadline = Instant::now() + SHUTDOWN_LIMIT;
            loop {
                if child.try_wait()?.is_some() {
                    break;
                }
                if Instant::now() >= deadline {
                    return Err("PTY child was not reaped before the shutdown deadline".into());
                }
                thread::sleep(SHUTDOWN_POLL);
            }
        }
        self.child = None;
        self.containment.remove()
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.stop_and_reap();
    }
}

struct SessionContainment {
    path: Option<PathBuf>,
    device: u64,
    inode: u64,
    owner: u32,
}

impl SessionContainment {
    fn create() -> Result<(Self, File)> {
        let memberships = fs::read_to_string("/proc/self/cgroup")?;
        let relative = memberships
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .ok_or("Orbit requires a cgroup-v2 hierarchy")?
            .strip_prefix('/')
            .ok_or("current cgroup-v2 path is not absolute")?;
        if !Path::new(relative)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err("current cgroup-v2 path contains an unsafe component".into());
        }
        let parent = Path::new(CGROUP_ROOT).join(relative);
        let parent_metadata = fs::metadata(&parent)?;
        let owner = unsafe { libc::geteuid() };
        if !parent_metadata.is_dir()
            || parent_metadata.uid() != owner
            || parent_metadata.mode() & 0o022 != 0
        {
            return Err(format!(
                "current cgroup {} is not a user-owned non-writable-by-others directory",
                parent.display()
            )
            .into());
        }
        let own_pid = std::process::id().to_string();
        if !fs::read_to_string(parent.join("cgroup.procs"))?
            .lines()
            .any(|pid| pid == own_pid)
        {
            return Err(
                format!("current cgroup {} does not contain Orbit", parent.display()).into(),
            );
        }

        let path = parent.join(format!(
            "_yazelix_orbit_{}_{}",
            std::process::id(),
            NEXT_CONTAINMENT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        let result = (|| {
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_dir() || metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
                return Err(format!(
                    "Session containment {} has unexpected ownership",
                    path.display()
                )
                .into());
            }
            let membership = OpenOptions::new()
                .write(true)
                .open(path.join("cgroup.procs"))?;
            OpenOptions::new()
                .write(true)
                .open(path.join("cgroup.kill"))?;
            fs::read_to_string(path.join("cgroup.events"))?;
            Ok((
                Self {
                    path: Some(path.clone()),
                    device: metadata.dev(),
                    inode: metadata.ino(),
                    owner,
                },
                membership,
            ))
        })();
        if result.is_err() {
            let _ = fs::remove_dir(&path);
        }
        result
    }

    fn verify_identity(&self) -> Result<&Path> {
        let path = self
            .path
            .as_deref()
            .ok_or("Session containment was already removed")?;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_dir()
            || metadata.dev() != self.device
            || metadata.ino() != self.inode
            || metadata.uid() != self.owner
            || metadata.mode() & 0o077 != 0
        {
            return Err(format!("untrusted Session containment {}", path.display()).into());
        }
        Ok(path)
    }

    fn is_empty(&self) -> Result<bool> {
        let events = fs::read_to_string(self.verify_identity()?.join("cgroup.events"))?;
        let populated = events
            .lines()
            .find_map(|line| line.strip_prefix("populated "))
            .ok_or("cgroup.events is missing populated state")?;
        match populated {
            "0" => Ok(true),
            "1" => Ok(false),
            _ => Err("invalid populated value in cgroup.events".into()),
        }
    }

    fn wait_empty(&self, deadline: Instant) -> Result<bool> {
        loop {
            if self.is_empty()? {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            thread::sleep(SHUTDOWN_POLL);
        }
    }

    fn kill_all(&self) -> Result {
        let mut kill = OpenOptions::new()
            .write(true)
            .open(self.verify_identity()?.join("cgroup.kill"))?;
        kill.write_all(b"1")?;
        Ok(())
    }

    fn remove(&mut self) -> Result {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        if !self.is_empty()? {
            return Err("refusing to remove a populated Session containment".into());
        }
        fs::remove_dir(path)?;
        self.path = None;
        Ok(())
    }
}

impl Drop for SessionContainment {
    fn drop(&mut self) {
        if self.path.is_none() {
            return;
        }
        let _ = self.kill_all();
        let _ = self.wait_empty(Instant::now() + SHUTDOWN_LIMIT);
        let _ = self.remove();
    }
}

pub(crate) struct Readiness {
    pub(crate) listener: bool,
    pub(crate) pty_read: bool,
    pub(crate) pty_write: bool,
    pub(crate) client: bool,
}

pub(crate) fn poll(
    listener: &UnixListener,
    pty: Option<(&Pty, bool)>,
    client: Option<&UnixStream>,
) -> Result<Readiness> {
    let mut fds = [pollfd(0, 0); 3];
    let mut len = 0;
    let mut push = |fd, events| {
        let index = len;
        fds[index] = pollfd(fd, events);
        len += 1;
        index
    };
    let listener_index = push(listener.as_raw_fd(), libc::POLLIN);
    let pty_index = pty.map(|(pty, writable)| {
        push(
            pty.master.as_raw_fd(),
            libc::POLLIN | if writable { libc::POLLOUT } else { 0 },
        )
    });
    let client_index = client.map(|client| push(client.as_raw_fd(), libc::POLLIN));

    loop {
        let result = unsafe { libc::poll(fds.as_mut_ptr(), len as _, 100) };
        if result >= 0 {
            break;
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }

    let events = |index: Option<usize>| index.map_or(0, |index| fds[index].revents);
    let pty = events(pty_index);
    Ok(Readiness {
        listener: events(Some(listener_index)) & libc::POLLIN != 0,
        pty_read: pty & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0,
        pty_write: pty & libc::POLLOUT != 0,
        client: events(client_index) & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0,
    })
}

pub(crate) struct SocketGuard(PathBuf, Option<(u64, u64)>);

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if fs::symlink_metadata(&self.0).is_ok_and(|metadata| {
            metadata.file_type().is_socket()
                && self.1.is_none_or(|(device, inode)| {
                    metadata.dev() == device && metadata.ino() == inode
                })
        }) {
            let _ = fs::remove_file(&self.0);
        }
    }
}

pub(crate) fn create_listener(path: &Path) -> Result<(UnixListener, SocketGuard)> {
    let parent = validate_private_parent(path)?;
    let claim = File::open(parent)?;
    // ponytail: one accepted Session makes the parent the smallest artifact-free claim;
    // use a per-socket lock only if concurrent Sessions in one directory are accepted.
    match claim.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(format!("socket claim already in progress at {}", path.display()).into());
        }
        Err(TryLockError::Error(error)) => return Err(error.into()),
    }
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if !metadata.file_type().is_socket() {
            return Err(format!("refusing to replace non-socket path {}", path.display()).into());
        }
        match UnixStream::connect(path) {
            Ok(_) => return Err(format!("server already listening at {}", path.display()).into()),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) =>
            {
                fs::remove_file(path)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    let listener = UnixListener::bind(path)?;
    let mut guard = SocketGuard(path.to_owned(), None);
    let metadata = fs::symlink_metadata(path)?;
    guard.1 = Some((metadata.dev(), metadata.ino()));
    listener.set_nonblocking(true)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok((listener, guard))
}

pub(crate) fn default_socket_path() -> Result<PathBuf> {
    let euid = unsafe { libc::geteuid() };
    let directory = env::var_os("XDG_RUNTIME_DIR").map_or_else(
        || PathBuf::from(format!("/tmp/yazelix-orbit-{euid}")),
        |root| PathBuf::from(root).join("yazelix-orbit"),
    );
    if !directory.exists() {
        fs::DirBuilder::new().mode(0o700).create(&directory)?;
    }
    let metadata = fs::metadata(&directory)?;
    if metadata.uid() != euid || metadata.mode() & 0o077 != 0 {
        return Err(format!("runtime directory {} is not private", directory.display()).into());
    }
    Ok(directory.join("orbit.sock"))
}

pub(crate) fn install_shutdown_signals() -> Result {
    set_signal_handler(
        &[libc::SIGINT, libc::SIGTERM, libc::SIGHUP],
        terminate as *const () as usize,
    )
}

pub(crate) fn ignore_broken_pipe() -> Result {
    set_signal_handler(&[libc::SIGPIPE], libc::SIG_IGN)
}

pub(crate) fn termination_requested() -> bool {
    TERMINATE.load(Ordering::Relaxed)
}

fn winsize(size: SurfaceSize) -> libc::winsize {
    libc::winsize {
        ws_row: size.rows,
        ws_col: size.cols,
        ws_xpixel: u16::try_from(size.screen_width).unwrap_or(u16::MAX),
        ws_ypixel: u16::try_from(size.screen_height).unwrap_or(u16::MAX),
    }
}

fn nonblocking(mut operation: impl FnMut() -> io::Result<usize>, retry_eio: bool) -> Result<PtyIo> {
    loop {
        match operation() {
            Ok(0) => return Ok(PtyIo::Closed),
            Ok(count) => return Ok(PtyIo::Ready(count)),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(PtyIo::Blocked);
            }
            Err(error) if retry_eio && error.raw_os_error() == Some(libc::EIO) => {
                thread::sleep(PTY_EIO_RETRY_DELAY);
                return Ok(PtyIo::Blocked);
            }
            Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                return Ok(PtyIo::Closed);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn pollfd(fd: RawFd, events: libc::c_short) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}

fn validate_private_parent(path: &Path) -> Result<&Path> {
    let parent = path.parent().ok_or("socket path has no parent")?;
    let metadata = fs::metadata(parent)?;
    let euid = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.uid() != euid || metadata.mode() & 0o077 != 0 {
        return Err(format!(
            "socket parent {} must be a user-owned 0700 directory",
            parent.display()
        )
        .into());
    }
    Ok(parent)
}

fn set_fd_flags(fd: RawFd, status: libc::c_int) -> Result {
    let old_status = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if old_status == -1 || unsafe { libc::fcntl(fd, libc::F_SETFL, old_status | status) } == -1 {
        return Err(io::Error::last_os_error().into());
    }
    let old_descriptor = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if old_descriptor == -1
        || unsafe { libc::fcntl(fd, libc::F_SETFD, old_descriptor | libc::FD_CLOEXEC) } == -1
    {
        return Err(io::Error::last_os_error().into());
    }
    Ok(())
}

extern "C" fn terminate(_: libc::c_int) {
    TERMINATE.store(true, Ordering::Relaxed);
}

fn set_signal_handler(signals: &[libc::c_int], handler: usize) -> Result {
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = handler;
    unsafe {
        libc::sigemptyset(&mut action.sa_mask);
    }
    for &signal in signals {
        if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } == -1 {
            return Err(io::Error::last_os_error().into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winsize_preserves_surface_pixels_beyond_the_cell_grid() {
        let size = SurfaceSize {
            cols: 100,
            rows: 40,
            cell_width: 9,
            cell_height: 18,
            screen_width: 920,
            screen_height: 740,
            padding_top: 10,
            padding_bottom: 10,
            padding_left: 10,
            padding_right: 10,
        };

        let size = winsize(size);

        assert_eq!((size.ws_col, size.ws_row), (100, 40));
        assert_eq!((size.ws_xpixel, size.ws_ypixel), (920, 740));
    }
}
