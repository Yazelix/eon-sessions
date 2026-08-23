use crate::{
    Result,
    attachment::Client,
    platform::Pty,
    runtime::{Presentation, next_revision, queue_pty_write},
};
use libghostty_vt::{
    Terminal,
    fmt::Format,
    focus,
    key::{
        Action as GhosttyKeyAction, Encoder as KeyEncoder, Event as GhosttyKeyEvent, Key,
        Mods as GhosttyModifiers,
    },
    mouse::{
        Action as GhosttyMouseAction, Button as GhosttyMouseButton, Encoder as MouseEncoder,
        EncoderSize as MouseEncoderSize, Event as GhosttyMouseEvent, Position as MousePosition,
    },
    paste,
    screen::Screen,
    selection::{FormatOptions, Selection},
    terminal::{Mode, Point, PointCoordinate, ScrollViewport},
};
use orbit_protocol::session::{
    self, ClientMessage, FailureCode, FocusEvent, KeyAction, KeyEvent, Modifiers, MouseAction,
    MouseButton, PhysicalKey, PreviewOutcome, SelectionAction, ServerMessage, SurfaceSize,
    VerticalDirection, VerticalPreview, ViewportCell, WheelOutcome,
};
use std::{cell::RefCell, collections::VecDeque};

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct SelectionState {
    anchor: Option<ViewportCell>,
    copied: Option<String>,
    visible: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_client_message(
    client: &mut Client,
    message: ClientMessage,
    terminal: &mut Terminal<'static, '_>,
    pty: Option<&Pty>,
    size: &mut SurfaceSize,
    writes: &RefCell<VecDeque<u8>>,
    presentation: &mut Presentation,
    selection: &mut SelectionState,
) -> Result<bool> {
    let Some(pty) = pty else {
        return client.fail(FailureCode::Terminal, "PTY is closed".into());
    };
    if let ClientMessage::PreviewVertical {
        frame_revision,
        direction,
    } = message
    {
        return handle_vertical_preview(client, frame_revision, direction, terminal, presentation);
    }
    if let ClientMessage::Selection(action) = message {
        return handle_selection(client, action, terminal, *size, presentation, selection);
    }
    let message = match message {
        ClientMessage::Resize(surface) => {
            clear_selection(terminal, selection, false)?;
            pty.resize(surface)?;
            terminal.resize(
                surface.cols,
                surface.rows,
                surface.cell_width,
                surface.cell_height,
            )?;
            *size = surface;
            presentation.advance()?;
            if !client.push_message(&ServerMessage::Accepted)? {
                return Ok(false);
            }
            return presentation.publish(Some(client), terminal);
        }
        message => message,
    };
    let return_live = matches!(&message, ClientMessage::Key(_));
    if let ClientMessage::Mouse(input) = &message
        && let Some(direction) = vertical_wheel_direction(input.button)
    {
        return handle_vertical_wheel(
            client,
            *input,
            direction,
            terminal,
            *size,
            writes,
            presentation,
            selection,
        );
    }
    let encoded = match encode_input(terminal, *size, message) {
        Ok(encoded) => encoded,
        Err(error) => {
            return client.fail(FailureCode::Terminal, error.to_string());
        }
    };
    if !queue_pty_write(&mut writes.borrow_mut(), &encoded) {
        let queued = client.fail(FailureCode::Terminal, "PTY input queue is full".into())?;
        client.close_when_flushed();
        return Ok(queued);
    }
    let returned_live =
        if return_live && !encoded.is_empty() && terminal.active_screen()? == Screen::Primary {
            let scrollbar = terminal.scrollbar()?;
            if scrollbar.offset.saturating_add(scrollbar.len) < scrollbar.total {
                clear_selection(terminal, selection, false)?;
                terminal.scroll_viewport(ScrollViewport::Bottom);
                presentation.advance()?;
                true
            } else {
                false
            }
        } else {
            false
        };
    if !client.push_message(&ServerMessage::Accepted)? {
        return Ok(false);
    }
    if returned_live {
        return presentation.publish(Some(client), terminal);
    }
    Ok(true)
}

fn vertical_wheel_direction(button: Option<MouseButton>) -> Option<VerticalDirection> {
    match button {
        Some(MouseButton::Four) => Some(VerticalDirection::Up),
        Some(MouseButton::Five) => Some(VerticalDirection::Down),
        _ => None,
    }
}

fn routes_vertical_wheel_to_terminal(terminal: &Terminal<'_, '_>) -> Result<bool> {
    Ok(terminal.is_mouse_tracking()?
        || terminal.active_screen()? == Screen::Alternate && terminal.mode(Mode::ALT_SCROLL)?)
}

fn handle_vertical_preview(
    client: &mut Client,
    frame_revision: u64,
    direction: VerticalDirection,
    terminal: &Terminal<'static, '_>,
    presentation: &Presentation,
) -> Result<bool> {
    if frame_revision != presentation.revision {
        return client.fail(
            FailureCode::InvalidInput,
            "preview frame revision is stale".into(),
        );
    }
    let outcome = if routes_vertical_wheel_to_terminal(terminal)? {
        PreviewOutcome::TerminalRouted
    } else {
        if terminal.mode(Mode::SYNC_OUTPUT)? {
            return client.fail(FailureCode::Terminal, "presentation is synchronized".into());
        }
        let scrollbar = terminal.scrollbar()?;
        let target = match direction {
            VerticalDirection::Up => scrollbar.offset.checked_sub(1),
            VerticalDirection::Down => {
                let below = scrollbar.offset.saturating_add(scrollbar.len);
                (below < scrollbar.total).then_some(below)
            }
        };
        let cols = terminal.cols()?;
        let row = match target {
            Some(y) => match u32::try_from(y)
                .map_err(|_| "preview row exceeds the terminal coordinate range".into())
                .and_then(|y| presentation.extractor.row(terminal, y, cols))
            {
                Ok(row) => Some(row),
                Err(error) => {
                    return client.fail(FailureCode::Terminal, error.to_string());
                }
            },
            None => None,
        };
        PreviewOutcome::Viewport {
            cols,
            edge_reached: row.is_none(),
            row,
        }
    };
    client.push_message(&ServerMessage::VerticalPreview(VerticalPreview {
        frame_revision,
        direction,
        outcome,
    }))
}

#[allow(clippy::too_many_arguments)]
fn handle_vertical_wheel(
    client: &mut Client,
    input: session::MouseEvent,
    direction: VerticalDirection,
    terminal: &mut Terminal<'static, '_>,
    size: SurfaceSize,
    writes: &RefCell<VecDeque<u8>>,
    presentation: &mut Presentation,
    selection: &mut SelectionState,
) -> Result<bool> {
    if routes_vertical_wheel_to_terminal(terminal)? {
        let routed = if terminal.is_mouse_tracking()? {
            ClientMessage::Mouse(input)
        } else {
            ClientMessage::Key(KeyEvent {
                action: KeyAction::Press,
                key: match direction {
                    VerticalDirection::Up => PhysicalKey::ARROW_UP,
                    VerticalDirection::Down => PhysicalKey::ARROW_DOWN,
                },
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: None,
                unshifted_codepoint: None,
            })
        };
        let encoded = match encode_input(terminal, size, routed) {
            Ok(encoded) if !encoded.is_empty() => encoded,
            Ok(_) => {
                return client.fail(
                    FailureCode::InvalidInput,
                    "wheel is outside the terminal grid".into(),
                );
            }
            Err(error) => {
                return client.fail(FailureCode::Terminal, error.to_string());
            }
        };
        let outcome = ServerMessage::WheelOutcome(WheelOutcome::TerminalRouted);
        if !client.can_push_message(&outcome)? {
            return client.fail(
                FailureCode::Terminal,
                "client output queue cannot admit wheel outcome".into(),
            );
        }
        if !queue_pty_write(&mut writes.borrow_mut(), &encoded) {
            let queued = client.fail(FailureCode::Terminal, "PTY input queue is full".into())?;
            client.close_when_flushed();
            return Ok(queued);
        }
        if !client.push_message(&outcome)? {
            return Err("wheel outcome admission contradicted its preflight".into());
        }
        return Ok(true);
    }

    if terminal.mode(Mode::SYNC_OUTPUT)? {
        return client.fail(FailureCode::Terminal, "presentation is synchronized".into());
    }
    if !client.can_push_frame_message() {
        return client.fail(
            FailureCode::Terminal,
            "client output queue cannot admit wheel outcome".into(),
        );
    }
    let next = match next_revision(presentation.revision) {
        Ok(next) => next,
        Err(error) => return client.fail(FailureCode::Terminal, error.to_string()),
    };
    let before = terminal.scrollbar()?.offset;
    clear_selection(terminal, selection, false)?;
    terminal.scroll_viewport(ScrollViewport::Delta(match direction {
        VerticalDirection::Up => -1,
        VerticalDirection::Down => 1,
    }));
    let after = terminal.scrollbar()?.offset;
    let applied_rows = i8::try_from(i128::from(after) - i128::from(before))
        .map_err(|_| "terminal applied an invalid wheel delta")?;
    if !matches!(applied_rows, -1..=1) {
        return Err("terminal applied more than one wheel row".into());
    }
    presentation.revision = next;
    presentation.synchronized_until = None;
    let frame = presentation.extractor.frame(next, terminal)?;
    if !client.push_message(&ServerMessage::WheelOutcome(WheelOutcome::Viewport {
        applied_rows,
        frame: Box::new(frame),
    }))? {
        return Err("wheel outcome admission contradicted its preflight".into());
    }
    Ok(true)
}

fn handle_selection(
    client: &mut Client,
    action: SelectionAction,
    terminal: &Terminal<'static, '_>,
    size: SurfaceSize,
    presentation: &mut Presentation,
    state: &mut SelectionState,
) -> Result<bool> {
    let (anchor, cell, finish) = match action {
        SelectionAction::Begin {
            frame_revision,
            cell,
        } => {
            if frame_revision != presentation.revision {
                return client.fail(
                    FailureCode::InvalidInput,
                    "selection frame revision is stale".into(),
                );
            }
            (cell, cell, false)
        }
        SelectionAction::Update { cell } => {
            let Some(anchor) = state.anchor else {
                return client.fail(
                    FailureCode::InvalidInput,
                    "selection update has no active selection".into(),
                );
            };
            (anchor, cell, false)
        }
        SelectionAction::Finish { cell } => {
            let Some(anchor) = state.anchor else {
                return client.fail(
                    FailureCode::InvalidInput,
                    "selection finish has no active selection".into(),
                );
            };
            (anchor, cell, true)
        }
        SelectionAction::Copy => {
            return match &state.copied {
                Some(text) => client.push_message(&ServerMessage::CopiedText(text.clone())),
                None => client.fail(
                    FailureCode::InvalidInput,
                    "no finished selection to copy".into(),
                ),
            };
        }
    };
    if cell.x >= size.cols || cell.y >= size.rows {
        return client.fail(
            FailureCode::InvalidInput,
            "selection cell is outside the current viewport".into(),
        );
    }
    if !client.can_push_result_frame() {
        return client.fail(
            FailureCode::Terminal,
            "client output queue cannot admit selection frame".into(),
        );
    }
    let next = match next_revision(presentation.revision) {
        Ok(next) => next,
        Err(error) => return client.fail(FailureCode::Terminal, error.to_string()),
    };

    let selected = match viewport_selection(terminal, anchor, cell) {
        Ok(selected) => selected,
        Err(error) => return client.fail(FailureCode::Terminal, error.to_string()),
    };
    let copied = if finish {
        match format_selection(terminal, &selected) {
            Ok(text) => Some(text),
            Err(error) => {
                return client.fail(FailureCode::Terminal, error.to_string());
            }
        }
    } else {
        None
    };
    if let Err(error) = terminal.set_selection(Some(&selected)) {
        return client.fail(FailureCode::Terminal, error.to_string());
    }

    state.anchor = (!finish).then_some(anchor);
    if finish || matches!(action, SelectionAction::Begin { .. }) {
        state.copied = copied;
    }
    state.visible = true;
    presentation.revision = next;
    if !client.push_message(&ServerMessage::Accepted)? {
        return Ok(false);
    }
    presentation.publish(Some(client), terminal)
}

fn viewport_selection<'terminal>(
    terminal: &'terminal Terminal<'_, '_>,
    start: ViewportCell,
    end: ViewportCell,
) -> Result<Selection<'terminal>> {
    let point = |cell: ViewportCell| {
        terminal.grid_ref(Point::Viewport(PointCoordinate {
            x: cell.x,
            y: u32::from(cell.y),
        }))
    };
    Ok(Selection::new(point(start)?, point(end)?, false))
}

