use crate::Result;
use libghostty_vt::{
    Error as GhosttyError, RenderState, Terminal,
    render::{CellIterator, CursorVisualStyle as GhosttyCursorShape, RowIterator},
    screen::{CellWide as GhosttyCellWidth, Screen},
    style::{RgbColor, Style, StyleColor as GhosttyStyleColor, Underline as GhosttyUnderline},
    terminal::{Point, PointCoordinate},
};
use orbit_protocol::{
    Capabilities, Cell as ProtocolCell, CellStyle, CellWidth, Colors, Cursor, CursorShape,
    CursorViewport, Dimensions, Frame, FrameSize, MAX_CELLS, MAX_FRAME_BYTES, Rgb, Row,
    Screen as ProtocolScreen, StyleColor, Underline, encode_frame,
};
use std::{collections::VecDeque, io, io::Write, os::unix::net::UnixStream};

const MAX_OUTPUT_BYTES: usize = 2 * (MAX_FRAME_BYTES + 32) + 4096;

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

        let colors = snapshot.colors()?;
        let screen = match terminal.active_screen()? {
            Screen::Primary => ProtocolScreen::Primary,
            Screen::Alternate => ProtocolScreen::Alternate,
        };
        let cursor = Cursor {
            visible: snapshot.cursor_visible()?,
            blinking: snapshot.cursor_blinking()?,
            password_input: snapshot.cursor_password_input()?,
            shape: cursor_shape(snapshot.cursor_visual_style()?)?,
            viewport: snapshot.cursor_viewport()?.map(|cursor| CursorViewport {
                x: cursor.x,
                y: cursor.y,
                at_wide_tail: cursor.at_wide_tail,
            }),
        };
        let title = terminal.title()?;
        let working_directory = terminal.pwd()?;
        let mut frame_size = FrameSize::new(
            title,
            working_directory,
            colors.cursor.is_some(),
            cursor.viewport.is_some(),
        )?;
        let title = title.to_owned();
        let working_directory = working_directory.to_owned();
        let mut row_iter = rows.update(&snapshot)?;
        let mut graphemes = String::new();
        let mut frame_rows = Vec::with_capacity(usize::from(row_count));
        for y in 0..row_count {
            frame_size.add_row()?;
            let row = row_iter.next().ok_or("render snapshot omitted a row")?;
            let raw_row = row.raw_row()?;
            let mut cell_iter = cells.update(row)?;
            let mut frame_cells = Vec::with_capacity(usize::from(cols));
            for x in 0..cols {
                let cell = cell_iter.next().ok_or("render snapshot omitted a cell")?;
                let raw_cell = cell.raw_cell()?;
                let mut style = cell.style()?;
                if let Some(background) = cell.bg_color()? {
                    style.bg_color = GhosttyStyleColor::Rgb(background);
                }

                let grapheme_count = cell.graphemes_len()?;
                if grapheme_count > MAX_FRAME_BYTES / size_of::<char>() {
                    return Err("cell grapheme exceeds the frame bound".into());
                }
                graphemes.clear();
                cell.graphemes_utf8(&mut graphemes)?;

                let hyperlink = if raw_cell.has_hyperlink()? {
                    let reference = terminal
                        .grid_ref(Point::Viewport(PointCoordinate { x, y: u32::from(y) }))?;
                    hyperlink(&reference)?
                } else {
                    String::new()
                };
                frame_size.add_cell(&graphemes, &hyperlink)?;
                frame_cells.push(ProtocolCell {
                    width: cell_width(raw_cell.wide()?)?,
                    style: cell_style(style, cell.is_selected()?, raw_cell.is_protected()?)?,
                    text: graphemes.clone(),
                    hyperlink,
                });
            }
            if cell_iter.next().is_some() {
                return Err("render snapshot added an unexpected cell".into());
            }
            frame_rows.push(Row {
                wrapped: raw_row.is_wrapped()?,
                wrap_continuation: raw_row.is_wrap_continuation()?,
                kitty_virtual_placeholder: raw_row.has_kitty_virtual_placeholder()?,
                cells: frame_cells,
            });
        }
        if row_iter.next().is_some() {
            return Err("render snapshot added an unexpected row".into());
        }
        let frame = Frame {
            revision,
            dimensions: Dimensions {
                cols,
                rows: row_count,
            },
            screen,
            title,
            working_directory,
            capabilities: Capabilities {
                hyperlinks: true,
                kitty_graphics: false,
            },
            colors: Colors {
                background: rgb(colors.background),
                foreground: rgb(colors.foreground),
                cursor: colors.cursor.map(rgb),
                palette: colors.palette.map(rgb),
            },
            cursor,
            rows: frame_rows,
        };
        Ok(encode_frame(&frame)?)
    }
}

fn hyperlink(reference: &libghostty_vt::screen::GridRef<'_>) -> Result<String> {
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
    Ok(String::from_utf8(bytes)?)
}

