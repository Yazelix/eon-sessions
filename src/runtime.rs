use crate::{Result, attachment, interaction, management, platform, presentation};

use libghostty_vt::{
    Terminal, TerminalOptions, ffi,
    style::RgbColor,
    terminal::{ClipboardWrite, ClipboardWriteError, Mode},
};
use orbit_protocol::session::{self, ClipboardLocation, ServerMessage, SurfaceSize};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

use attachment::{Client, Incoming};
use interaction::{SelectionState, apply_pty_output, clear_selection, handle_client_message};
use platform::{Pty, PtyIo};
use presentation::Extractor;

pub(crate) const MAX_PTY_WRITE_BYTES: usize = session::MAX_PASTE_BYTES + 16;
const MAX_EXIT_PTY_READS: usize = 4;
const MAX_PENDING_CLIPBOARD_WRITES: usize = 64;
const SYNCHRONIZED_OUTPUT_TIMEOUT: Duration = Duration::from_secs(1);

pub(crate) const INITIAL_SIZE: SurfaceSize = SurfaceSize {
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

pub(crate) struct Presentation {
    pub(crate) extractor: Extractor,
    pub(crate) revision: u64,
    pub(crate) synchronized_until: Option<Instant>,
}

impl Presentation {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            extractor: Extractor::new()?,
            revision: 0,
            synchronized_until: None,
        })
    }

    pub(crate) fn advance(&mut self) -> Result {
        self.revision = next_revision(self.revision)?;
        Ok(())
    }

    fn publish_change(
        &mut self,
        client: Option<&mut Client>,
        terminal: &Terminal<'static, '_>,
    ) -> Result<bool> {
        if self.synchronized_until.is_none() {
            self.advance()?;
        }
        self.publish(client, terminal)
    }

    pub(crate) fn publish(
        &mut self,
        client: Option<&mut Client>,
        terminal: &Terminal<'static, '_>,
    ) -> Result<bool> {
        if terminal.mode(Mode::SYNC_OUTPUT)? {
            self.synchronized_until
                .get_or_insert_with(|| Instant::now() + SYNCHRONIZED_OUTPUT_TIMEOUT);
            return Ok(true);
        }
        self.synchronized_until = None;
        let Some(client) = client.filter(|client| client.accepts_output()) else {
            return Ok(true);
        };
        let frame = match self.extractor.frame(self.revision, terminal) {
            Ok(frame) => frame,
            Err(error) => {
                eprintln!("orbit: presentation client disconnected: {error}");
                return Ok(false);
            }
        };
        client.push_frame(frame)
    }

    fn release_if_expired(
        &mut self,
        client: Option<&mut Client>,
        terminal: &mut Terminal<'static, '_>,
        now: Instant,
    ) -> Result<bool> {
        match self.synchronized_until {
            Some(deadline) if now >= deadline => self.release(client, terminal),
            _ => Ok(true),
        }
    }

    fn release(
        &mut self,
        client: Option<&mut Client>,
        terminal: &mut Terminal<'static, '_>,
    ) -> Result<bool> {
        if self.synchronized_until.is_none() {
            return Ok(true);
        }
        terminal.set_mode(Mode::SYNC_OUTPUT, false)?;
        self.publish(client, terminal)
    }
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

pub(crate) fn install_ansi_palette(
    terminal: &mut Terminal<'_, '_>,
    colors: Option<[RgbColor; 16]>,
) -> Result {
    if let Some(colors) = colors {
        let mut palette = terminal.default_color_palette()?;
        palette.0[..16].copy_from_slice(&colors);
        terminal.set_default_color_palette(Some(palette))?;
    }
    Ok(())
}

