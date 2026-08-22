use crate::Result;
use orbit_protocol::{
    Frame, MAX_FRAME_BYTES,
    session::{self, ClientMessage, Failure, FailureCode, ServerMessage, WheelOutcome},
};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    time::{Duration, Instant},
};

const NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_OUTPUT_BYTES: usize = 2 * (MAX_FRAME_BYTES + session::HEADER_BYTES) + 4096;

pub(crate) enum Incoming {
    Attached,
    Message(ClientMessage),
    Pending,
    Disconnect,
}

pub(crate) struct Client {
    stream: UnixStream,
    input: Vec<u8>,
    output: OutputQueue,
    close_after_flush: bool,
    negotiation_deadline: Option<Instant>,
    initial_frame_pending: bool,
}

impl Client {
    pub(crate) fn accepts_output(&self) -> bool {
        self.negotiation_deadline.is_none() && !self.close_after_flush
    }

    pub(crate) fn poll_stream(&self) -> Option<&UnixStream> {
        (!self.close_after_flush).then_some(&self.stream)
    }

    pub(crate) fn is_expired(&self, now: Instant) -> bool {
        self.negotiation_deadline
            .is_some_and(|deadline| now >= deadline)
    }

    pub(crate) fn read_ready(&mut self) -> Result<bool> {
        if self.close_after_flush {
            return Ok(true);
        }
        let mut bytes = [0; 1024];
        let read = match self.stream.read(&mut bytes) {
            Ok(0) => return Ok(false),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => return Ok(true),
            Err(error) if is_disconnect(&error) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        self.input.extend_from_slice(&bytes[..read]);
        Ok(true)
    }

    pub(crate) fn next_incoming(&mut self) -> Result<Incoming> {
        let length = match session::client_message_len(&self.input) {
            Ok(Some(length)) if self.input.len() >= length => length,
            Ok(_) => return Ok(Incoming::Pending),
            Err(session::Error::UnsupportedVersion { .. }) => return Ok(Incoming::Disconnect),
            Err(error) => return self.reject_protocol(error.to_string()),
        };
        let message = match session::decode_client_message(&self.input[..length]) {
            Ok(message) => message,
            Err(error) => return self.reject_protocol(error.to_string()),
        };
        drop(self.input.drain(..length));

        if self.negotiation_deadline.is_some() {
            return match message {
                ClientMessage::Hello => {
                    self.negotiation_deadline = None;
                    if !self.push_message(&ServerMessage::Attached)? {
                        return Ok(Incoming::Disconnect);
                    }
                    self.initial_frame_pending = true;
                    Ok(Incoming::Attached)
                }
                _ => self.reject_protocol("first client message must be Hello".into()),
            };
        }
        if matches!(message, ClientMessage::Hello) {
            return self.reject_protocol("Hello may only be sent once".into());
        }
        Ok(Incoming::Message(message))
    }

    fn reject_protocol(&mut self, detail: String) -> Result<Incoming> {
        let queued = self.fail(FailureCode::Protocol, detail)?;
        self.close_when_flushed();
        Ok(if queued {
            Incoming::Pending
        } else {
            Incoming::Disconnect
        })
    }

    pub(crate) fn can_push_result_frame(&self) -> bool {
        self.output.can_push_result_frame()
    }

    pub(crate) fn can_push_frame_message(&self) -> bool {
        self.output.can_push_frame_message()
    }

    pub(crate) fn can_push_message(&self, message: &ServerMessage) -> Result<bool> {
        self.output.can_push_message(message)
    }

    pub(crate) fn push_message(&mut self, message: &ServerMessage) -> Result<bool> {
        self.output.push_message(message)
    }

    pub(crate) fn push_frame(&mut self, frame: Frame) -> Result<bool> {
        let queued = if self.initial_frame_pending {
            self.output.push_initial_frame(frame)
        } else {
            self.output.push_frame(frame)
        }?;
        if queued {
            self.initial_frame_pending = false;
        }
        Ok(queued)
    }

    pub(crate) fn fail(&mut self, code: FailureCode, mut detail: String) -> Result<bool> {
        while detail.len() > session::MAX_FAILURE_BYTES {
            detail.pop();
        }
        self.push_message(&ServerMessage::Failure(Failure { code, detail }))
    }

    pub(crate) fn close_when_flushed(&mut self) {
        self.input = Vec::new();
        self.close_after_flush = true;
    }

    pub(crate) fn is_closing(&self) -> bool {
        self.close_after_flush
    }

    pub(crate) fn finish_session(&mut self, code: i32) -> Result {
        if self.negotiation_deadline.is_none() {
            if !self.close_after_flush {
                let _ = self.push_message(&ServerMessage::Exited { code })?;
            }
            let _ = self.output.flush(&mut self.stream);
        }
        Ok(())
    }

    pub(crate) fn flush(&mut self) -> io::Result<bool> {
        Ok(self.output.flush(&mut self.stream)?
            && !(self.close_after_flush && self.output.is_empty()))
    }

    #[cfg(test)]
    pub(crate) fn test_pair(negotiation_deadline: Option<Instant>) -> Result<(Self, UnixStream)> {
        let (stream, peer) = UnixStream::pair()?;
        stream.set_nonblocking(true)?;
        Ok((
            Self {
                stream,
                input: Vec::new(),
                output: OutputQueue::default(),
                close_after_flush: false,
                negotiation_deadline,
                initial_frame_pending: false,
            },
            peer,
        ))
    }

    #[cfg(test)]
    pub(crate) fn initial_frame_pending(&self) -> bool {
        self.initial_frame_pending
    }

    #[cfg(test)]
    pub(crate) fn output_is_empty(&self) -> bool {
        self.output.is_empty()
    }
}

pub(crate) fn accept(listener: &UnixListener, active: &mut Option<Client>) -> Result {
    loop {
        match listener.accept() {
            Ok((mut stream, _)) if active.is_some() => {
                let busy = session::encode_server_message(&ServerMessage::Busy)?;
                let _ = stream.write_all(&busy);
            }
            Ok((stream, _)) => {
                stream.set_nonblocking(true)?;
                *active = Some(Client {
                    stream,
                    input: Vec::new(),
                    output: OutputQueue::default(),
                    close_after_flush: false,
                    negotiation_deadline: Some(Instant::now() + NEGOTIATION_TIMEOUT),
                    initial_frame_pending: false,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

pub(crate) fn is_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
    )
}

struct Message {
    bytes: Vec<u8>,
    class: MessageClass,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MessageClass {
    Ordered,
    Preview,
    Frame,
}

#[derive(Default)]
struct OutputQueue {
    messages: VecDeque<Message>,
    offset: usize,
    bytes: usize,
}

impl OutputQueue {
    fn can_push_result_frame(&self) -> bool {
        self.bytes
            .saturating_add(MAX_FRAME_BYTES + 2 * session::HEADER_BYTES)
            <= MAX_OUTPUT_BYTES
    }

    fn can_push_frame_message(&self) -> bool {
        self.bytes
            .saturating_sub(self.replaceable_suffix(MessageClass::Frame).1)
            .saturating_add(MAX_FRAME_BYTES + session::HEADER_BYTES)
            <= MAX_OUTPUT_BYTES
    }

    fn can_push_message(&self, message: &ServerMessage) -> Result<bool> {
        Ok(self
            .bytes
            .saturating_add(session::encode_server_message(message)?.len())
            <= MAX_OUTPUT_BYTES)
    }

    fn push_message(&mut self, message: &ServerMessage) -> Result<bool> {
        let class = match message {
            ServerMessage::VerticalPreview(_) => MessageClass::Preview,
            ServerMessage::WheelOutcome(WheelOutcome::Viewport { .. }) => MessageClass::Frame,
            _ => MessageClass::Ordered,
        };
        Ok(self.push_replaceable(session::encode_server_message(message)?, class))
    }

    fn push_initial_frame(&mut self, frame: Frame) -> Result<bool> {
        Ok(self.push(
            session::encode_server_message(&ServerMessage::Frame(Box::new(frame)))?,
            MessageClass::Ordered,
        ))
    }

    fn push_frame(&mut self, frame: Frame) -> Result<bool> {
        let message = session::encode_server_message(&ServerMessage::Frame(Box::new(frame)))?;
        Ok(self.push_replaceable(message, MessageClass::Frame))
    }

    fn push_replaceable(&mut self, message: Vec<u8>, class: MessageClass) -> bool {
        let (remove, removed_bytes) = self.replaceable_suffix(class);
        if self
            .bytes
            .saturating_sub(removed_bytes)
            .saturating_add(message.len())
            > MAX_OUTPUT_BYTES
        {
            return false;
        }
        for _ in 0..remove {
            let previous = self.messages.pop_back().expect("back existed");
            self.bytes -= previous.bytes.len();
        }
        self.push(message, class)
    }

    fn replaceable_suffix(&self, class: MessageClass) -> (usize, usize) {
        let replaceable = |back: &Message| match class {
            MessageClass::Ordered => false,
            MessageClass::Preview => back.class == MessageClass::Preview,
            MessageClass::Frame => {
                matches!(back.class, MessageClass::Preview | MessageClass::Frame)
            }
        };
        let mut remove = 0;
        let mut removed_bytes = 0usize;
        for (index, back) in self.messages.iter().enumerate().rev() {
            if !replaceable(back) || index == 0 && self.offset != 0 {
                break;
            }
            remove += 1;
            removed_bytes += back.bytes.len();
            if class == MessageClass::Preview {
                break;
            }
        }
        (remove, removed_bytes)
    }

    fn push(&mut self, bytes: Vec<u8>, class: MessageClass) -> bool {
        if self.bytes.saturating_add(bytes.len()) > MAX_OUTPUT_BYTES {
            return false;
        }
        self.bytes += bytes.len();
        self.messages.push_back(Message { bytes, class });
        true
    }

    fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn flush(&mut self, stream: &mut UnixStream) -> io::Result<bool> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_frames_keep_the_initial_and_only_the_latest_revision() {
        let mut output = OutputQueue::default();
        assert!(output.push(vec![0], MessageClass::Ordered));
        assert!(output.push_replaceable(vec![1], MessageClass::Frame));
        assert!(output.push_replaceable(vec![2], MessageClass::Preview));
        assert!(output.push_replaceable(vec![3], MessageClass::Preview));

        assert_eq!(output.messages.len(), 3);
        assert!(output.messages.front().unwrap().bytes.ends_with(&[0]));
        assert!(output.messages.back().unwrap().bytes.ends_with(&[3]));

        assert!(output.push_replaceable(vec![4], MessageClass::Frame));
        assert_eq!(output.messages.len(), 2);
        assert!(output.messages.back().unwrap().bytes.ends_with(&[4]));

        assert!(output.push_replaceable(vec![5], MessageClass::Preview));
        assert_eq!(output.messages.len(), 3);
        assert!(output.messages.back().unwrap().bytes.ends_with(&[5]));

        let mut full = OutputQueue::default();
        assert!(full.push(
            vec![0; MAX_FRAME_BYTES + session::HEADER_BYTES],
            MessageClass::Ordered
        ));
        assert!(full.push_replaceable(
            vec![1; MAX_FRAME_BYTES + session::HEADER_BYTES],
            MessageClass::Frame
        ));
        assert!(full.can_push_frame_message());
    }

    #[test]
    fn fatal_decode_releases_client_input_storage() -> Result {
        let (mut client, mut peer) = Client::test_pair(None)?;
        let mut message =
            session::encode_client_message(&ClientMessage::Mouse(session::MouseEvent {
                action: session::MouseAction::Press,
                button: Some(session::MouseButton::Left),
                modifiers: session::Modifiers::empty(),
                x: 1.0,
                y: 1.0,
            }))?;
        message[session::HEADER_BYTES + 4..session::HEADER_BYTES + 8]
            .copy_from_slice(&f32::MAX.to_bits().to_le_bytes());
        peer.write_all(&message)?;

        assert!(client.read_ready()?);
        assert!(matches!(client.next_incoming()?, Incoming::Pending));
        assert!(client.close_after_flush);
        assert!(client.input.is_empty());
        assert_eq!(client.input.capacity(), 0);
        let _ = client.flush()?;
        assert_eq!(
            crate::read_server_message(&mut peer)?,
            Some(ServerMessage::Failure(Failure {
                code: FailureCode::Protocol,
                detail: "invalid mouse coordinates".into(),
            }))
        );
        Ok(())
    }

    #[test]
    fn terminal_closing_client_finishes_with_queued_failure_only() -> Result {
        let (mut client, mut peer) = Client::test_pair(None)?;
        let failure = ServerMessage::Failure(Failure {
            code: FailureCode::Protocol,
            detail: "terminal".into(),
        });
        let expected = session::encode_server_message(&failure)?;
        assert!(client.push_message(&failure)?);
        client.close_when_flushed();

        client.finish_session(17)?;
        assert!(client.output.is_empty());
        drop(client);

        let mut actual = Vec::new();
        peer.read_to_end(&mut actual)?;
        assert_eq!(actual, expected);
        Ok(())
    }
}
