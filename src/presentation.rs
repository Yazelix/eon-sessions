use crate::Result;
use libghostty_vt::{
    Error as GhosttyError, RenderState, Terminal,
    render::{CellIterator, RowIterator},
    screen::Screen,
    style::{RgbColor, Style, StyleColor},
    terminal::{Point, PointCoordinate},
};
use std::{collections::VecDeque, io, io::Write, os::unix::net::UnixStream};

pub(crate) const MAX_CELLS: usize = 100_000;
pub(crate) const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 2 * (MAX_FRAME_BYTES + 32) + 4096;
pub(crate) const MAGIC: &[u8; 4] = b"ORBF";
const VERSION: u16 = 1;

pub(crate) fn is_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
    )
}

pub(crate) struct Extractor {
    state: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
}

impl Extractor {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            state: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
        })
    }

    pub(crate) fn frame(
        &mut self,
        revision: u64,
        terminal: &Terminal<'static, '_>,
    ) -> Result<Vec<u8>> {
        let Self { state, rows, cells } = self;
        let snapshot = state.update(terminal)?;
        let cols = snapshot.cols()?;
        let row_count = snapshot.rows()?;
        if usize::from(cols) * usize::from(row_count) > MAX_CELLS {
            return Err("presentation dimensions exceed the cell bound".into());
        }

        let mut out = Encoder(Vec::with_capacity(32 * 1024));
        out.bytes(MAGIC)?;
        out.u16(VERSION)?;
        out.u64(revision)?;
        out.u16(cols)?;
        out.u16(row_count)?;
        out.u8(match terminal.active_screen()? {
            Screen::Primary => 0,
            Screen::Alternate => 1,
        })?;
        out.string(terminal.title()?.as_bytes())?;
        out.string(terminal.pwd()?.as_bytes())?;

        out.u8(1)?; // Hyperlinks are carried per cell.
        out.u8(0)?; // Kitty graphics are explicitly unsupported by frame version 1.

        let colors = snapshot.colors()?;
        out.rgb(colors.background)?;
        out.rgb(colors.foreground)?;
        out.u8(colors.cursor.is_some().into())?;
        if let Some(cursor) = colors.cursor {
            out.rgb(cursor)?;
        }
        for color in colors.palette {
            out.rgb(color)?;
        }

        out.u8(snapshot.cursor_visible()?.into())?;
        out.u8(snapshot.cursor_blinking()?.into())?;
        out.u8(snapshot.cursor_password_input()?.into())?;
        out.u8(u8::try_from(u32::from(snapshot.cursor_visual_style()?))?)?;
        if let Some(cursor) = snapshot.cursor_viewport()? {
            out.u8(1)?;
            out.u16(cursor.x)?;
            out.u16(cursor.y)?;
            out.u8(cursor.at_wide_tail.into())?;
        } else {
            out.u8(0)?;
        }

        let mut row_iter = rows.update(&snapshot)?;
        let mut graphemes = String::new();
        for y in 0..row_count {
            let row = row_iter.next().ok_or("render snapshot omitted a row")?;
            let raw_row = row.raw_row()?;
            let row_flags = u8::from(raw_row.is_wrapped()?)
                | (u8::from(raw_row.is_wrap_continuation()?) << 1)
                | (u8::from(raw_row.has_kitty_virtual_placeholder()?) << 2);
            out.u8(row_flags)?;

            let mut cell_iter = cells.update(row)?;
            for x in 0..cols {
                let cell = cell_iter.next().ok_or("render snapshot omitted a cell")?;
                let raw_cell = cell.raw_cell()?;
                out.u8(u8::try_from(u32::from(raw_cell.wide()?))?)?;
                let mut style = cell.style()?;
                if let Some(background) = cell.bg_color()? {
                    style.bg_color = StyleColor::Rgb(background);
                }
                out.style(style, cell.is_selected()?, raw_cell.is_protected()?)?;

                let grapheme_count = cell.graphemes_len()?;
                if grapheme_count > MAX_FRAME_BYTES / size_of::<char>() {
                    return Err("cell grapheme exceeds the frame bound".into());
                }
                graphemes.clear();
                cell.graphemes_utf8(&mut graphemes)?;
                out.string(graphemes.as_bytes())?;

                if raw_cell.has_hyperlink()? {
                    let reference = terminal
                        .grid_ref(Point::Viewport(PointCoordinate { x, y: u32::from(y) }))?;
                    out.string(&hyperlink(&reference)?)?;
                } else {
                    out.string(&[])?;
                }
            }
            if cell_iter.next().is_some() {
                return Err("render snapshot added an unexpected cell".into());
            }
        }
        if row_iter.next().is_some() {
            return Err("render snapshot added an unexpected row".into());
        }
        Ok(out.0)
    }
}