pub(super) fn run(
    socket: &Path,
    command: &[String],
    ansi_palette: Option<[RgbColor; 16]>,
    management_launch: Option<management::Launch>,
) -> Result<i32> {
    platform::install_shutdown_signals()?;
    let (listener, _socket_guard) = platform::create_listener(socket)?;
    let prepared_management = management_launch
        .map(|launch| launch.prepare(socket))
        .transpose()?;
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
    install_ansi_palette(&mut terminal, ansi_palette)?;
    terminal.resize(size.cols, size.rows, size.cell_width, size.cell_height)?;
    terminal.on_pty_write(move |_, bytes| {
        if !queue_pty_write(&mut response_sink.borrow_mut(), bytes) {
            overflow_sink.set(true);
        }
    })?;
    let clipboard_writes = Rc::new(RefCell::new(PendingClipboardWrites::default()));
    let clipboard_sink = Rc::clone(&clipboard_writes);
    terminal.on_clipboard_write(move |_, write| clipboard_sink.borrow_mut().capture(write))?;

    let mut presentation = Presentation::new()?;
    let mut client: Option<Client> = None;
    let mut management_owner = prepared_management
        .map(|prepared| prepared.publish(socket))
        .transpose()?;
    let mut selection = SelectionState::default();
    let mut pty_open = true;
    loop {
        clipboard_writes.borrow_mut().enabled = client.as_ref().is_some_and(Client::accepts_output);
        if let Some(owner) = &mut management_owner {
            owner.release_expired(Instant::now());
        }
        if platform::termination_requested() {
            let status = pty
                .stop_and_reap()?
                .ok_or("PTY child status unavailable after explicit shutdown")?;
            if let Some(owner) = &mut management_owner {
                owner.finish_explicit(status)?;
            }
            return Ok(0);
        }
        if let Some(status) = pty.try_wait()? {
            let _ = pty.stop_and_reap()?;
            let changed = clear_selection(&terminal, &mut selection, true)?
                | drain_exited_pty(&mut pty, &mut terminal, &mut selection)?;
            fail_on_pty_write_overflow(&response_overflow)?;
            if !publish_clipboard_writes(&mut client, &clipboard_writes)? {
                disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
            }
            if changed && !presentation.publish_change(client.as_mut(), &terminal)? {
                disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
            }
            if !presentation.release(client.as_mut(), &mut terminal)? {
                disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
            }
            let code = status.code().unwrap_or(1);
            if let Some(client) = &mut client {
                client.finish_session(code)?;
            }
            if let Some(owner) = &mut management_owner {
                owner.finish_natural(status)?;
            }
            return Ok(code);
        }
        if client
            .as_ref()
            .is_some_and(|client| client.is_expired(Instant::now()))
        {
            disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
        }
        if !presentation.release_if_expired(client.as_mut(), &mut terminal, Instant::now())? {
            disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
        }

        let readiness = platform::poll(
            &listener,
            management_owner.as_ref().map(management::Owner::listener),
            pty_open.then(|| (&pty, !writes.borrow().is_empty())),
            client.as_ref().and_then(Client::poll_stream),
            management_owner
                .as_ref()
                .and_then(management::Owner::client_readiness),
        )?;
        let pty_was_open = pty_open;

        if readiness.listener {
            attachment::accept(&listener, &mut client)?;
        }
        if readiness.management_listener
            && let Some(owner) = &mut management_owner
        {
            owner.accept()?;
        }
        if readiness.pty_read {
            let (open, changed) = read_pty_turn(&mut pty, &mut terminal, &mut selection)?;
            fail_on_pty_write_overflow(&response_overflow)?;
            pty_open = open;
            if !publish_clipboard_writes(&mut client, &clipboard_writes)? {
                disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
            }
            if changed && !presentation.publish_change(client.as_mut(), &terminal)? {
                disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
            }
        }
        if readiness.pty_write && pty_open {
            pty_open = flush_pty(&mut pty, &mut writes.borrow_mut())?;
        }
        if pty_was_open && !pty_open {
            discard_pty_writes(&writes);
        }
        if readiness.client
            && let Some(active) = client.as_mut()
            && !read_client(
                active,
                &mut terminal,
                pty_open.then_some(&pty),
                &mut size,
                &writes,
                &mut presentation,
                &mut selection,
            )?
        {
            disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
        }
        let management_stop = if readiness.management_client
            && let Some(owner) = &mut management_owner
        {
            owner.read_request()?
        } else {
            false
        };
        if management_stop {
            if pty.try_wait()?.is_some() {
                continue;
            }
            let status = pty
                .stop_and_reap()?
                .ok_or("PTY child status unavailable after management stop")?;
            let owner = management_owner
                .as_mut()
                .expect("management stop has one owner");
            let tombstone = owner.finish_explicit(status)?;
            owner.reply_stopped(&tombstone)?;
            return Ok(0);
        }
        if let Some(owner) = &mut management_owner {
            owner.flush()?;
        }
        if let Some(active) = &mut client
            && !active.flush()?
        {
            disconnect_client(&mut client, &terminal, &mut selection, &mut presentation)?;
        }
    }
}

fn discard_pty_writes(writes: &RefCell<VecDeque<u8>>) {
    drop(writes.take());
}

pub(crate) fn queue_pty_write(writes: &mut VecDeque<u8>, bytes: &[u8]) -> bool {
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
    let Some(client) = active.as_mut().filter(|client| client.accepts_output()) else {
        pending.clear();
        return Ok(true);
    };
    if pending.overflowed {
        pending.clear();
        return Ok(false);
    }
    while let Some((location, text)) = pending.writes.pop_front() {
        if !client.push_message(&ServerMessage::ClipboardWrite { location, text })? {
            pending.clear();
            return Ok(false);
        }
    }
    Ok(true)
}