fn format_selection(terminal: &Terminal<'_, '_>, selected: &Selection<'_>) -> Result<String> {
    let options = || {
        FormatOptions::new()
            .with_emit_format(Format::Plain)
            .with_unwrap(true)
            .with_trim(true)
            .with_selection(selected)
    };
    let required = match terminal.format_selection_buf(options(), &mut []) {
        Ok(Some(written)) => written,
        Ok(None) => return Err("terminal returned no selection text".into()),
        Err(libghostty_vt::error::Error::OutOfSpace { required }) => required,
        Err(error) => return Err(error.into()),
    };
    if required > session::MAX_COPY_BYTES {
        return Err("selection text exceeds the copy limit".into());
    }
    let mut bytes = vec![0; required];
    let Some(written) = terminal.format_selection_buf(options(), &mut bytes)? else {
        return Err("terminal returned no selection text".into());
    };
    bytes.truncate(written);
    Ok(String::from_utf8(bytes)?)
}

pub(crate) fn clear_selection(
    terminal: &Terminal<'_, '_>,
    state: &mut SelectionState,
    forget_copy: bool,
) -> Result<bool> {
    let changed = state.visible;
    if changed {
        terminal.set_selection(None)?;
    }
    state.anchor = None;
    state.visible = false;
    if forget_copy {
        state.copied = None;
    }
    Ok(changed)
}