fn hyperlink(reference: &libghostty_vt::screen::GridRef<'_>) -> Result<Vec<u8>> {
    let mut bytes = vec![0; 256];
    let length = match reference.hyperlink_uri(&mut bytes) {
        Ok(length) => length,
        Err(GhosttyError::OutOfSpace { required }) if required <= MAX_FRAME_BYTES => {
            bytes.resize(required, 0);
            reference.hyperlink_uri(&mut bytes)?
        }
        Err(GhosttyError::OutOfSpace { .. }) => {
            return Err("hyperlink exceeds the frame bound".into());
        }
        Err(error) => return Err(error.into()),
    };
    bytes.truncate(length);
    Ok(bytes)
}

struct Encoder(Vec<u8>);

impl Encoder {
    fn bytes(&mut self, bytes: &[u8]) -> Result {
        if self.0.len().saturating_add(bytes.len()) > MAX_FRAME_BYTES {
            return Err("presentation frame exceeds the byte bound".into());
        }
        self.0.extend_from_slice(bytes);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result {
        self.bytes(&value.to_le_bytes())
    }

    fn string(&mut self, value: &[u8]) -> Result {
        self.bytes(&u32::try_from(value.len())?.to_le_bytes())?;
        self.bytes(value)
    }

    fn rgb(&mut self, color: RgbColor) -> Result {
        self.bytes(&[color.r, color.g, color.b])
    }

    fn style(&mut self, style: Style, selected: bool, protected: bool) -> Result {
        self.style_color(style.fg_color)?;
        self.style_color(style.bg_color)?;
        self.style_color(style.underline_color)?;
        let flags = u16::from(style.bold)
            | (u16::from(style.italic) << 1)
            | (u16::from(style.faint) << 2)
            | (u16::from(style.blink) << 3)
            | (u16::from(style.inverse) << 4)
            | (u16::from(style.invisible) << 5)
            | (u16::from(style.strikethrough) << 6)
            | (u16::from(style.overline) << 7)
            | (u16::from(selected) << 8)
            | (u16::from(protected) << 9);
        self.u16(flags)?;
        self.u8(u8::try_from(u32::from(style.underline))?)
    }

    fn style_color(&mut self, color: StyleColor) -> Result {
        let (tag, value) = match color {
            StyleColor::None => (0, [0, 0, 0]),
            StyleColor::Palette(index) => (1, [index.0, 0, 0]),
            StyleColor::Rgb(color) => (2, [color.r, color.g, color.b]),
        };
        self.u8(tag)?;
        self.bytes(&value)
    }
}

struct Message {
    bytes: Vec<u8>,
    coalescible: bool,
}

#[derive(Default)]
pub(crate) struct OutputQueue {
    messages: VecDeque<Message>,
    offset: usize,
    bytes: usize,
}

impl OutputQueue {
    pub(crate) fn push_initial_frame(&mut self, frame: Vec<u8>) -> bool {
        self.push(frame_message(frame), false)
    }

    pub(crate) fn push_frame(&mut self, frame: Vec<u8>) -> bool {
        let message = frame_message(frame);
        if self.messages.back().is_some_and(|back| back.coalescible)
            && (self.messages.len() > 1 || self.offset == 0)
        {
            let previous = self.messages.pop_back().expect("back existed");
            self.bytes -= previous.bytes.len();
        }
        self.push(message, true)
    }

    pub(crate) fn push_line(&mut self, line: &str) -> bool {
        let mut message = Vec::with_capacity(line.len() + 1);
        message.extend_from_slice(line.as_bytes());
        message.push(b'\n');
        self.push(message, false)
    }