fn read_client(
    client: &mut Client,
    terminal: &mut Terminal<'static, '_>,
    pty: Option<&Pty>,
    size: &mut SurfaceSize,
    writes: &RefCell<VecDeque<u8>>,
    presentation: &mut Presentation,
    selection: &mut SelectionState,
) -> Result<bool> {
    if !client.read_ready()? {
        return Ok(false);
    }
    loop {
        match client.next_incoming()? {
            Incoming::Attached => {
                if !presentation.publish(Some(client), terminal)? {
                    return Ok(false);
                }
            }
            Incoming::Message(message) => {
                if !handle_client_message(
                    client,
                    message,
                    terminal,
                    pty,
                    size,
                    writes,
                    presentation,
                    selection,
                )? {
                    return Ok(false);
                }
            }
            Incoming::Pending => return Ok(true),
            Incoming::Disconnect => return Ok(false),
        }
        if client.is_closing() {
            return Ok(true);
        }
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

pub(crate) fn next_revision(revision: u64) -> Result<u64> {
    revision
        .checked_add(1)
        .ok_or_else(|| "presentation revision exhausted".into())
}

fn disconnect_client(
    client: &mut Option<Client>,
    terminal: &Terminal<'_, '_>,
    selection: &mut SelectionState,
    presentation: &mut Presentation,
) -> Result {
    *client = None;
    if clear_selection(terminal, selection, true)? {
        presentation.advance()?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::diagnostic::{read_message, write_message};
    use orbit_protocol::session::{
        ClientMessage, Failure, FailureCode, Modifiers, MouseAction, MouseButton,
    };
    use std::{io::Read, os::unix::net::UnixStream, thread, time::Instant};

    pub(crate) fn terminal() -> Result<Terminal<'static, 'static>> {
        terminal_with_scrollback(0)
    }

    pub(crate) fn terminal_with_scrollback(
        max_scrollback: usize,
    ) -> Result<Terminal<'static, 'static>> {
        let size = INITIAL_SIZE;
        let mut terminal = Terminal::new(TerminalOptions {
            cols: size.cols,
            rows: size.rows,
            max_scrollback,
        })?;
        terminal.resize(size.cols, size.rows, size.cell_width, size.cell_height)?;
        Ok(terminal)
    }

    pub(crate) fn attached_client() -> Result<(Client, UnixStream)> {
        Client::test_pair(None)
    }

    fn closing_client() -> Result<(Client, UnixStream, Vec<u8>)> {
        let (mut client, peer) = attached_client()?;
        let failure = ServerMessage::Failure(Failure {
            code: FailureCode::Protocol,
            detail: "terminal".into(),
        });
        let expected = session::encode_server_message(&failure)?;
        assert!(client.push_message(&failure)?);
        client.close_when_flushed();
        Ok((client, peer, expected))
    }

    pub(crate) fn wheel(button: MouseButton) -> ClientMessage {
        ClientMessage::Mouse(session::MouseEvent {
            action: MouseAction::Press,
            button: Some(button),
            modifiers: Modifiers::empty(),
            x: 1.0,
            y: 1.0,
        })
    }

    pub(crate) fn flush_message(
        client: &mut Client,
        peer: &mut UnixStream,
    ) -> Result<ServerMessage> {
        let _ = client.flush()?;
        read_message(peer)?.ok_or_else(|| "client output closed".into())
    }

    fn expect_frame(
        client: &mut Client,
        peer: &mut UnixStream,
        revision: u64,
        title: &str,
    ) -> Result {
        let ServerMessage::Frame(frame) = flush_message(client, peer)? else {
            return Err("client output was not a frame".into());
        };
        assert_eq!((frame.revision, frame.title.as_str()), (revision, title));
        Ok(())
    }

    pub(crate) fn fill_output(client: &mut Client) -> Result {
        let failure = ServerMessage::Failure(Failure {
            code: FailureCode::Terminal,
            detail: "x".repeat(session::MAX_FAILURE_BYTES),
        });
        while client.push_message(&failure)? {}
        while client.push_message(&ServerMessage::Accepted)? {}
        Ok(())
    }

    #[test]
    fn synchronized_presentation_coalesces_defers_and_times_out() -> Result {
        let (mut client, mut peer) = attached_client()?;
        let mut terminal = terminal()?;
        let mut presentation = Presentation::new()?;

        terminal.vt_write(b"ordinary");
        assert!(presentation.publish_change(Some(&mut client), &terminal)?);
        expect_frame(&mut client, &mut peer, 1, "")?;

        terminal.vt_write(b"\x1b[?2026h\x1b]2;held-one\x1b\\");
        assert!(presentation.publish_change(Some(&mut client), &terminal)?);
        terminal.vt_write(b"\x1b]2;held-two\x1b\\");
        assert!(presentation.publish_change(Some(&mut client), &terminal)?);
        assert_eq!(presentation.revision, 2);
        assert!(client.output_is_empty());

        terminal.vt_write(b"\x1b[?2026l");
        assert!(presentation.publish_change(Some(&mut client), &terminal)?);
        expect_frame(&mut client, &mut peer, 2, "held-two")?;

        terminal.vt_write(b"\x1b[?2026h\x1b]2;single-read\x1b\\\x1b[?2026l");
        assert!(presentation.publish_change(Some(&mut client), &terminal)?);
        expect_frame(&mut client, &mut peer, 3, "single-read")?;

        terminal.vt_write(b"\x1b[?2026h\x1b]2;deferred-initial\x1b\\");
        assert!(presentation.publish_change(None, &terminal)?);
        let deadline = presentation
            .synchronized_until
            .expect("synchronized output has a deadline");
        let (mut attaching, mut attaching_peer) = Client::test_pair(Some(deadline))?;
        write_message(&mut attaching_peer, &ClientMessage::Hello)?;
        assert!(attaching.read_ready()?);
        assert!(matches!(attaching.next_incoming()?, Incoming::Attached));
        assert!(presentation.publish(Some(&mut attaching), &terminal)?);
        assert!(matches!(
            flush_message(&mut attaching, &mut attaching_peer)?,
            ServerMessage::Attached
        ));
        assert!(attaching.output_is_empty());
        assert!(presentation.release_if_expired(
            Some(&mut attaching),
            &mut terminal,
            deadline - Duration::from_millis(1),
        )?);
        assert!(attaching.output_is_empty());
        assert!(terminal.mode(Mode::SYNC_OUTPUT)?);
        assert!(presentation.release_if_expired(Some(&mut attaching), &mut terminal, deadline)?);
        assert!(!terminal.mode(Mode::SYNC_OUTPUT)?);
        assert!(!attaching.initial_frame_pending());

        terminal.vt_write(b"\x1b]2;live-after-timeout\x1b\\");
        assert!(presentation.publish_change(Some(&mut attaching), &terminal)?);
        expect_frame(&mut attaching, &mut attaching_peer, 4, "deferred-initial")?;
        expect_frame(&mut attaching, &mut attaching_peer, 5, "live-after-timeout")?;

        terminal.vt_write(b"\x1b[?2026h\x1b]2;exit-release\x1b\\");
        assert!(presentation.publish_change(Some(&mut attaching), &terminal)?);
        assert!(attaching.output_is_empty());
        assert!(presentation.release(Some(&mut attaching), &mut terminal)?);
        assert!(!terminal.mode(Mode::SYNC_OUTPUT)?);
        expect_frame(&mut attaching, &mut attaching_peer, 6, "exit-release")?;

        let (mut blocked, _) = attached_client()?;
        fill_output(&mut blocked)?;
        terminal.vt_write(b"\x1b[?2026hpressure");
        assert!(presentation.publish_change(Some(&mut blocked), &terminal)?);
        terminal.vt_write(b"\x1b[?2026l");
        assert!(!presentation.publish_change(Some(&mut blocked), &terminal)?);
        assert_eq!(presentation.revision, 7);
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
        fill_output(&mut client)?;
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
    fn terminal_closing_client_cannot_apply_later_input() -> Result {
        let (mut client, mut peer) = attached_client()?;
        write_message(&mut peer, &ClientMessage::Paste(b"ignored".to_vec()))?;
        client.close_when_flushed();
        let pty = Pty::spawn(&["/bin/sh".into()], INITIAL_SIZE)?;
        let mut terminal = terminal()?;
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut presentation = Presentation::new()?;
        let mut selection = SelectionState::default();

        assert!(read_client(
            &mut client,
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert!(writes.borrow().is_empty());
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
        let mut presentation = Presentation::new()?;
        let mut selection = SelectionState::default();

        assert!(handle_client_message(
            &mut client,
            ClientMessage::Paste(b"rejected".to_vec()),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(writes.borrow().len(), MAX_PTY_WRITE_BYTES);
        assert!(client.is_closing());
        let _ = client.flush()?;
        assert_eq!(
            read_message(&mut peer)?,
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
        let mut presentation = Presentation::new()?;
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
            &mut presentation,
            &mut selection,
        )?);
        assert!(writes.borrow().is_empty());
        let _ = client.flush()?;
        assert_eq!(
            read_message(&mut peer)?,
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
        let mut presentation = Presentation::new()?;
        presentation.revision = 1;

        presentation.publish(active.as_mut(), &terminal)?;
        let client = active
            .as_mut()
            .expect("closing client remains while draining");
        let _ = client.flush()?;
        assert!(client.output_is_empty());
        drop(active);

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
                let _ = pty.stop_and_reap();
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
