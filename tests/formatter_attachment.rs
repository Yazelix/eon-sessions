use libghostty_vt::{
    Terminal, TerminalOptions,
    fmt::{Format, Formatter, FormatterOptions},
    key::{Action, Encoder as KeyEncoder, Event as KeyEvent, Key},
    screen::Screen,
    style::PaletteIndex,
    terminal::{Mode, Point, PointCoordinate},
};
use std::{cell::RefCell, rc::Rc};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn terminal() -> TestResult<Terminal<'static, 'static>> {
    let mut terminal = Terminal::new(TerminalOptions {
        cols: 12,
        rows: 4,
        max_scrollback: 32,
    })?;
    terminal.resize(12, 4, 8, 16)?;
    Ok(terminal)
}

fn format_terminal(terminal: &Terminal, format: Format, extras: bool) -> TestResult<Vec<u8>> {
    format_terminal_with_tabstops(terminal, format, extras, extras)
}

fn format_terminal_with_tabstops(
    terminal: &Terminal,
    format: Format,
    extras: bool,
    tabstops: bool,
) -> TestResult<Vec<u8>> {
    let options = FormatterOptions::new()
        .with_format(format)
        .with_palette(extras)
        .with_modes(extras)
        .with_scrolling_region(extras)
        .with_tabstops(tabstops)
        .with_pwd(extras)
        .with_keyboard(extras)
        .with_cursor(extras)
        .with_style(extras)
        .with_hyperlink(extras)
        .with_protection(extras)
        .with_kitty_keyboard(extras)
        .with_charsets(extras);
    let mut formatter = Formatter::new(terminal, options)?;
    Ok(formatter.format_alloc(None)?.to_vec())
}

fn bootstrap(terminal: &Terminal) -> TestResult<Vec<u8>> {
    format_terminal(terminal, Format::Vt, true)
}

fn bootstrap_without_tabstops(terminal: &Terminal) -> TestResult<Vec<u8>> {
    format_terminal_with_tabstops(terminal, Format::Vt, true, false)
}

fn semantic_view(terminal: &Terminal) -> TestResult<(Screen, u16, u16, bool, Vec<u8>)> {
    Ok((
        terminal.active_screen()?,
        terminal.cursor_x()?,
        terminal.cursor_y()?,
        terminal.is_cursor_pending_wrap()?,
        format_terminal(terminal, Format::Plain, false)?,
    ))
}

fn hyperlink_at(terminal: &Terminal, x: u16, y: u32) -> TestResult<Vec<u8>> {
    let cell = terminal.grid_ref(Point::Viewport(PointCoordinate { x, y }))?;
    let mut uri = vec![0; 128];
    let len = cell.hyperlink_uri(&mut uri)?;
    uri.truncate(len);
    Ok(uri)
}

fn encoded_up(terminal: &Terminal) -> TestResult<Vec<u8>> {
    let mut event = KeyEvent::new()?;
    event
        .set_action(Action::Press)
        .set_key(Key::ArrowUp)
        .set_utf8::<String>(None);
    let mut encoder = KeyEncoder::new()?;
    encoder.set_options_from_terminal(terminal);
    let mut bytes = Vec::new();
    encoder.encode_to_vec(&event, &mut bytes)?;
    Ok(bytes)
}

#[test]
fn formatter_loses_inactive_primary_screen() -> TestResult {
    let mut authoritative = terminal()?;
    authoritative.vt_write(b"primary");
    authoritative.vt_write(b"\x1b[?1049h");
    authoritative.vt_write(b"alternate");

    let mut reconstructed = terminal()?;
    reconstructed.vt_write(&bootstrap(&authoritative)?);
    assert_eq!(reconstructed.active_screen()?, Screen::Alternate);

    let future_tail = b"\x1b[?1049l";
    authoritative.vt_write(future_tail);
    reconstructed.vt_write(future_tail);

    assert_eq!(
        format_terminal(&authoritative, Format::Plain, false)?,
        b"primary"
    );
    assert!(format_terminal(&reconstructed, Format::Plain, false)?.is_empty());
    Ok(())
}

#[test]
fn formatter_preserves_modes_palette_and_input_but_loses_cursor_and_cell_link() -> TestResult {
    let mut authoritative = terminal()?;
    authoritative.vt_write(b"\x1b[?1h");
    authoritative.vt_write(b"\x1b[=3;1u");
    authoritative.vt_write(b"\x1b[3g\x1b[1;5H\x1bH\x1b[1;1H");
    authoritative.vt_write(b"\x1b]4;1;rgb:12/34/56\x1b\\");
    authoritative.vt_write(b"\x1b]7;file:///tmp/orbit\x1b\\");
    authoritative.vt_write(b"\x1b]8;;https://example.test\x1b\\H\x1b]8;;\x1b\\");

    let mut reconstructed = terminal()?;
    reconstructed.vt_write(&bootstrap(&authoritative)?);

    assert_eq!(
        format_terminal(&authoritative, Format::Plain, false)?,
        format_terminal(&reconstructed, Format::Plain, false)?
    );
    assert_eq!(authoritative.cursor_x()?, 1);
    assert_eq!(reconstructed.cursor_x()?, 4);
    assert!(reconstructed.mode(Mode::DECCKM)?);
    assert_eq!(
        authoritative.kitty_keyboard_flags()?,
        reconstructed.kitty_keyboard_flags()?
    );
    assert_eq!(
        authoritative.color_palette()?.get(PaletteIndex::RED),
        reconstructed.color_palette()?.get(PaletteIndex::RED)
    );
    assert_eq!(reconstructed.pwd()?, "file:///tmp/orbit");
    assert_eq!(hyperlink_at(&authoritative, 0, 0)?, b"https://example.test");
    assert!(hyperlink_at(&reconstructed, 0, 0)?.is_empty());
    assert_eq!(encoded_up(&authoritative)?, encoded_up(&reconstructed)?);

    let future_tab = b"\x1b[1;1H\tX";
    authoritative.vt_write(future_tab);
    reconstructed.vt_write(future_tab);
    assert_eq!(
        semantic_view(&authoritative)?,
        semantic_view(&reconstructed)?
    );
    Ok(())
}