    fn push(&mut self, bytes: Vec<u8>, coalescible: bool) -> bool {
        if self.bytes.saturating_add(bytes.len()) > MAX_OUTPUT_BYTES {
            return false;
        }
        self.bytes += bytes.len();
        self.messages.push_back(Message { bytes, coalescible });
        true
    }

    pub(crate) fn flush(&mut self, stream: &mut UnixStream) -> io::Result<bool> {
        while let Some(message) = self.messages.front() {
            match stream.write(&message.bytes[self.offset..]) {
                Ok(0) => return Ok(false),
                Ok(written) => {
                    self.offset += written;
                    self.bytes -= written;
                    if self.offset == message.bytes.len() {
                        self.messages.pop_front();
                        self.offset = 0;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(true),
                Err(error) if is_disconnect(&error) => return Ok(false),
                Err(error) => return Err(error),
            }
        }
        Ok(true)
    }
}

fn frame_message(frame: Vec<u8>) -> Vec<u8> {
    let mut message = format!("FRAME {}\n", frame.len()).into_bytes();
    message.extend(frame);
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::{BufRead, BufReader, Read},
        os::unix::{fs::PermissionsExt, net::UnixStream},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        thread,
        time::{Duration, Instant},
    };

    type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> TestResult<Self> {
            let path = std::env::temp_dir().join(format!(
                "orbit-presentation-{}-{}",
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

    struct Server {
        thread: Option<thread::JoinHandle<()>>,
        stop: PathBuf,
    }

    impl Server {
        fn start(socket: PathBuf, command: Vec<String>, stop: PathBuf) -> Self {
            let thread = thread::spawn(move || {
                assert_eq!(crate::run_server(&socket, &command).unwrap(), 0);
            });
            Self {
                thread: Some(thread),
                stop,
            }
        }

        fn finish(mut self) -> TestResult {
            fs::write(&self.stop, b"stop")?;
            self.join()
        }

        fn join(&mut self) -> TestResult {
            self.thread
                .take()
                .expect("server thread exists")
                .join()
                .map_err(|_| "presentation server panicked".into())
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            if self.thread.is_some() {
                let _ = fs::write(&self.stop, b"stop");
                let _ = self.join();
            }
        }
    }

    struct Frame {
        revision: u64,
        cols: u16,
        rows: u16,
        screen: u8,
        title: String,
        pwd: String,
        hyperlinks: u8,
        graphics: u8,
        cursor: Option<(u16, u16)>,
        row_flags: Vec<u8>,
        cells: Vec<Cell>,
    }

    struct Cell {
        wide: u8,
        foreground: (u8, [u8; 3]),
        background: (u8, [u8; 3]),
        flags: u16,
        text: String,
        hyperlink: String,
    }

    struct Decoder<'a> {
        bytes: &'a [u8],
        position: usize,
    }

    impl<'a> Decoder<'a> {
        fn take(&mut self, length: usize) -> TestResult<&'a [u8]> {
            let end = self.position.checked_add(length).ok_or("frame overflow")?;
            let value = self
                .bytes
                .get(self.position..end)
                .ok_or("truncated frame")?;
            self.position = end;
            Ok(value)
        }

        fn u8(&mut self) -> TestResult<u8> {
            Ok(self.take(1)?[0])
        }

        fn u16(&mut self) -> TestResult<u16> {
            Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
        }

        fn u32(&mut self) -> TestResult<u32> {
            Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
        }

        fn u64(&mut self) -> TestResult<u64> {
            Ok(u64::from_le_bytes(self.take(8)?.try_into()?))
        }

        fn string(&mut self) -> TestResult<String> {
            let length = usize::try_from(self.u32()?)?;
            Ok(std::str::from_utf8(self.take(length)?)?.to_owned())
        }

        fn color(&mut self) -> TestResult<(u8, [u8; 3])> {
            Ok((self.u8()?, self.take(3)?.try_into()?))
        }
    }

    fn decode_frame(bytes: &[u8]) -> TestResult<Frame> {
        let mut decoder = Decoder { bytes, position: 0 };
        if decoder.take(MAGIC.len())? != MAGIC || decoder.u16()? != VERSION {
            return Err("unsupported presentation frame".into());
        }
        let revision = decoder.u64()?;
        let cols = decoder.u16()?;
        let rows = decoder.u16()?;
        let screen = decoder.u8()?;
        let title = decoder.string()?;
        let pwd = decoder.string()?;
        let hyperlinks = decoder.u8()?;
        let graphics = decoder.u8()?;
        decoder.take(6)?;
        let cursor_color = decoder.u8()?;
        decoder.take(usize::from(cursor_color) * 3 + 256 * 3)?;
        decoder.take(4)?;
        let cursor = if decoder.u8()? == 1 {
            let cursor = (decoder.u16()?, decoder.u16()?);
            decoder.u8()?;
            Some(cursor)
        } else {
            None
        };

        let mut row_flags = Vec::with_capacity(usize::from(rows));
        let mut cells = Vec::with_capacity(usize::from(cols) * usize::from(rows));
        for _ in 0..rows {
            row_flags.push(decoder.u8()?);
            for _ in 0..cols {
                let wide = decoder.u8()?;
                let foreground = decoder.color()?;
                let background = decoder.color()?;
                decoder.color()?;
                let flags = decoder.u16()?;
                decoder.u8()?;
                let text = decoder.string()?;
                let hyperlink = decoder.string()?;
                cells.push(Cell {
                    wide,
                    foreground,
                    background,
                    flags,
                    text,
                    hyperlink,
                });
            }
        }
        if decoder.position != bytes.len() {
            return Err("presentation frame has trailing bytes".into());
        }
        Ok(Frame {
            revision,
            cols,
            rows,
            screen,
            title,
            pwd,
            hyperlinks,
            graphics,
            cursor,
            row_flags,
            cells,
        })
    }

    fn read_frame(reader: &mut BufReader<UnixStream>) -> TestResult<Option<Frame>> {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Err("presentation connection closed".into());
        }
        if header.trim() == "BUSY" {
            return Ok(None);
        }
        let length: usize = header
            .trim_end_matches(['\r', '\n'])
            .strip_prefix("FRAME ")
            .ok_or_else(|| format!("non-frame bytes crossed the boundary: {header:?}"))?
            .parse()?;
        if length > MAX_FRAME_BYTES {
            return Err("frame exceeded its declared bound".into());
        }
        let mut payload = vec![0; length];
        reader.read_exact(&mut payload)?;
        Ok(Some(decode_frame(&payload)?))
    }

    fn attach(socket: &Path) -> TestResult<(BufReader<UnixStream>, Frame)> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match UnixStream::connect(socket) {
                Ok(stream) => {
                    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                    let mut reader = BufReader::new(stream);
                    if let Some(frame) = read_frame(&mut reader)? {
                        return Ok((reader, frame));
                    }
                }
                Err(error)
                    if Instant::now() < deadline
                        && matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                        ) => {}
                Err(error) => return Err(error.into()),
            }
            if Instant::now() >= deadline {
                return Err("server did not accept a presentation client".into());
            }
            thread::yield_now();
        }
    }

    fn wait_file(path: &Path) -> TestResult {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.exists() {
            if Instant::now() >= deadline {
                return Err(format!("timed out waiting for {}", path.display()).into());
            }
            thread::yield_now();
        }
        Ok(())
    }

    #[test]
    fn pending_frames_keep_the_initial_and_only_the_latest_revision() {
        let mut output = OutputQueue::default();
        assert!(output.push_initial_frame(vec![0]));
        assert!(output.push_frame(vec![1]));
        assert!(output.push_frame(vec![2]));

        assert_eq!(output.messages.len(), 2);
        assert!(output.messages.front().unwrap().bytes.ends_with(&[0]));
        assert!(output.messages.back().unwrap().bytes.ends_with(&[2]));
    }

    #[test]
    fn real_pty_reattach_converges_through_complete_ordered_frames() -> TestResult {
        let directory = TestDir::new()?;
        let socket = directory.0.join("orbit.sock");
        let ready = directory.0.join("ready");
        let enrich = directory.0.join("enrich");
        let release = directory.0.join("release");
        let finished = directory.0.join("finished");
        let stop = directory.0.join("stop");
        let primary = "P".repeat(80);
        let script = format!(
            "printf '{primary}'; : > '{ready}'; while [ ! -e '{enrich}' ] && [ ! -e '{stop}' ]; do sleep 0.01; done; [ -e '{stop}' ] && exit; printf '\\033[?1049h\\033[2J\\033[H'; printf '\\033]2;rich\\033\\\\\\033]7;file:///tmp/orbit\\033\\\\'; printf '\\033]8;;https://example.test\\033\\\\\\033[1;38;2;12;34;56mA\\033[0m\\033]8;;\\033\\\\'; printf 'e\\314\\201\\347\\225\\214'; printf '\\033[4;1H\\033[48;2;5;6;7m\\033[2K\\033[0m\\033[3;5H'; while [ ! -e '{release}' ] && [ ! -e '{stop}' ]; do sleep 0.01; done; [ -e '{stop}' ] && exit; i=0; while [ $i -lt 200 ]; do dd if=/dev/zero bs=8192 count=1 2>/dev/null; sleep 0.005; i=$((i + 1)); done; printf '\\033[?1049lX'; printf '\\033[3'; sleep 0.02; printf '2mS\\033[0m'; printf '\\303'; sleep 0.02; printf '\\251'; printf '\\033_Ga=q;'; sleep 0.02; printf '\\033\\\\'; printf '\\033]2;final\\033\\\\'; : > '{finished}'; while [ ! -e '{stop}' ]; do sleep 0.01; done",
            ready = ready.display(),
            enrich = enrich.display(),
            release = release.display(),
            finished = finished.display(),
            stop = stop.display(),
        );
        let server = Server::start(
            socket.clone(),
            vec!["/bin/sh".into(), "-c".into(), script],
            stop,
        );

        wait_file(&ready)?;
        let (mut first_reader, mut rich) = attach(&socket)?;
        fs::write(&enrich, b"enrich")?;
        while rich.title != "rich" {
            let next =
                read_frame(&mut first_reader)?.ok_or("client became busy after attachment")?;
            assert!(next.revision > rich.revision);
            rich = next;
        }
        assert_eq!((rich.cols, rich.rows, rich.screen), (80, 24, 1));
        assert_eq!(rich.title, "rich");
        assert_eq!(rich.pwd, "file:///tmp/orbit");
        assert_eq!((rich.hyperlinks, rich.graphics), (1, 0));
        assert_eq!(rich.cursor, Some((4, 2)));
        let linked = rich.cells.iter().find(|cell| cell.text == "A").unwrap();
        assert_eq!(linked.hyperlink, "https://example.test");
        assert_eq!(linked.foreground, (2, [12, 34, 56]));
        assert_ne!(linked.flags & 1, 0);
        assert!(rich.cells.iter().any(|cell| cell.text == "e\u{301}"));
        assert!(
            rich.cells
                .iter()
                .any(|cell| cell.text == "界" && cell.wide == 1)
        );
        assert!(
            rich.cells
                .iter()
                .any(|cell| cell.background == (2, [5, 6, 7]))
        );
        drop(first_reader);

        let (slow_client, _) = attach(&socket)?;
        fs::write(&release, b"go")?;
        wait_file(&finished)?;
        drop(slow_client);
        let (mut second_reader, mut final_frame) = attach(&socket)?;
        let initial_revision = final_frame.revision;
        while final_frame.title != "final" {
            let next =
                read_frame(&mut second_reader)?.ok_or("client became busy after attachment")?;
            assert!(next.revision > final_frame.revision);
            final_frame = next;
        }
        assert!(initial_revision >= rich.revision);
        assert_eq!(final_frame.screen, 0);
        assert_eq!(final_frame.title, "final");
        assert_eq!(final_frame.pwd, "file:///tmp/orbit");
        assert_eq!(final_frame.cells[0].text, "P");
        assert_eq!(final_frame.cells[79].text, "P");
        assert_eq!(final_frame.cells[80].text, "X");
        assert_ne!(final_frame.row_flags[0] & 1, 0);
        assert!(final_frame.cells.iter().any(|cell| cell.text == "S"));
        assert!(final_frame.cells.iter().any(|cell| cell.text == "é"));

        server.finish()
    }
}