pub(crate) fn apply_pty_output(
    terminal: &mut Terminal<'_, '_>,
    state: &mut SelectionState,
    bytes: &[u8],
) -> Result {
    clear_selection(terminal, state, false)?;
    terminal.vt_write(bytes);
    Ok(())
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
            let unshifted_codepoint = input
                .unshifted_codepoint
                .or((input.key == PhysicalKey::SPACE).then_some(' '));
            let text = input
                .text
                .filter(|_| input.action != KeyAction::Release || unshifted_codepoint.is_some());
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
                .set_utf8(text);
            if let Some(codepoint) = unshifted_codepoint {
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
        ClientMessage::Hello
        | ClientMessage::ObserveMetadata
        | ClientMessage::Resize(_)
        | ClientMessage::Selection(_)
        | ClientMessage::PreviewVertical { .. } => Err("message is not terminal input".into()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        presentation::Extractor,
        runtime::{
            INITIAL_SIZE, MAX_PTY_WRITE_BYTES,
            tests::{
                attached_client, fill_output, flush_message, terminal, terminal_with_scrollback,
                wheel,
            },
        },
    };
    use orbit_protocol::session::Failure;

    #[test]
    fn authoritative_selection_rejects_stale_input_and_freezes_copy() -> Result {
        let mut text = terminal_with_scrollback(100)?;
        text.vt_write(b"A\r\n\r\nZ\r\nB\x1b[3;1H\x1b[X");
        let cell = |(x, y)| ViewportCell { x, y };
        for (start, end, expected) in [
            ((0, 0), (0, 0), "A"),
            ((0, 0), (0, 3), "A\n\n\nB"),
            ((0, 3), (0, 0), "A\n\n\nB"),
            ((0, 1), (0, 1), ""),
            ((0, 2), (0, 2), ""),
        ] {
            let selected = viewport_selection(&text, cell(start), cell(end))?;
            assert_eq!(format_selection(&text, &selected)?, expected);
        }

        for line in 0..30 {
            text.vt_write(format!("history-{line:02}\r\n").as_bytes());
        }
        text.scroll_viewport(ScrollViewport::Top);
        let history = text.scrollbar()?;
        assert_eq!(history.offset, 0);
        assert!(history.offset + history.len < history.total);
        let retained = viewport_selection(&text, cell((0, 0)), cell((0, 0)))?;
        assert_eq!(format_selection(&text, &retained)?, "A");

        let (mut client, mut peer) = attached_client()?;
        let pty = Pty::spawn(&["/bin/sh".into()], INITIAL_SIZE)?;
        let mut terminal = terminal()?;
        terminal.vt_write("alpha 界\r\n".as_bytes());
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut presentation = Presentation::new()?;
        presentation.revision = 1;
        let mut selection = SelectionState::default();

        macro_rules! reject {
            ($action:expr) => {{
                let unchanged = presentation.revision;
                assert!(handle_client_message(
                    &mut client,
                    ClientMessage::Selection($action),
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut presentation,
                    &mut selection,
                )?);
                assert_eq!(presentation.revision, unchanged);
                assert_eq!(selection, SelectionState::default());
                assert!(matches!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::Failure(Failure {
                        code: FailureCode::InvalidInput,
                        ..
                    })
                ));
            }};
        }
        macro_rules! select {
            ($action:expr) => {{
                assert!(handle_client_message(
                    &mut client,
                    ClientMessage::Selection($action),
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut presentation,
                    &mut selection,
                )?);
                assert_eq!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::Accepted
                );
                let ServerMessage::Frame(frame) = flush_message(&mut client, &mut peer)? else {
                    return Err("selection did not publish a frame".into());
                };
                assert_eq!(frame.revision, presentation.revision);
                assert!(frame.rows[0].cells[0].style.selected);
            }};
        }
        reject!(session::SelectionAction::Begin {
            frame_revision: 0,
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        reject!(session::SelectionAction::Update {
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        reject!(session::SelectionAction::Begin {
            frame_revision: presentation.revision,
            cell: session::ViewportCell { x: size.cols, y: 0 },
        });

        select!(session::SelectionAction::Begin {
            frame_revision: presentation.revision,
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        apply_pty_output(&mut terminal, &mut selection, b"!")?;
        assert_eq!(selection, SelectionState::default());
        reject!(session::SelectionAction::Finish {
            cell: session::ViewportCell { x: 6, y: 0 },
        });

        select!(session::SelectionAction::Begin {
            frame_revision: presentation.revision,
            cell: session::ViewportCell { x: 0, y: 0 },
        });
        select!(session::SelectionAction::Update {
            cell: session::ViewportCell { x: 6, y: 0 },
        });
        select!(session::SelectionAction::Finish {
            cell: session::ViewportCell { x: 6, y: 0 },
        });

        assert!(handle_client_message(
            &mut client,
            ClientMessage::Selection(session::SelectionAction::Copy),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(
            flush_message(&mut client, &mut peer)?,
            ServerMessage::CopiedText("alpha 界".into())
        );

        apply_pty_output(&mut terminal, &mut selection, b"later")?;
        assert!(
            !Extractor::new()?
                .frame(presentation.revision + 1, &terminal)?
                .rows[0]
                .cells
                .iter()
                .any(|cell| cell.style.selected)
        );
        assert_eq!(selection.copied.as_deref(), Some("alpha 界"));

        let (mut blocked, _) = attached_client()?;
        fill_output(&mut blocked)?;
        let stable_revision = presentation.revision;
        assert!(!handle_client_message(
            &mut blocked,
            ClientMessage::Selection(session::SelectionAction::Begin {
                frame_revision: presentation.revision,
                cell: session::ViewportCell { x: 1, y: 0 },
            }),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(presentation.revision, stable_revision);
        assert_eq!(selection.copied.as_deref(), Some("alpha 界"));

        apply_pty_output(&mut terminal, &mut selection, b"\x1b[?1049h")?;
        assert_eq!(terminal.active_screen()?, Screen::Alternate);
        assert_eq!(selection.copied.as_deref(), Some("alpha 界"));
        assert!(selection.anchor.is_none());
        assert!(!selection.visible);
        Ok(())
    }

    #[test]
    fn conformance_c8_authoritative_viewport_routes_wheel_and_key_from_terminal_state() -> Result {
        let (mut client, mut peer) = attached_client()?;
        let pty = Pty::spawn(&["/bin/sh".into()], INITIAL_SIZE)?;
        let mut terminal = terminal_with_scrollback(100)?;
        for line in 0..32 {
            terminal.vt_write(format!("history-{line:02}\r\n").as_bytes());
        }
        let mut size = INITIAL_SIZE;
        let writes = RefCell::new(VecDeque::new());
        let mut presentation = Presentation::new()?;
        let mut selection = SelectionState::default();
        macro_rules! accept {
            ($message:expr, $frame:expr) => {{
                let previous_revision = presentation.revision;
                assert!(handle_client_message(
                    &mut client,
                    $message,
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut presentation,
                    &mut selection,
                )?);
                assert_eq!(
                    presentation.revision,
                    previous_revision + u64::from($frame)
                );
                assert_eq!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::Accepted
                );
                if $frame {
                    assert!(matches!(
                        flush_message(&mut client, &mut peer)?,
                        ServerMessage::Frame(frame) if frame.revision == presentation.revision
                    ));
                }
            }};
        }
        macro_rules! viewport_wheel {
            ($button:expr, $applied:expr) => {{
                let previous_revision = presentation.revision;
                assert!(handle_client_message(
                    &mut client,
                    wheel($button),
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut presentation,
                    &mut selection,
                )?);
                assert_eq!(presentation.revision, previous_revision + 1);
                assert!(matches!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::WheelOutcome(WheelOutcome::Viewport {
                        applied_rows,
                        frame,
                    }) if applied_rows == $applied && frame.revision == presentation.revision
                ));
            }};
        }
        macro_rules! terminal_wheel {
            ($button:expr) => {{
                let previous_revision = presentation.revision;
                assert!(handle_client_message(
                    &mut client,
                    wheel($button),
                    &mut terminal,
                    Some(&pty),
                    &mut size,
                    &writes,
                    &mut presentation,
                    &mut selection,
                )?);
                assert_eq!(presentation.revision, previous_revision);
                assert_eq!(
                    flush_message(&mut client, &mut peer)?,
                    ServerMessage::WheelOutcome(WheelOutcome::TerminalRouted)
                );
            }};
        }

        let live = terminal.scrollbar()?;
        assert_eq!(live.offset + live.len, live.total);
        let stable_revision = presentation.revision;
        let stable_offset = live.offset;
        {
            let selected = viewport_selection(
                &terminal,
                ViewportCell { x: 0, y: 0 },
                ViewportCell { x: 1, y: 0 },
            )?;
            terminal.set_selection(Some(&selected))?;
        }
        selection.anchor = Some(ViewportCell { x: 0, y: 0 });
        selection.visible = true;
        assert!(handle_client_message(
            &mut client,
            ClientMessage::PreviewVertical {
                frame_revision: stable_revision,
                direction: VerticalDirection::Up,
            },
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        let ServerMessage::VerticalPreview(VerticalPreview {
            frame_revision,
            direction: VerticalDirection::Up,
            outcome:
                PreviewOutcome::Viewport {
                    cols: 80,
                    edge_reached: false,
                    row: Some(row),
                },
        }) = flush_message(&mut client, &mut peer)?
        else {
            return Err("upward preview did not return one adjacent row".into());
        };
        assert_eq!(frame_revision, stable_revision);
        assert!(row.cells.iter().all(|cell| !cell.style.selected));
        assert_eq!(presentation.revision, stable_revision);
        assert_eq!(terminal.scrollbar()?.offset, stable_offset);
        assert!(selection.visible);

        assert!(handle_client_message(
            &mut client,
            ClientMessage::PreviewVertical {
                frame_revision: stable_revision,
                direction: VerticalDirection::Down,
            },
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert!(matches!(
            flush_message(&mut client, &mut peer)?,
            ServerMessage::VerticalPreview(VerticalPreview {
                outcome: PreviewOutcome::Viewport {
                    edge_reached: true,
                    row: None,
                    ..
                },
                ..
            })
        ));

        assert!(handle_client_message(
            &mut client,
            ClientMessage::PreviewVertical {
                frame_revision: stable_revision + 1,
                direction: VerticalDirection::Up,
            },
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert!(matches!(
            flush_message(&mut client, &mut peer)?,
            ServerMessage::Failure(Failure {
                code: FailureCode::InvalidInput,
                ..
            })
        ));
        assert_eq!(presentation.revision, stable_revision);
        assert_eq!(terminal.scrollbar()?.offset, stable_offset);

        viewport_wheel!(MouseButton::Four, -1);
        assert!(!selection.visible);
        let scrolled = terminal.scrollbar()?;
        assert_eq!(scrolled.offset + 1, live.offset);

        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Top);
        viewport_wheel!(MouseButton::Four, 0);
        assert_eq!(terminal.scrollbar()?.offset, 0);

        viewport_wheel!(MouseButton::Five, 1);
        assert_eq!(terminal.scrollbar()?.offset, 1);

        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Bottom);
        viewport_wheel!(MouseButton::Five, 0);
        let live = terminal.scrollbar()?;
        assert_eq!(live.offset + live.len, live.total);

        terminal.vt_write(b"\x1b[?1000h\x1b[?1006h");
        let tracked_revision = presentation.revision;
        assert!(handle_client_message(
            &mut client,
            ClientMessage::PreviewVertical {
                frame_revision: tracked_revision,
                direction: VerticalDirection::Up,
            },
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(
            flush_message(&mut client, &mut peer)?,
            ServerMessage::VerticalPreview(VerticalPreview {
                frame_revision: tracked_revision,
                direction: VerticalDirection::Up,
                outcome: PreviewOutcome::TerminalRouted,
            })
        );
        assert!(writes.borrow().is_empty());
        terminal_wheel!(MouseButton::Four);
        assert_eq!(
            writes.take().into_iter().collect::<Vec<_>>(),
            b"\x1b[<64;1;1M"
        );
        accept!(wheel(MouseButton::Six), false);
        assert_eq!(
            writes.take().into_iter().collect::<Vec<_>>(),
            b"\x1b[<66;1;1M"
        );

        terminal.vt_write(b"\x1b[?1000l\x1b[?1006l\x1b[?1049h\x1b[?1007h");
        for (modes, button, expected) in [
            (
                b"\x1b[?1l".as_slice(),
                MouseButton::Four,
                b"\x1b[A".as_slice(),
            ),
            (
                b"\x1b[?1h".as_slice(),
                MouseButton::Five,
                b"\x1bOB".as_slice(),
            ),
        ] {
            terminal.vt_write(modes);
            terminal_wheel!(button);
            assert_eq!(writes.take().into_iter().collect::<Vec<_>>(), expected);
        }

        terminal.vt_write(b"\x1b[?1007l");
        viewport_wheel!(MouseButton::Four, 0);
        assert!(writes.borrow().is_empty());

        terminal.vt_write(b"\x1b[?1049l");
        terminal.vt_write(b"\x1b[?2026h");
        let held_revision = presentation.revision;
        let held_offset = terminal.scrollbar()?.offset;
        for message in [
            ClientMessage::PreviewVertical {
                frame_revision: held_revision,
                direction: VerticalDirection::Up,
            },
            wheel(MouseButton::Four),
        ] {
            assert!(handle_client_message(
                &mut client,
                message,
                &mut terminal,
                Some(&pty),
                &mut size,
                &writes,
                &mut presentation,
                &mut selection,
            )?);
            assert!(matches!(
                flush_message(&mut client, &mut peer)?,
                ServerMessage::Failure(Failure {
                    code: FailureCode::Terminal,
                    ..
                })
            ));
            assert_eq!(presentation.revision, held_revision);
            assert_eq!(terminal.scrollbar()?.offset, held_offset);
        }
        terminal.vt_write(b"\x1b[?2026l");
        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(-1));
        let key = ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::A,
            modifiers: Modifiers::empty(),
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: Some("x".into()),
            unshifted_codepoint: Some('x'),
        });
        accept!(key, true);
        assert_eq!(writes.take().into_iter().collect::<Vec<_>>(), b"x");
        let live = terminal.scrollbar()?;
        assert_eq!(live.offset + live.len, live.total);

        terminal.scroll_viewport(libghostty_vt::terminal::ScrollViewport::Delta(-1));
        let before_empty = terminal.scrollbar()?;
        accept!(
            ClientMessage::Key(KeyEvent {
                action: KeyAction::Release,
                key: PhysicalKey::ENTER,
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: None,
                unshifted_codepoint: None,
            }),
            false
        );
        assert!(writes.borrow().is_empty());
        assert_eq!(terminal.scrollbar()?.offset, before_empty.offset);

        assert!(queue_pty_write(
            &mut writes.borrow_mut(),
            &vec![b'q'; MAX_PTY_WRITE_BYTES],
        ));
        assert!(handle_client_message(
            &mut client,
            ClientMessage::Key(KeyEvent {
                action: KeyAction::Press,
                key: PhysicalKey::A,
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: Some("rejected".into()),
                unshifted_codepoint: Some('r'),
            }),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(terminal.scrollbar()?.offset, before_empty.offset);
        assert_eq!(presentation.revision, 6);
        assert!(client.is_closing());
        assert!(matches!(
            flush_message(&mut client, &mut peer)?,
            ServerMessage::Failure(Failure {
                code: FailureCode::Terminal,
                ..
            })
        ));

        let (mut pressure_client, mut pressure_peer) = attached_client()?;
        terminal.vt_write(b"\x1b[?1000h\x1b[?1006h");
        let pressure_revision = presentation.revision;
        let pressure_offset = terminal.scrollbar()?.offset;
        assert!(handle_client_message(
            &mut pressure_client,
            wheel(MouseButton::Four),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(writes.borrow().len(), MAX_PTY_WRITE_BYTES);
        assert_eq!(presentation.revision, pressure_revision);
        assert_eq!(terminal.scrollbar()?.offset, pressure_offset);
        assert!(pressure_client.is_closing());
        assert!(matches!(
            flush_message(&mut pressure_client, &mut pressure_peer)?,
            ServerMessage::Failure(Failure {
                code: FailureCode::Terminal,
                ..
            })
        ));
        terminal.vt_write(b"\x1b[?1000l\x1b[?1006l");

        let (mut blocked_client, _) = attached_client()?;
        fill_output(&mut blocked_client)?;
        let blocked_revision = presentation.revision;
        let blocked_offset = terminal.scrollbar()?.offset;
        assert!(!handle_client_message(
            &mut blocked_client,
            wheel(MouseButton::Four),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(presentation.revision, blocked_revision);
        assert_eq!(terminal.scrollbar()?.offset, blocked_offset);
        let resized = SurfaceSize {
            rows: 25,
            screen_height: 400,
            ..INITIAL_SIZE
        };
        assert!(!handle_client_message(
            &mut blocked_client,
            ClientMessage::Resize(resized),
            &mut terminal,
            Some(&pty),
            &mut size,
            &writes,
            &mut presentation,
            &mut selection,
        )?);
        assert_eq!(size, resized);
        assert_eq!(presentation.revision, 7);
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
    fn kitty_release_never_falls_back_to_text() -> Result {
        let mut terminal = terminal()?;
        terminal.vt_write(b"\x1b[>3u");
        let release = |key, text: &str| {
            ClientMessage::Key(KeyEvent {
                action: KeyAction::Release,
                key,
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: Some(text.into()),
                unshifted_codepoint: None,
            })
        };

        assert_eq!(
            encode_input(&terminal, INITIAL_SIZE, release(PhysicalKey::SPACE, " "))?,
            b"\x1b[32;1:3u"
        );
        assert!(
            encode_input(
                &terminal,
                INITIAL_SIZE,
                release(PhysicalKey::UNIDENTIFIED, "!")
            )?
            .is_empty()
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
}
