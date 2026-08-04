//! Versioned, bounded messages for one local Orbit client session.

use std::{fmt, str};

use crate::{Frame, MAX_CELLS, MAX_FRAME_BYTES, MIN_FRAME_BYTES, decode_frame, encode_frame};

/// Local-session framing discriminator.
pub const MAGIC: &[u8; 4] = b"ORBS";
/// The only local-session revision understood by this package.
pub const VERSION: u16 = 1;
/// Fixed bytes before a message payload.
pub const HEADER_BYTES: usize = 12;
/// Largest payload accepted by the local-session decoder.
pub const MAX_PAYLOAD_BYTES: usize = MAX_FRAME_BYTES;
/// Largest paste accepted as one semantic event.
pub const MAX_PASTE_BYTES: usize = 1024 * 1024;
/// Largest text associated with one key event.
pub const MAX_KEY_TEXT_BYTES: usize = 4096;
/// Largest client-visible failure detail.
pub const MAX_FAILURE_BYTES: usize = 1024;

const CLIENT_HELLO: u8 = 1;
const CLIENT_KEY: u8 = 2;
const CLIENT_MOUSE: u8 = 3;
const CLIENT_FOCUS: u8 = 4;
const CLIENT_PASTE: u8 = 5;
const CLIENT_RESIZE: u8 = 6;
const SERVER_ATTACHED: u8 = 129;
const SERVER_BUSY: u8 = 130;
const SERVER_INCOMPATIBLE: u8 = 131;
const SERVER_FRAME: u8 = 132;
const SERVER_ACCEPTED: u8 = 133;
const SERVER_FAILURE: u8 = 134;
const SERVER_EXITED: u8 = 135;