#[test]
fn formatter_preserves_future_charset_and_active_link_without_tabstop_side_effect() -> TestResult {
    let mut authoritative = terminal()?;
    authoritative.vt_write(b"\x1b(0");
    authoritative.vt_write(b"\x1b]8;;https://example.test\x1b\\");

    let mut reconstructed = terminal()?;
    reconstructed.vt_write(&bootstrap_without_tabstops(&authoritative)?);

    authoritative.vt_write(b"q");
    reconstructed.vt_write(b"q");
    assert_eq!(
        semantic_view(&authoritative)?,
        semantic_view(&reconstructed)?
    );
    assert_eq!(
        hyperlink_at(&authoritative, 0, 0)?,
        hyperlink_at(&reconstructed, 0, 0)?
    );
    Ok(())
}

#[test]
fn formatter_loses_pending_wrap_before_future_text() -> TestResult {
    let mut authoritative = terminal()?;
    authoritative.vt_write(b"abcdefghijkl");
    assert!(authoritative.is_cursor_pending_wrap()?);

    let mut reconstructed = terminal()?;
    reconstructed.vt_write(&bootstrap(&authoritative)?);
    assert!(!reconstructed.is_cursor_pending_wrap()?);

    authoritative.vt_write(b"Z");
    reconstructed.vt_write(b"Z");
    assert_ne!(
        semantic_view(&authoritative)?,
        semantic_view(&reconstructed)?
    );
    Ok(())
}

#[test]
fn formatter_loses_split_parser_state() -> TestResult {
    let fire = "🔥".as_bytes();
    let cases: &[(&str, &[u8], &[u8])] = &[
        ("CSI", b"\x1b", b"[31mX\x1b[0m"),
        ("UTF-8", &fire[..2], &fire[2..]),
        ("APC", b"\x1b_", b"Ga=d\x1b\\X"),
    ];

    for (name, prefix, future_tail) in cases {
        let mut authoritative = terminal()?;
        authoritative.vt_write(prefix);
        let mut reconstructed = terminal()?;
        reconstructed.vt_write(&bootstrap(&authoritative)?);

        authoritative.vt_write(future_tail);
        reconstructed.vt_write(future_tail);
        assert_ne!(
            semantic_view(&authoritative)?,
            semantic_view(&reconstructed)?,
            "{name} continuation unexpectedly converged"
        );
    }
    Ok(())
}

#[test]
fn formatter_does_not_transfer_title_or_kitty_images() -> TestResult {
    let mut authoritative = terminal()?;
    authoritative.set_kitty_image_storage_limit(1024 * 1024)?;
    authoritative.vt_write(b"\x1b]2;orbit-title\x1b\\");
    authoritative.vt_write(b"\x1b_Ga=T,f=32,s=1,v=1,i=7,q=2;/wAA/w==\x1b\\");
    assert!(authoritative.kitty_graphics()?.generation()? > 0);

    let mut reconstructed = terminal()?;
    reconstructed.set_kitty_image_storage_limit(1024 * 1024)?;
    reconstructed.vt_write(&bootstrap(&authoritative)?);

    assert_eq!(authoritative.title()?, "orbit-title");
    assert!(reconstructed.title()?.is_empty());
    assert_eq!(reconstructed.kitty_graphics()?.generation()?, 0);
    Ok(())
}

#[test]
fn only_authoritative_terminal_emits_pty_responses() -> TestResult {
    let responses = Rc::new(RefCell::new(Vec::<Vec<u8>>::new()));
    let response_sink = Rc::clone(&responses);
    let mut authoritative = terminal()?;
    authoritative.on_pty_write(move |_, bytes| {
        response_sink.borrow_mut().push(bytes.to_vec());
    })?;

    let mut reconstructed = terminal()?;
    reconstructed.vt_write(&bootstrap(&authoritative)?);

    let query = b"\x1b[6n";
    authoritative.vt_write(query);
    reconstructed.vt_write(query);
    assert_eq!(responses.borrow().len(), 1);
    Ok(())
}

#[test]
fn formatter_changes_scrollback_count_but_sample_reflow_converges() -> TestResult {
    let mut authoritative = terminal()?;
    for line in 0..8 {
        authoritative.vt_write(format!("line-{line}\r\n").as_bytes());
    }

    let mut reconstructed = terminal()?;
    reconstructed.vt_write(&bootstrap(&authoritative)?);
    assert_eq!(authoritative.scrollback_rows()?, 5);
    assert_eq!(reconstructed.scrollback_rows()?, 4);

    authoritative.resize(8, 4, 8, 16)?;
    reconstructed.resize(8, 4, 8, 16)?;
    authoritative.vt_write(b"tail");
    reconstructed.vt_write(b"tail");
    assert_eq!(
        semantic_view(&authoritative)?,
        semantic_view(&reconstructed)?
    );
    Ok(())
}