fn rgb(color: RgbColor) -> Rgb {
    Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

fn cursor_shape(shape: GhosttyCursorShape) -> Result<CursorShape> {
    match shape {
        GhosttyCursorShape::Bar => Ok(CursorShape::Bar),
        GhosttyCursorShape::Block => Ok(CursorShape::Block),
        GhosttyCursorShape::Underline => Ok(CursorShape::Underline),
        GhosttyCursorShape::BlockHollow => Ok(CursorShape::BlockHollow),
        _ => Err("libghostty returned an unsupported cursor shape".into()),
    }
}

fn cell_width(width: GhosttyCellWidth) -> Result<CellWidth> {
    match width {
        GhosttyCellWidth::Narrow => Ok(CellWidth::Narrow),
        GhosttyCellWidth::Wide => Ok(CellWidth::Wide),
        GhosttyCellWidth::SpacerTail => Ok(CellWidth::SpacerTail),
        GhosttyCellWidth::SpacerHead => Ok(CellWidth::SpacerHead),
    }
}

fn style_color(color: GhosttyStyleColor) -> StyleColor {
    match color {
        GhosttyStyleColor::None => StyleColor::None,
        GhosttyStyleColor::Palette(index) => StyleColor::Palette(index.0),
        GhosttyStyleColor::Rgb(color) => StyleColor::Rgb(rgb(color)),
    }
}

fn underline(underline: GhosttyUnderline) -> Result<Underline> {
    match underline {
        GhosttyUnderline::None => Ok(Underline::None),
        GhosttyUnderline::Single => Ok(Underline::Single),
        GhosttyUnderline::Double => Ok(Underline::Double),
        GhosttyUnderline::Curly => Ok(Underline::Curly),
        GhosttyUnderline::Dotted => Ok(Underline::Dotted),
        GhosttyUnderline::Dashed => Ok(Underline::Dashed),
        _ => Err("libghostty returned an unsupported underline style".into()),
    }
}

fn cell_style(style: Style, selected: bool, protected: bool) -> Result<CellStyle> {
    Ok(CellStyle {
        foreground: style_color(style.fg_color),
        background: style_color(style.bg_color),
        underline_color: style_color(style.underline_color),
        bold: style.bold,
        italic: style.italic,
        faint: style.faint,
        blink: style.blink,
        inverse: style.inverse,
        invisible: style.invisible,
        strikethrough: style.strikethrough,
        overline: style.overline,
        selected,
        protected,
        underline: underline(style.underline)?,
    })
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
        Ok(Some(orbit_protocol::decode_frame(&payload)?))
    }

    fn cell(frame: &Frame, x: usize, y: usize) -> &ProtocolCell {
        &frame.rows[y].cells[x]
    }

    fn cells(frame: &Frame) -> impl Iterator<Item = &ProtocolCell> {
        frame.rows.iter().flat_map(|row| &row.cells)
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
        assert_eq!(
            (rich.dimensions.cols, rich.dimensions.rows, rich.screen),
            (80, 24, ProtocolScreen::Alternate)
        );
        assert_eq!(rich.title, "rich");
        assert_eq!(rich.working_directory, "file:///tmp/orbit");
        assert!(rich.capabilities.hyperlinks);
        assert!(!rich.capabilities.kitty_graphics);
        assert_eq!(
            rich.cursor.viewport.map(|cursor| (cursor.x, cursor.y)),
            Some((4, 2))
        );
        let linked = cells(&rich).find(|cell| cell.text == "A").unwrap();
        assert_eq!(linked.hyperlink, "https://example.test");
        assert_eq!(
            linked.style.foreground,
            StyleColor::Rgb(Rgb {
                r: 12,
                g: 34,
                b: 56
            })
        );
        assert!(linked.style.bold);
        assert!(cells(&rich).any(|cell| cell.text == "e\u{301}"));
        assert!(cells(&rich).any(|cell| cell.text == "界" && cell.width == CellWidth::Wide));
        assert!(
            cells(&rich)
                .any(|cell| { cell.style.background == StyleColor::Rgb(Rgb { r: 5, g: 6, b: 7 }) })
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
        assert_eq!(final_frame.screen, ProtocolScreen::Primary);
        assert_eq!(final_frame.title, "final");
        assert_eq!(final_frame.working_directory, "file:///tmp/orbit");
        assert_eq!(cell(&final_frame, 0, 0).text, "P");
        assert_eq!(cell(&final_frame, 79, 0).text, "P");
        assert_eq!(cell(&final_frame, 0, 1).text, "X");
        assert!(final_frame.rows[0].wrapped);
        assert!(cells(&final_frame).any(|cell| cell.text == "S"));
        assert!(cells(&final_frame).any(|cell| cell.text == "é"));

        server.finish()
    }
}