/// A local-session validation or framing failure.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The message does not start with [`MAGIC`].
    InvalidMagic,
    /// The framing revision is unsupported.
    UnsupportedVersion { version: u16 },
    /// Reserved header flags are set.
    InvalidFlags { value: u8 },
    /// A payload exceeds its message-specific bound.
    PayloadTooLarge { size: usize, maximum: usize },
    /// The message ends before its declared payload does.
    Truncated,
    /// A complete message is followed by undeclared bytes.
    TrailingBytes { count: usize },
    /// A tag is unknown or invalid for the field being decoded.
    InvalidTag { field: &'static str, value: u8 },
    /// A non-tag scalar value is invalid for its field.
    InvalidValue { field: &'static str },
    /// A UTF-8 field is malformed.
    InvalidUtf8 { field: &'static str },
    /// A version range is empty or reversed.
    InvalidVersionRange,
    /// Consumed modifiers are not a subset of active modifiers.
    ConsumedModifiersNotActive,
    /// Mouse coordinates are negative or non-finite.
    InvalidCoordinates,
    /// Surface dimensions are empty, inconsistent, or too large.
    InvalidSurfaceSize,
    /// An embedded ORBF frame is invalid.
    Frame(crate::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("invalid local-session message magic"),
            Self::UnsupportedVersion { version } => {
                write!(formatter, "unsupported local-session version {version}")
            }
            Self::InvalidFlags { value } => {
                write!(formatter, "invalid local-session flags {value:#x}")
            }
            Self::PayloadTooLarge { size, maximum } => {
                write!(
                    formatter,
                    "message payload is {size} bytes; maximum is {maximum}"
                )
            }
            Self::Truncated => formatter.write_str("truncated local-session message"),
            Self::TrailingBytes { count } => {
                write!(
                    formatter,
                    "local-session message has {count} trailing bytes"
                )
            }
            Self::InvalidTag { field, value } => {
                write!(formatter, "invalid local-session {field} tag {value}")
            }
            Self::InvalidValue { field } => {
                write!(formatter, "invalid local-session {field}")
            }
            Self::InvalidUtf8 { field } => {
                write!(formatter, "local-session {field} is not UTF-8")
            }
            Self::InvalidVersionRange => formatter.write_str("invalid protocol version range"),
            Self::ConsumedModifiersNotActive => {
                formatter.write_str("consumed key modifiers are not active")
            }
            Self::InvalidCoordinates => formatter.write_str("invalid mouse coordinates"),
            Self::InvalidSurfaceSize => formatter.write_str("invalid surface size"),
            Self::Frame(error) => write!(formatter, "invalid presentation frame: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::Error> for Error {
    fn from(value: crate::Error) -> Self {
        Self::Frame(value)
    }
}

/// Local-session codec result.
pub type Result<T> = std::result::Result<T, Error>;

/// A client-to-Orbit semantic message.
#[derive(Clone, Debug, PartialEq)]
pub enum ClientMessage {
    /// Opens a session whose supported revisions overlap this inclusive range.
    Hello {
        minimum_version: u16,
        maximum_version: u16,
    },
    /// One physical key event and its text meaning.
    Key(KeyEvent),
    /// One pointer event in surface pixels.
    Mouse(MouseEvent),
    /// A native focus transition.
    Focus(FocusEvent),
    /// Opaque pasted bytes; embedded newlines and NULs are preserved.
    Paste(Vec<u8>),
    /// A requested terminal and drawing-surface size.
    Resize(SurfaceSize),
}

/// An Orbit-to-client session message.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerMessage {
    /// The client owns the single attachment at this negotiated revision.
    Attached { version: u16 },
    /// Another client already owns the single attachment.
    Busy,
    /// The offered revisions do not overlap Orbit's accepted revision.
    Incompatible {
        minimum_version: u16,
        maximum_version: u16,
    },
    /// One canonical, complete Orbit presentation frame.
    Frame(Box<Frame>),
    /// Orbit accepted a semantic event.
    Accepted,
    /// Orbit rejected a message without inventing terminal input.
    Failure(Failure),
    /// The authoritative child process exited.
    Exited { code: i32 },
}

/// Physical key action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Press,
    Release,
    Repeat,
}

/// Stable physical key identity matching Orbit's terminal-aware mapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalKey(u16);

impl PhysicalKey {
    /// Highest physical-key identity in local-session v1.
    pub const MAX_RAW: u16 = 175;
    pub const UNIDENTIFIED: Self = Self(0);
    pub const BACKQUOTE: Self = Self(1);
    pub const BACKSLASH: Self = Self(2);
    pub const BRACKET_LEFT: Self = Self(3);
    pub const BRACKET_RIGHT: Self = Self(4);
    pub const COMMA: Self = Self(5);
    pub const DIGIT_0: Self = Self(6);
    pub const DIGIT_1: Self = Self(7);
    pub const DIGIT_2: Self = Self(8);
    pub const DIGIT_3: Self = Self(9);
    pub const DIGIT_4: Self = Self(10);
    pub const DIGIT_5: Self = Self(11);
    pub const DIGIT_6: Self = Self(12);
    pub const DIGIT_7: Self = Self(13);
    pub const DIGIT_8: Self = Self(14);
    pub const DIGIT_9: Self = Self(15);
    pub const EQUAL: Self = Self(16);
    pub const INTL_BACKSLASH: Self = Self(17);
    pub const INTL_RO: Self = Self(18);
    pub const INTL_YEN: Self = Self(19);
    pub const A: Self = Self(20);
    pub const B: Self = Self(21);
    pub const C: Self = Self(22);
    pub const D: Self = Self(23);
    pub const E: Self = Self(24);
    pub const F: Self = Self(25);
    pub const G: Self = Self(26);
    pub const H: Self = Self(27);
    pub const I: Self = Self(28);
    pub const J: Self = Self(29);
    pub const K: Self = Self(30);
    pub const L: Self = Self(31);
    pub const M: Self = Self(32);
    pub const N: Self = Self(33);
    pub const O: Self = Self(34);
    pub const P: Self = Self(35);
    pub const Q: Self = Self(36);
    pub const R: Self = Self(37);
    pub const S: Self = Self(38);
    pub const T: Self = Self(39);
    pub const U: Self = Self(40);
    pub const V: Self = Self(41);
    pub const W: Self = Self(42);
    pub const X: Self = Self(43);
    pub const Y: Self = Self(44);
    pub const Z: Self = Self(45);
    pub const MINUS: Self = Self(46);
    pub const PERIOD: Self = Self(47);
    pub const QUOTE: Self = Self(48);
    pub const SEMICOLON: Self = Self(49);
    pub const SLASH: Self = Self(50);
    pub const ALT_LEFT: Self = Self(51);
    pub const ALT_RIGHT: Self = Self(52);
    pub const BACKSPACE: Self = Self(53);
    pub const CAPS_LOCK: Self = Self(54);
    pub const CONTEXT_MENU: Self = Self(55);
    pub const CONTROL_LEFT: Self = Self(56);
    pub const CONTROL_RIGHT: Self = Self(57);
    pub const ENTER: Self = Self(58);
    pub const META_LEFT: Self = Self(59);
    pub const META_RIGHT: Self = Self(60);
    pub const SHIFT_LEFT: Self = Self(61);
    pub const SHIFT_RIGHT: Self = Self(62);
    pub const SPACE: Self = Self(63);
    pub const TAB: Self = Self(64);
    pub const CONVERT: Self = Self(65);
    pub const KANA_MODE: Self = Self(66);
    pub const NON_CONVERT: Self = Self(67);
    pub const DELETE: Self = Self(68);
    pub const END: Self = Self(69);
    pub const HELP: Self = Self(70);
    pub const HOME: Self = Self(71);
    pub const INSERT: Self = Self(72);
    pub const PAGE_DOWN: Self = Self(73);
    pub const PAGE_UP: Self = Self(74);
    pub const ARROW_DOWN: Self = Self(75);
    pub const ARROW_LEFT: Self = Self(76);
    pub const ARROW_RIGHT: Self = Self(77);
    pub const ARROW_UP: Self = Self(78);
    pub const NUM_LOCK: Self = Self(79);
    pub const NUMPAD_0: Self = Self(80);
    pub const NUMPAD_1: Self = Self(81);
    pub const NUMPAD_2: Self = Self(82);
    pub const NUMPAD_3: Self = Self(83);
    pub const NUMPAD_4: Self = Self(84);
    pub const NUMPAD_5: Self = Self(85);
    pub const NUMPAD_6: Self = Self(86);
    pub const NUMPAD_7: Self = Self(87);
    pub const NUMPAD_8: Self = Self(88);
    pub const NUMPAD_9: Self = Self(89);
    pub const NUMPAD_ADD: Self = Self(90);
    pub const NUMPAD_BACKSPACE: Self = Self(91);
    pub const NUMPAD_CLEAR: Self = Self(92);
    pub const NUMPAD_CLEAR_ENTRY: Self = Self(93);
    pub const NUMPAD_COMMA: Self = Self(94);
    pub const NUMPAD_DECIMAL: Self = Self(95);
    pub const NUMPAD_DIVIDE: Self = Self(96);
    pub const NUMPAD_ENTER: Self = Self(97);
    pub const NUMPAD_EQUAL: Self = Self(98);
    pub const NUMPAD_MEMORY_ADD: Self = Self(99);
    pub const NUMPAD_MEMORY_CLEAR: Self = Self(100);
    pub const NUMPAD_MEMORY_RECALL: Self = Self(101);
    pub const NUMPAD_MEMORY_STORE: Self = Self(102);
    pub const NUMPAD_MEMORY_SUBTRACT: Self = Self(103);
    pub const NUMPAD_MULTIPLY: Self = Self(104);
    pub const NUMPAD_PAREN_LEFT: Self = Self(105);
    pub const NUMPAD_PAREN_RIGHT: Self = Self(106);
    pub const NUMPAD_SUBTRACT: Self = Self(107);
    pub const NUMPAD_SEPARATOR: Self = Self(108);
    pub const NUMPAD_UP: Self = Self(109);
    pub const NUMPAD_DOWN: Self = Self(110);
    pub const NUMPAD_RIGHT: Self = Self(111);
    pub const NUMPAD_LEFT: Self = Self(112);
    pub const NUMPAD_BEGIN: Self = Self(113);
    pub const NUMPAD_HOME: Self = Self(114);
    pub const NUMPAD_END: Self = Self(115);
    pub const NUMPAD_INSERT: Self = Self(116);
    pub const NUMPAD_DELETE: Self = Self(117);
    pub const NUMPAD_PAGE_UP: Self = Self(118);
    pub const NUMPAD_PAGE_DOWN: Self = Self(119);
    pub const ESCAPE: Self = Self(120);
    pub const F1: Self = Self(121);
    pub const F2: Self = Self(122);
    pub const F3: Self = Self(123);
    pub const F4: Self = Self(124);
    pub const F5: Self = Self(125);
    pub const F6: Self = Self(126);
    pub const F7: Self = Self(127);
    pub const F8: Self = Self(128);
    pub const F9: Self = Self(129);
    pub const F10: Self = Self(130);
    pub const F11: Self = Self(131);
    pub const F12: Self = Self(132);
    pub const F13: Self = Self(133);
    pub const F14: Self = Self(134);
    pub const F15: Self = Self(135);
    pub const F16: Self = Self(136);
    pub const F17: Self = Self(137);
    pub const F18: Self = Self(138);
    pub const F19: Self = Self(139);
    pub const F20: Self = Self(140);
    pub const F21: Self = Self(141);
    pub const F22: Self = Self(142);
    pub const F23: Self = Self(143);
    pub const F24: Self = Self(144);
    pub const F25: Self = Self(145);
    pub const FN: Self = Self(146);
    pub const FN_LOCK: Self = Self(147);
    pub const PRINT_SCREEN: Self = Self(148);
    pub const SCROLL_LOCK: Self = Self(149);
    pub const PAUSE: Self = Self(150);
    pub const BROWSER_BACK: Self = Self(151);
    pub const BROWSER_FAVORITES: Self = Self(152);
    pub const BROWSER_FORWARD: Self = Self(153);
    pub const BROWSER_HOME: Self = Self(154);
    pub const BROWSER_REFRESH: Self = Self(155);
    pub const BROWSER_SEARCH: Self = Self(156);
    pub const BROWSER_STOP: Self = Self(157);
    pub const EJECT: Self = Self(158);
    pub const LAUNCH_APP_1: Self = Self(159);
    pub const LAUNCH_APP_2: Self = Self(160);
    pub const LAUNCH_MAIL: Self = Self(161);
    pub const MEDIA_PLAY_PAUSE: Self = Self(162);
    pub const MEDIA_SELECT: Self = Self(163);
    pub const MEDIA_STOP: Self = Self(164);
    pub const MEDIA_TRACK_NEXT: Self = Self(165);
    pub const MEDIA_TRACK_PREVIOUS: Self = Self(166);
    pub const POWER: Self = Self(167);
    pub const SLEEP: Self = Self(168);
    pub const AUDIO_VOLUME_DOWN: Self = Self(169);
    pub const AUDIO_VOLUME_MUTE: Self = Self(170);
    pub const AUDIO_VOLUME_UP: Self = Self(171);
    pub const WAKE_UP: Self = Self(172);
    pub const COPY: Self = Self(173);
    pub const CUT: Self = Self(174);
    pub const PASTE: Self = Self(175);

    /// Constructs a physical key if it belongs to the v1 identity set.
    pub const fn from_raw(value: u16) -> Option<Self> {
        if value <= Self::MAX_RAW {
            Some(Self(value))
        } else {
            None
        }
    }

    /// Returns the stable v1 identity.
    pub const fn raw(self) -> u16 {
        self.0
    }
}

/// Modifier bits, including lock and left/right-side information.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers(u16);

impl Modifiers {
    pub const SHIFT: Self = Self(1);
    pub const CTRL: Self = Self(2);
    pub const ALT: Self = Self(4);
    pub const SUPER: Self = Self(8);
    pub const CAPS_LOCK: Self = Self(16);
    pub const NUM_LOCK: Self = Self(32);
    pub const SHIFT_SIDE: Self = Self(64);
    pub const CTRL_SIDE: Self = Self(128);
    pub const ALT_SIDE: Self = Self(256);
    pub const SUPER_SIDE: Self = Self(512);
    pub const MASK: u16 = 1023;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_bits(value: u16) -> Option<Self> {
        let valid_sides = (value & Self::SHIFT_SIDE.0 == 0 || value & Self::SHIFT.0 != 0)
            && (value & Self::CTRL_SIDE.0 == 0 || value & Self::CTRL.0 != 0)
            && (value & Self::ALT_SIDE.0 == 0 || value & Self::ALT.0 != 0)
            && (value & Self::SUPER_SIDE.0 == 0 || value & Self::SUPER.0 != 0);
        if value & !Self::MASK == 0 && valid_sides {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// Complete semantic information for one key event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub action: KeyAction,
    pub key: PhysicalKey,
    pub modifiers: Modifiers,
    pub consumed_modifiers: Modifiers,
    pub composing: bool,
    pub text: Option<String>,
    pub unshifted_codepoint: Option<char>,
}

/// Mouse action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Release,
    Motion,
}

/// Mouse button identity. Four through Seven are wheel directions and are
/// valid only with [`MouseAction::Press`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Unknown,
    Left,
    Middle,
    Right,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Eleven,
}

/// Pointer event in surface pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MouseEvent {
    pub action: MouseAction,
    /// `None` is valid only for motion with no pressed button.
    pub button: Option<MouseButton>,
    pub modifiers: Modifiers,
    pub x: f32,
    pub y: f32,
}

/// Native focus transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusEvent {
    Gained,
    Lost,
}

/// Terminal grid and drawing-surface measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceSize {
    pub cols: u16,
    pub rows: u16,
    pub screen_width: u32,
    pub screen_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_left: u32,
    pub padding_right: u32,
}

/// Stable class of client-visible failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureCode {
    InvalidInput,
    Protocol,
    Terminal,
}

/// Bounded client-visible failure detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: FailureCode,
    pub detail: String,
}

/// Returns the complete client-message length once a valid header is available.
pub fn client_message_len(bytes: &[u8]) -> Result<Option<usize>> {
    message_len(bytes, client_payload_limits)
}

/// Returns the complete server-message length once a valid header is available.
pub fn server_message_len(bytes: &[u8]) -> Result<Option<usize>> {
    message_len(bytes, server_payload_limits)
}

fn message_len(
    bytes: &[u8],
    payload_limits: fn(u8) -> Result<(usize, usize)>,
) -> Result<Option<usize>> {
    if bytes.len() < HEADER_BYTES {
        return Ok(None);
    }
    if &bytes[..4] != MAGIC {
        return Err(Error::InvalidMagic);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != VERSION {
        return Err(Error::UnsupportedVersion { version });
    }
    if bytes[7] != 0 {
        return Err(Error::InvalidFlags { value: bytes[7] });
    }
    let (minimum, maximum) = payload_limits(bytes[6])?;
    let payload = u32::from_le_bytes(bytes[8..12].try_into().expect("fixed header")) as usize;
    validate_bound(payload, maximum)?;
    if payload < minimum {
        return Err(Error::InvalidValue {
            field: "message payload length",
        });
    }
    Ok(Some(HEADER_BYTES + payload))
}

fn client_payload_limits(kind: u8) -> Result<(usize, usize)> {
    match kind {
        CLIENT_HELLO => Ok((4, 4)),
        CLIENT_KEY => Ok((16, 16 + MAX_KEY_TEXT_BYTES)),
        CLIENT_MOUSE => Ok((12, 12)),
        CLIENT_FOCUS => Ok((1, 1)),
        CLIENT_PASTE => Ok((0, MAX_PASTE_BYTES)),
        CLIENT_RESIZE => Ok((36, 36)),
        value => Err(Error::InvalidTag {
            field: "client message",
            value,
        }),
    }
}

fn server_payload_limits(kind: u8) -> Result<(usize, usize)> {
    match kind {
        SERVER_ATTACHED => Ok((2, 2)),
        SERVER_BUSY | SERVER_ACCEPTED => Ok((0, 0)),
        SERVER_INCOMPATIBLE | SERVER_EXITED => Ok((4, 4)),
        SERVER_FRAME => Ok((MIN_FRAME_BYTES, MAX_PAYLOAD_BYTES)),
        SERVER_FAILURE => Ok((5, 5 + MAX_FAILURE_BYTES)),
        value => Err(Error::InvalidTag {
            field: "server message",
            value,
        }),
    }
}

/// Encodes one client message with an ORBS v1 header.
pub fn encode_client_message(message: &ClientMessage) -> Result<Vec<u8>> {
    let mut payload = Vec::new();
    let kind = match message {
        ClientMessage::Hello {
            minimum_version,
            maximum_version,
        } => {
            validate_version_range(*minimum_version, *maximum_version)?;
            put_u16(&mut payload, *minimum_version);
            put_u16(&mut payload, *maximum_version);
            CLIENT_HELLO
        }
        ClientMessage::Key(event) => {
            validate_key(event)?;
            payload.push(key_action_tag(event.action));
            put_u16(&mut payload, event.key.raw());
            put_u16(&mut payload, event.modifiers.bits());
            put_u16(&mut payload, event.consumed_modifiers.bits());
            payload.push(u8::from(event.composing));
            put_optional_text(&mut payload, event.text.as_deref())?;
            put_u32(
                &mut payload,
                event.unshifted_codepoint.map_or(u32::MAX, u32::from),
            );
            CLIENT_KEY
        }
        ClientMessage::Mouse(event) => {
            validate_mouse(event)?;
            payload.push(mouse_action_tag(event.action));
            payload.push(
                event
                    .button
                    .map_or(0, |button| mouse_button_tag(button) + 1),
            );
            put_u16(&mut payload, event.modifiers.bits());
            put_u32(&mut payload, event.x.to_bits());
            put_u32(&mut payload, event.y.to_bits());
            CLIENT_MOUSE
        }
        ClientMessage::Focus(event) => {
            payload.push(match event {
                FocusEvent::Gained => 0,
                FocusEvent::Lost => 1,
            });
            CLIENT_FOCUS
        }
        ClientMessage::Paste(bytes) => {
            validate_bound(bytes.len(), MAX_PASTE_BYTES)?;
            payload.extend_from_slice(bytes);
            CLIENT_PASTE
        }
        ClientMessage::Resize(size) => {
            validate_surface_size(size)?;
            put_u16(&mut payload, size.cols);
            put_u16(&mut payload, size.rows);
            for value in [
                size.screen_width,
                size.screen_height,
                size.cell_width,
                size.cell_height,
                size.padding_top,
                size.padding_bottom,
                size.padding_left,
                size.padding_right,
            ] {
                put_u32(&mut payload, value);
            }
            CLIENT_RESIZE
        }
    };
    frame_message(kind, payload)
}

/// Decodes exactly one client message.
pub fn decode_client_message(bytes: &[u8]) -> Result<ClientMessage> {
    let (kind, payload) = exact_message(bytes, client_message_len(bytes)?)?;
    let mut reader = Reader::new(payload);
    let message = match kind {
        CLIENT_HELLO => {
            let minimum_version = reader.u16()?;
            let maximum_version = reader.u16()?;
            validate_version_range(minimum_version, maximum_version)?;
            ClientMessage::Hello {
                minimum_version,
                maximum_version,
            }
        }
        CLIENT_KEY => {
            let action = decode_key_action(reader.u8()?)?;
            let raw_key = reader.u16()?;
            let key = PhysicalKey::from_raw(raw_key).ok_or(Error::InvalidValue {
                field: "physical key",
            })?;
            let modifiers = checked_modifiers(reader.u16()?)?;
            let consumed_modifiers = checked_modifiers(reader.u16()?)?;
            let composing = decode_bool(reader.u8()?, "composing")?;
            let text = reader.optional_text("key text", MAX_KEY_TEXT_BYTES)?;
            let raw_codepoint = reader.u32()?;
            let unshifted_codepoint = if raw_codepoint == u32::MAX {
                None
            } else {
                Some(char::from_u32(raw_codepoint).ok_or(Error::InvalidValue {
                    field: "unshifted codepoint",
                })?)
            };
            let event = KeyEvent {
                action,
                key,
                modifiers,
                consumed_modifiers,
                composing,
                text,
                unshifted_codepoint,
            };
            validate_key(&event)?;
            ClientMessage::Key(event)
        }
        CLIENT_MOUSE => {
            let action = decode_mouse_action(reader.u8()?)?;
            let button_tag = reader.u8()?;
            let button = if button_tag == 0 {
                None
            } else {
                Some(decode_mouse_button(button_tag - 1)?)
            };
            let modifiers = checked_modifiers(reader.u16()?)?;
            let event = MouseEvent {
                action,
                button,
                modifiers,
                x: f32::from_bits(reader.u32()?),
                y: f32::from_bits(reader.u32()?),
            };
            validate_mouse(&event)?;
            ClientMessage::Mouse(event)
        }
        CLIENT_FOCUS => ClientMessage::Focus(match reader.u8()? {
            0 => FocusEvent::Gained,
            1 => FocusEvent::Lost,
            value => {
                return Err(Error::InvalidTag {
                    field: "focus",
                    value,
                });
            }
        }),
        CLIENT_PASTE => {
            validate_bound(payload.len(), MAX_PASTE_BYTES)?;
            reader.take_remaining();
            ClientMessage::Paste(payload.to_vec())
        }
        CLIENT_RESIZE => {
            let size = SurfaceSize {
                cols: reader.u16()?,
                rows: reader.u16()?,
                screen_width: reader.u32()?,
                screen_height: reader.u32()?,
                cell_width: reader.u32()?,
                cell_height: reader.u32()?,
                padding_top: reader.u32()?,
                padding_bottom: reader.u32()?,
                padding_left: reader.u32()?,
                padding_right: reader.u32()?,
            };
            validate_surface_size(&size)?;
            ClientMessage::Resize(size)
        }
        value => {
            return Err(Error::InvalidTag {
                field: "client message",
                value,
            });
        }
    };
    reader.finish()?;
    Ok(message)
}

/// Encodes one server message with an ORBS v1 header.
pub fn encode_server_message(message: &ServerMessage) -> Result<Vec<u8>> {
    let mut payload = Vec::new();
    let kind = match message {
        ServerMessage::Attached { version } => {
            if *version != VERSION {
                return Err(Error::UnsupportedVersion { version: *version });
            }
            put_u16(&mut payload, *version);
            SERVER_ATTACHED
        }
        ServerMessage::Busy => SERVER_BUSY,
        ServerMessage::Incompatible {
            minimum_version,
            maximum_version,
        } => {
            validate_version_range(*minimum_version, *maximum_version)?;
            put_u16(&mut payload, *minimum_version);
            put_u16(&mut payload, *maximum_version);
            SERVER_INCOMPATIBLE
        }
        ServerMessage::Frame(frame) => {
            payload = encode_frame(frame)?;
            SERVER_FRAME
        }
        ServerMessage::Accepted => SERVER_ACCEPTED,
        ServerMessage::Failure(failure) => {
            payload.push(failure_code_tag(failure.code));
            put_bounded_bytes(&mut payload, failure.detail.as_bytes(), MAX_FAILURE_BYTES)?;
            SERVER_FAILURE
        }
        ServerMessage::Exited { code } => {
            payload.extend_from_slice(&code.to_le_bytes());
            SERVER_EXITED
        }
    };
    frame_message(kind, payload)
}

/// Decodes exactly one server message.
pub fn decode_server_message(bytes: &[u8]) -> Result<ServerMessage> {
    let (kind, payload) = exact_message(bytes, server_message_len(bytes)?)?;
    if kind == SERVER_FRAME {
        return Ok(ServerMessage::Frame(Box::new(decode_frame(payload)?)));
    }
    let mut reader = Reader::new(payload);
    let message = match kind {
        SERVER_ATTACHED => {
            let version = reader.u16()?;
            if version != VERSION {
                return Err(Error::UnsupportedVersion { version });
            }
            ServerMessage::Attached { version }
        }
        SERVER_BUSY => ServerMessage::Busy,
        SERVER_INCOMPATIBLE => {
            let minimum_version = reader.u16()?;
            let maximum_version = reader.u16()?;
            validate_version_range(minimum_version, maximum_version)?;
            ServerMessage::Incompatible {
                minimum_version,
                maximum_version,
            }
        }
        SERVER_ACCEPTED => ServerMessage::Accepted,
        SERVER_FAILURE => {
            let code = decode_failure_code(reader.u8()?)?;
            let detail = reader.bounded_text("failure detail", MAX_FAILURE_BYTES)?;
            ServerMessage::Failure(Failure { code, detail })
        }
        SERVER_EXITED => ServerMessage::Exited {
            code: i32::from_le_bytes(reader.array()?),
        },
        value => {
            return Err(Error::InvalidTag {
                field: "server message",
                value,
            });
        }
    };
    reader.finish()?;
    Ok(message)
}

fn frame_message(kind: u8, payload: Vec<u8>) -> Result<Vec<u8>> {
    validate_bound(payload.len(), MAX_PAYLOAD_BYTES)?;
    let mut framed = Vec::with_capacity(HEADER_BYTES + payload.len());
    framed.extend_from_slice(MAGIC);
    put_u16(&mut framed, VERSION);
    framed.push(kind);
    framed.push(0);
    put_u32(
        &mut framed,
        u32::try_from(payload.len()).expect("payload bound fits u32"),
    );
    framed.extend_from_slice(&payload);
    Ok(framed)
}

fn exact_message(bytes: &[u8], length: Option<usize>) -> Result<(u8, &[u8])> {
    let Some(length) = length else {
        return Err(Error::Truncated);
    };
    if bytes.len() < length {
        return Err(Error::Truncated);
    }
    if bytes.len() > length {
        return Err(Error::TrailingBytes {
            count: bytes.len() - length,
        });
    }
    Ok((bytes[6], &bytes[HEADER_BYTES..]))
}

fn validate_version_range(minimum: u16, maximum: u16) -> Result<()> {
    if minimum == 0 || minimum > maximum {
        Err(Error::InvalidVersionRange)
    } else {
        Ok(())
    }
}

fn validate_bound(size: usize, maximum: usize) -> Result<()> {
    if size > maximum {
        Err(Error::PayloadTooLarge { size, maximum })
    } else {
        Ok(())
    }
}

fn validate_key(event: &KeyEvent) -> Result<()> {
    checked_modifiers(event.modifiers.bits())?;
    checked_modifiers(event.consumed_modifiers.bits())?;
    if !event.modifiers.contains(event.consumed_modifiers) {
        return Err(Error::ConsumedModifiersNotActive);
    }
    if let Some(text) = &event.text {
        validate_bound(text.len(), MAX_KEY_TEXT_BYTES)?;
        if text.chars().any(|character| {
            matches!(
                character,
                '\0'..='\u{1f}' | '\u{7f}' | '\u{f700}'..='\u{f8ff}'
            )
        }) {
            return Err(Error::InvalidValue { field: "key text" });
        }
    }
    Ok(())
}

fn validate_mouse(event: &MouseEvent) -> Result<()> {
    checked_modifiers(event.modifiers.bits())?;
    if event.button.is_none() && event.action != MouseAction::Motion {
        return Err(Error::InvalidValue {
            field: "mouse button",
        });
    }
    if matches!(
        event.button,
        Some(MouseButton::Four | MouseButton::Five | MouseButton::Six | MouseButton::Seven)
    ) && event.action != MouseAction::Press
    {
        return Err(Error::InvalidValue {
            field: "mouse action",
        });
    }
    if event.x.is_finite() && event.y.is_finite() && event.x >= 0.0 && event.y >= 0.0 {
        Ok(())
    } else {
        Err(Error::InvalidCoordinates)
    }
}

fn validate_surface_size(size: &SurfaceSize) -> Result<()> {
    let cells = usize::from(size.cols).checked_mul(usize::from(size.rows));
    let horizontal_padding = size.padding_left.checked_add(size.padding_right);
    let vertical_padding = size.padding_top.checked_add(size.padding_bottom);
    let grid_width = u32::from(size.cols).checked_mul(size.cell_width);
    let grid_height = u32::from(size.rows).checked_mul(size.cell_height);
    let used_width = grid_width.and_then(|grid| horizontal_padding?.checked_add(grid));
    let used_height = grid_height.and_then(|grid| vertical_padding?.checked_add(grid));
    if size.cols == 0
        || size.rows == 0
        || size.screen_width == 0
        || size.screen_height == 0
        || size.cell_width == 0
        || size.cell_height == 0
        || !matches!(cells, Some(count) if count <= MAX_CELLS)
        || !matches!(horizontal_padding, Some(padding) if padding < size.screen_width)
        || !matches!(vertical_padding, Some(padding) if padding < size.screen_height)
        || !matches!(used_width, Some(used) if used <= size.screen_width)
        || !matches!(used_height, Some(used) if used <= size.screen_height)
    {
        return Err(Error::InvalidSurfaceSize);
    }
    Ok(())
}

fn key_action_tag(action: KeyAction) -> u8 {
    match action {
        KeyAction::Press => 0,
        KeyAction::Release => 1,
        KeyAction::Repeat => 2,
    }
}

fn decode_key_action(value: u8) -> Result<KeyAction> {
    match value {
        0 => Ok(KeyAction::Press),
        1 => Ok(KeyAction::Release),
        2 => Ok(KeyAction::Repeat),
        value => Err(Error::InvalidTag {
            field: "key action",
            value,
        }),
    }
}

fn mouse_action_tag(action: MouseAction) -> u8 {
    match action {
        MouseAction::Press => 0,
        MouseAction::Release => 1,
        MouseAction::Motion => 2,
    }
}

fn decode_mouse_action(value: u8) -> Result<MouseAction> {
    match value {
        0 => Ok(MouseAction::Press),
        1 => Ok(MouseAction::Release),
        2 => Ok(MouseAction::Motion),
        value => Err(Error::InvalidTag {
            field: "mouse action",
            value,
        }),
    }
}

fn mouse_button_tag(button: MouseButton) -> u8 {
    match button {
        MouseButton::Unknown => 0,
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::Four => 4,
        MouseButton::Five => 5,
        MouseButton::Six => 6,
        MouseButton::Seven => 7,
        MouseButton::Eight => 8,
        MouseButton::Nine => 9,
        MouseButton::Ten => 10,
        MouseButton::Eleven => 11,
    }
}

fn decode_mouse_button(value: u8) -> Result<MouseButton> {
    match value {
        0 => Ok(MouseButton::Unknown),
        1 => Ok(MouseButton::Left),
        2 => Ok(MouseButton::Middle),
        3 => Ok(MouseButton::Right),
        4 => Ok(MouseButton::Four),
        5 => Ok(MouseButton::Five),
        6 => Ok(MouseButton::Six),
        7 => Ok(MouseButton::Seven),
        8 => Ok(MouseButton::Eight),
        9 => Ok(MouseButton::Nine),
        10 => Ok(MouseButton::Ten),
        11 => Ok(MouseButton::Eleven),
        value => Err(Error::InvalidTag {
            field: "mouse button",
            value,
        }),
    }
}

fn failure_code_tag(code: FailureCode) -> u8 {
    match code {
        FailureCode::InvalidInput => 0,
        FailureCode::Protocol => 1,
        FailureCode::Terminal => 2,
    }
}

fn decode_failure_code(value: u8) -> Result<FailureCode> {
    match value {
        0 => Ok(FailureCode::InvalidInput),
        1 => Ok(FailureCode::Protocol),
        2 => Ok(FailureCode::Terminal),
        value => Err(Error::InvalidTag {
            field: "failure code",
            value,
        }),
    }
}

fn checked_modifiers(value: u16) -> Result<Modifiers> {
    Modifiers::from_bits(value).ok_or(Error::InvalidValue { field: "modifiers" })
}

fn decode_bool(value: u8, field: &'static str) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(Error::InvalidTag { field, value }),
    }
}

fn put_optional_text(target: &mut Vec<u8>, value: Option<&str>) -> Result<()> {
    match value {
        Some(value) => put_bounded_bytes(target, value.as_bytes(), MAX_KEY_TEXT_BYTES),
        None => {
            put_u32(target, u32::MAX);
            Ok(())
        }
    }
}

fn put_bounded_bytes(target: &mut Vec<u8>, value: &[u8], maximum: usize) -> Result<()> {
    validate_bound(value.len(), maximum)?;
    put_u32(
        target,
        u32::try_from(value.len()).expect("message-specific bound fits u32"),
    );
    target.extend_from_slice(value);
    Ok(())
}

fn put_u16(target: &mut Vec<u8>, value: u16) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| Error::Truncated)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(length).ok_or(Error::Truncated)?;
        let value = self.bytes.get(self.offset..end).ok_or(Error::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn take_remaining(&mut self) {
        self.offset = self.bytes.len();
    }

    fn optional_text(&mut self, field: &'static str, maximum: usize) -> Result<Option<String>> {
        let length = self.u32()?;
        if length == u32::MAX {
            return Ok(None);
        }
        self.text_with_length(field, length as usize, maximum)
            .map(Some)
    }

    fn bounded_text(&mut self, field: &'static str, maximum: usize) -> Result<String> {
        let length = self.u32()? as usize;
        self.text_with_length(field, length, maximum)
    }

    fn text_with_length(
        &mut self,
        field: &'static str,
        length: usize,
        maximum: usize,
    ) -> Result<String> {
        validate_bound(length, maximum)?;
        let bytes = self.take(length)?;
        str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| Error::InvalidUtf8 { field })
    }

    fn finish(self) -> Result<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::TrailingBytes {
                count: self.bytes.len() - self.offset,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Capabilities, Cell, CellStyle, CellWidth, Colors, Cursor, CursorShape, Dimensions, Frame,
        PALETTE_LEN, Rgb, Row, Screen, StyleColor, Underline,
    };

    fn frame() -> Frame {
        Frame {
            revision: 7,
            dimensions: Dimensions { cols: 1, rows: 1 },
            screen: Screen::Primary,
            title: "session".into(),
            working_directory: "/tmp".into(),
            capabilities: Capabilities {
                hyperlinks: true,
                kitty_graphics: false,
            },
            colors: Colors {
                background: Rgb::BLACK,
                foreground: Rgb {
                    r: 240,
                    g: 240,
                    b: 240,
                },
                cursor: None,
                palette: [Rgb::BLACK; PALETTE_LEN],
            },
            cursor: Cursor {
                visible: true,
                blinking: false,
                password_input: false,
                shape: CursorShape::Block,
                viewport: None,
            },
            rows: vec![Row {
                wrapped: false,
                wrap_continuation: false,
                kitty_virtual_placeholder: false,
                cells: vec![Cell {
                    width: CellWidth::Narrow,
                    style: CellStyle {
                        foreground: StyleColor::None,
                        background: StyleColor::None,
                        underline_color: StyleColor::None,
                        bold: false,
                        italic: false,
                        faint: false,
                        blink: false,
                        inverse: false,
                        invisible: false,
                        strikethrough: false,
                        overline: false,
                        selected: false,
                        protected: false,
                        underline: Underline::None,
                    },
                    text: "界".into(),
                    hyperlink: "https://example.test".into(),
                }],
            }],
        }
    }

    fn header(kind: u8, payload: usize) -> [u8; HEADER_BYTES] {
        let mut header = [0; HEADER_BYTES];
        header[..4].copy_from_slice(MAGIC);
        header[4..6].copy_from_slice(&VERSION.to_le_bytes());
        header[6] = kind;
        header[8..12].copy_from_slice(&(payload as u32).to_le_bytes());
        header
    }

    #[test]
    fn every_client_message_round_trips() {
        let messages = [
            ClientMessage::Hello {
                minimum_version: VERSION,
                maximum_version: VERSION,
            },
            ClientMessage::Key(KeyEvent {
                action: KeyAction::Repeat,
                key: PhysicalKey::A,
                modifiers: Modifiers::CTRL
                    .union(Modifiers::SHIFT)
                    .union(Modifiers::SHIFT_SIDE),
                consumed_modifiers: Modifiers::SHIFT,
                composing: true,
                text: Some("e\u{301}".into()),
                unshifted_codepoint: Some('e'),
            }),
            ClientMessage::Mouse(MouseEvent {
                action: MouseAction::Motion,
                button: Some(MouseButton::Eleven),
                modifiers: Modifiers::ALT,
                x: 12.5,
                y: 4.25,
            }),
            ClientMessage::Focus(FocusEvent::Gained),
            ClientMessage::Paste(b"first\nsecond\0\xff".to_vec()),
            ClientMessage::Resize(SurfaceSize {
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
            }),
        ];

        for message in messages {
            let encoded = encode_client_message(&message).unwrap();
            assert_eq!(client_message_len(&encoded).unwrap(), Some(encoded.len()));
            assert_eq!(decode_client_message(&encoded).unwrap(), message);
        }
    }

    #[test]
    fn every_server_message_round_trips() {
        let messages = [
            ServerMessage::Attached { version: VERSION },
            ServerMessage::Busy,
            ServerMessage::Incompatible {
                minimum_version: VERSION,
                maximum_version: VERSION,
            },
            ServerMessage::Frame(Box::new(frame())),
            ServerMessage::Accepted,
            ServerMessage::Failure(Failure {
                code: FailureCode::InvalidInput,
                detail: "bad key".into(),
            }),
            ServerMessage::Exited { code: 17 },
        ];

        for message in messages {
            let encoded = encode_server_message(&message).unwrap();
            assert_eq!(server_message_len(&encoded).unwrap(), Some(encoded.len()));
            assert_eq!(decode_server_message(&encoded).unwrap(), message);
        }
    }

    #[test]
    fn every_mouse_button_round_trips() {
        for button in [
            MouseButton::Unknown,
            MouseButton::Left,
            MouseButton::Middle,
            MouseButton::Right,
            MouseButton::Four,
            MouseButton::Five,
            MouseButton::Six,
            MouseButton::Seven,
            MouseButton::Eight,
            MouseButton::Nine,
            MouseButton::Ten,
            MouseButton::Eleven,
        ] {
            let message = ClientMessage::Mouse(MouseEvent {
                action: MouseAction::Press,
                button: Some(button),
                modifiers: Modifiers::empty(),
                x: 1.0,
                y: 2.0,
            });
            assert_eq!(
                decode_client_message(&encode_client_message(&message).unwrap()).unwrap(),
                message
            );
        }

        let invalid = ClientMessage::Mouse(MouseEvent {
            action: MouseAction::Motion,
            button: None,
            modifiers: Modifiers::SHIFT_SIDE,
            x: 1.0,
            y: 2.0,
        });
        assert_eq!(
            encode_client_message(&invalid),
            Err(Error::InvalidValue { field: "modifiers" })
        );
    }

    #[test]
    fn mouse_action_button_combinations_are_canonical() {
        let mouse = |action, button| {
            ClientMessage::Mouse(MouseEvent {
                action,
                button,
                modifiers: Modifiers::empty(),
                x: 1.0,
                y: 2.0,
            })
        };

        for action in [MouseAction::Press, MouseAction::Release] {
            assert_eq!(
                encode_client_message(&mouse(action, None)),
                Err(Error::InvalidValue {
                    field: "mouse button"
                })
            );
        }

        let mut invalid = encode_client_message(&mouse(MouseAction::Motion, None)).unwrap();
        invalid[HEADER_BYTES] = mouse_action_tag(MouseAction::Press);
        assert_eq!(
            decode_client_message(&invalid),
            Err(Error::InvalidValue {
                field: "mouse button"
            })
        );

        for action in [MouseAction::Release, MouseAction::Motion] {
            assert_eq!(
                encode_client_message(&mouse(action, Some(MouseButton::Four))),
                Err(Error::InvalidValue {
                    field: "mouse action"
                })
            );

            let mut invalid =
                encode_client_message(&mouse(MouseAction::Press, Some(MouseButton::Four))).unwrap();
            invalid[HEADER_BYTES] = mouse_action_tag(action);
            assert_eq!(
                decode_client_message(&invalid),
                Err(Error::InvalidValue {
                    field: "mouse action"
                })
            );
        }
    }

    #[test]
    fn framing_is_incremental_strict_and_bounded() {
        let encoded = encode_client_message(&ClientMessage::Hello {
            minimum_version: VERSION,
            maximum_version: VERSION,
        })
        .unwrap();
        for end in 0..HEADER_BYTES {
            assert_eq!(client_message_len(&encoded[..end]).unwrap(), None);
        }
        for end in 0..encoded.len() {
            assert_eq!(
                decode_client_message(&encoded[..end]),
                Err(Error::Truncated)
            );
        }

        let mut corrupt = encoded.clone();
        corrupt[0] ^= 1;
        assert_eq!(client_message_len(&corrupt), Err(Error::InvalidMagic));
        corrupt = encoded.clone();
        corrupt[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
        assert_eq!(
            client_message_len(&corrupt),
            Err(Error::UnsupportedVersion { version: 2 })
        );
        corrupt = encoded.clone();
        corrupt[7] = 1;
        assert_eq!(
            client_message_len(&corrupt),
            Err(Error::InvalidFlags { value: 1 })
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            decode_client_message(&trailing),
            Err(Error::TrailingBytes { count: 1 })
        );
    }

    #[test]
    fn declared_payloads_are_validated_from_the_header() {
        assert_eq!(
            client_message_len(&header(u8::MAX, 0)),
            Err(Error::InvalidTag {
                field: "client message",
                value: u8::MAX,
            })
        );
        assert_eq!(
            client_message_len(&header(CLIENT_PASTE, MAX_PASTE_BYTES + 1)),
            Err(Error::PayloadTooLarge {
                size: MAX_PASTE_BYTES + 1,
                maximum: MAX_PASTE_BYTES,
            })
        );
        assert_eq!(
            client_message_len(&header(CLIENT_HELLO, 3)),
            Err(Error::InvalidValue {
                field: "message payload length",
            })
        );
        assert_eq!(
            client_message_len(&header(SERVER_BUSY, 0)),
            Err(Error::InvalidTag {
                field: "client message",
                value: SERVER_BUSY,
            })
        );
        assert_eq!(
            server_message_len(&header(CLIENT_HELLO, 4)),
            Err(Error::InvalidTag {
                field: "server message",
                value: CLIENT_HELLO,
            })
        );
        assert_eq!(
            server_message_len(&header(SERVER_FAILURE, MAX_FAILURE_BYTES + 6)),
            Err(Error::PayloadTooLarge {
                size: MAX_FAILURE_BYTES + 6,
                maximum: MAX_FAILURE_BYTES + 5,
            })
        );
        let mut minimum_frame = frame();
        minimum_frame.title.clear();
        minimum_frame.working_directory.clear();
        minimum_frame.rows[0].cells[0].text.clear();
        minimum_frame.rows[0].cells[0].hyperlink.clear();
        let message = ServerMessage::Frame(Box::new(minimum_frame));
        let encoded = encode_server_message(&message).unwrap();
        assert_eq!(encoded.len(), HEADER_BYTES + MIN_FRAME_BYTES);
        assert_eq!(decode_server_message(&encoded), Ok(message));

        assert_eq!(
            server_message_len(&header(SERVER_FRAME, MIN_FRAME_BYTES - 1)),
            Err(Error::InvalidValue {
                field: "message payload length",
            })
        );
        assert_eq!(
            server_message_len(&header(SERVER_FRAME, MAX_PAYLOAD_BYTES + 1)),
            Err(Error::PayloadTooLarge {
                size: MAX_PAYLOAD_BYTES + 1,
                maximum: MAX_PAYLOAD_BYTES,
            })
        );
    }

    #[test]
    fn semantic_values_reject_invalid_states() {
        assert!(PhysicalKey::from_raw(PhysicalKey::MAX_RAW).is_some());
        assert!(PhysicalKey::from_raw(PhysicalKey::MAX_RAW + 1).is_none());
        assert!(Modifiers::from_bits(Modifiers::MASK).is_some());
        assert!(Modifiers::from_bits(Modifiers::MASK + 1).is_none());
        assert!(Modifiers::from_bits(Modifiers::SHIFT_SIDE.bits()).is_none());

        let oversized = ClientMessage::Paste(vec![0; MAX_PASTE_BYTES + 1]);
        assert_eq!(
            encode_client_message(&oversized),
            Err(Error::PayloadTooLarge {
                size: MAX_PASTE_BYTES + 1,
                maximum: MAX_PASTE_BYTES,
            })
        );

        let invalid = ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::A,
            modifiers: Modifiers::empty(),
            consumed_modifiers: Modifiers::CTRL,
            composing: false,
            text: Some("a".into()),
            unshifted_codepoint: Some('a'),
        });
        assert_eq!(
            encode_client_message(&invalid),
            Err(Error::ConsumedModifiersNotActive)
        );

        let invalid = ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::A,
            modifiers: Modifiers::SHIFT_SIDE,
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: Some("A".into()),
            unshifted_codepoint: Some('a'),
        });
        assert_eq!(
            encode_client_message(&invalid),
            Err(Error::InvalidValue { field: "modifiers" })
        );

        for text in ["\u{1b}", "\u{7f}", "\u{f700}"] {
            let invalid = ClientMessage::Key(KeyEvent {
                action: KeyAction::Press,
                key: PhysicalKey::UNIDENTIFIED,
                modifiers: Modifiers::empty(),
                consumed_modifiers: Modifiers::empty(),
                composing: false,
                text: Some(text.into()),
                unshifted_codepoint: None,
            });
            assert_eq!(
                encode_client_message(&invalid),
                Err(Error::InvalidValue { field: "key text" })
            );
        }

        let invalid = ClientMessage::Mouse(MouseEvent {
            action: MouseAction::Motion,
            button: None,
            modifiers: Modifiers::empty(),
            x: f32::NAN,
            y: 0.0,
        });
        assert_eq!(
            encode_client_message(&invalid),
            Err(Error::InvalidCoordinates)
        );

        let invalid = ClientMessage::Resize(SurfaceSize {
            cols: 100,
            rows: 40,
            screen_width: 899,
            screen_height: 720,
            cell_width: 9,
            cell_height: 18,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        });
        assert_eq!(
            encode_client_message(&invalid),
            Err(Error::InvalidSurfaceSize)
        );

        assert_eq!(
            encode_server_message(&ServerMessage::Attached {
                version: VERSION + 1,
            }),
            Err(Error::UnsupportedVersion {
                version: VERSION + 1,
            })
        );
    }

    #[test]
    fn malformed_typed_payloads_are_rejected() {
        let key = ClientMessage::Key(KeyEvent {
            action: KeyAction::Press,
            key: PhysicalKey::A,
            modifiers: Modifiers::SHIFT,
            consumed_modifiers: Modifiers::empty(),
            composing: false,
            text: Some("a".into()),
            unshifted_codepoint: Some('a'),
        });
        let encoded = encode_client_message(&key).unwrap();

        let mut corrupt = encoded.clone();
        corrupt[6] = u8::MAX;
        assert!(matches!(
            decode_client_message(&corrupt),
            Err(Error::InvalidTag {
                field: "client message",
                ..
            })
        ));

        let mut corrupt = encoded.clone();
        corrupt[15..17].copy_from_slice(&(Modifiers::MASK + 1).to_le_bytes());
        assert_eq!(
            decode_client_message(&corrupt),
            Err(Error::InvalidValue { field: "modifiers" })
        );

        let mut corrupt = encoded.clone();
        corrupt[24] = b'\x1b';
        assert_eq!(
            decode_client_message(&corrupt),
            Err(Error::InvalidValue { field: "key text" })
        );

        let mut corrupt = encoded.clone();
        corrupt[24] = 0xff;
        assert_eq!(
            decode_client_message(&corrupt),
            Err(Error::InvalidUtf8 { field: "key text" })
        );

        let mut corrupt = encoded;
        corrupt[25..29].copy_from_slice(&0xd800_u32.to_le_bytes());
        assert_eq!(
            decode_client_message(&corrupt),
            Err(Error::InvalidValue {
                field: "unshifted codepoint"
            })
        );

        let mut corrupt = encode_client_message(&ClientMessage::Resize(SurfaceSize {
            cols: 1,
            rows: 1,
            screen_width: 8,
            screen_height: 16,
            cell_width: 8,
            cell_height: 16,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        }))
        .unwrap();
        corrupt[12..14].fill(0);
        assert_eq!(
            decode_client_message(&corrupt),
            Err(Error::InvalidSurfaceSize)
        );

        let server = encode_server_message(&ServerMessage::Busy).unwrap();
        assert!(matches!(
            decode_client_message(&server),
            Err(Error::InvalidTag {
                field: "client message",
                ..
            })
        ));
    }
}
