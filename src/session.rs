use std::net::TcpStream;
use std::time::Duration;

use crate::auth::{HostKeys, current_hostname, modern_auth_tlv};
use crate::codec::{
    Frame, decode_session_handshake, encode_session_handshake, read_frame, write_frame,
};
use crate::error::{HdcError, Result};
use crate::protocol::{
    AuthType, CHANNEL_COMMAND, CHANNEL_HANDSHAKE, DAEMON_AUTH_SUCCESS, DAEMON_UNAUTHORIZED,
    HdcCommand, MIN_AUTH_VERSION, MessageLevel, SessionHandshake, TAG_DAEMON_AUTHSTATUS,
    TAG_EMGMSG, parse_tlv_map,
};
use crate::types::{CommandStatus, DriverMessage, DriverMessageLevel, ShellResult};

#[derive(Debug, Clone)]
pub(crate) struct SessionOptions {
    pub key_dir: std::path::PathBuf,
    pub connect_key: String,
    pub version: String,
    pub timeout: Duration,
}

pub(crate) struct Session {
    stream: TcpStream,
    keys: HostKeys,
    session_id: u32,
    connect_key: String,
    version: String,
    hostname: String,
    authenticated: bool,
    command_channel_open: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum HandshakeStatus {
    Authorized,
    Pending(String),
    Rejected(String),
}

impl Session {
    pub(crate) fn connect(addr: &str, options: SessionOptions) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(options.timeout))?;
        stream.set_write_timeout(Some(options.timeout))?;
        let _ = stream.set_nodelay(true);

        Ok(Self {
            stream,
            keys: HostKeys::load_or_create(&options.key_dir)?,
            session_id: generate_session_id(),
            connect_key: options.connect_key,
            version: options.version,
            hostname: current_hostname(),
            authenticated: false,
            command_channel_open: false,
        })
    }

    pub(crate) fn authenticate(&mut self) -> Result<()> {
        self.send_handshake(AuthType::None, modern_auth_tlv())?;

        let mut saw_auth_ok = false;
        let mut pending_message: Option<String> = None;
        loop {
            let frame = match self.recv_frame() {
                Ok(frame) => frame,
                Err(HdcError::Io(error))
                    if pending_message.is_some()
                        && matches!(
                            error.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        ) =>
                {
                    let message = pending_message.take().unwrap_or_else(|| {
                        "timed out waiting for device authorization".to_string()
                    });
                    return Err(HdcError::protocol(format!(
                        "timed out waiting for device authorization: {message}"
                    )));
                }
                Err(error) => return Err(error),
            };
            match frame.command {
                HdcCommand::KernelHandshake => {
                    let handshake = decode_session_handshake(&frame.payload)?;
                    match handshake.auth_type {
                        AuthType::PublicKey => {
                            let tlv_map = parse_tlv_map(&handshake.buf)?;
                            if tlv_map
                                .get(crate::protocol::TAG_AUTH_TYPE)
                                .map(String::as_str)
                                != Some(crate::protocol::AUTH_VERIFY_RSA_3072_SHA512)
                            {
                                return Err(HdcError::protocol(
                                    "daemon did not advertise RSA_3072_SHA512 auth",
                                ));
                            }
                            let payload = self.keys.public_key_payload(&self.hostname)?;
                            self.send_handshake(AuthType::PublicKey, payload)?;
                            pending_message = None;
                        }
                        AuthType::Signature => {
                            let signature = self
                                .keys
                                .sign_token_pss_sha512_base64(handshake.buf.as_bytes())?;
                            self.send_handshake(AuthType::Signature, signature)?;
                            pending_message = None;
                        }
                        AuthType::Ok => match interpret_auth_ok(&handshake)? {
                            HandshakeStatus::Authorized => {
                                saw_auth_ok = true;
                                pending_message = None;
                            }
                            HandshakeStatus::Pending(message) => {
                                pending_message = Some(message);
                            }
                            HandshakeStatus::Rejected(message) => {
                                return Err(HdcError::protocol(message));
                            }
                        },
                        AuthType::Fail => {
                            return Err(HdcError::protocol(if handshake.buf.is_empty() {
                                "daemon rejected authentication".to_string()
                            } else {
                                handshake.buf
                            }));
                        }
                        other => {
                            return Err(HdcError::protocol(format!(
                                "unexpected auth state during handshake: {:?}",
                                other
                            )));
                        }
                    }
                }
                HdcCommand::KernelChannelClose if frame.channel_id == CHANNEL_HANDSHAKE => {
                    self.acknowledge_channel_close(frame.channel_id, &frame.payload)?;
                    if !saw_auth_ok {
                        if pending_message.is_some() {
                            continue;
                        }
                        return Err(HdcError::protocol(
                            "handshake channel closed before authentication completed",
                        ));
                    }
                    self.authenticated = true;
                    return Ok(());
                }
                HdcCommand::KernelEcho | HdcCommand::KernelEchoRaw | HdcCommand::HeartbeatMsg => {}
                other => {
                    return Err(HdcError::protocol(format!(
                        "unexpected command during handshake: {:?}",
                        other
                    )));
                }
            }
        }
    }

    pub(crate) fn exec_shell(&mut self, command: &str) -> Result<ShellResult> {
        if !self.authenticated {
            return Err(HdcError::protocol("session is not authenticated"));
        }

        self.command_channel_open = true;
        self.send_frame(Frame {
            channel_id: CHANNEL_COMMAND,
            command: HdcCommand::UnityExecute,
            payload: command.as_bytes().to_vec(),
        })?;

        let mut stdout = Vec::new();
        let mut messages = Vec::new();
        let mut status = CommandStatus::Ok;

        loop {
            let frame = self.recv_frame()?;
            if frame.channel_id != CHANNEL_COMMAND {
                continue;
            }
            match frame.command {
                HdcCommand::KernelEchoRaw => stdout.extend_from_slice(&frame.payload),
                HdcCommand::KernelEcho => {
                    if let Some(message) = parse_driver_message(&frame.payload)? {
                        if message.level == DriverMessageLevel::Fail {
                            status = CommandStatus::FailedHint;
                        }
                        messages.push(message);
                    }
                }
                HdcCommand::KernelChannelClose => {
                    self.command_channel_open = false;
                    return Ok(ShellResult {
                        stdout,
                        messages,
                        status,
                    });
                }
                HdcCommand::HeartbeatMsg => {}
                other => {
                    return Err(HdcError::protocol(format!(
                        "unexpected command on command channel: {:?}",
                        other
                    )));
                }
            }
        }
    }

    pub(crate) fn close_active_command_channel(&mut self) -> Result<()> {
        if !self.command_channel_open {
            return Ok(());
        }

        self.send_frame(Frame {
            channel_id: CHANNEL_COMMAND,
            command: HdcCommand::KernelChannelClose,
            payload: vec![1],
        })?;

        loop {
            let frame = self.recv_frame()?;
            if frame.channel_id != CHANNEL_COMMAND {
                continue;
            }
            match frame.command {
                HdcCommand::KernelChannelClose => {
                    self.command_channel_open = false;
                    return Ok(());
                }
                HdcCommand::KernelEcho | HdcCommand::KernelEchoRaw | HdcCommand::HeartbeatMsg => {}
                other => {
                    return Err(HdcError::protocol(format!(
                        "unexpected command while closing channel: {:?}",
                        other
                    )));
                }
            }
        }
    }

    fn send_handshake(&mut self, auth_type: AuthType, buf: String) -> Result<()> {
        let handshake = SessionHandshake {
            banner: crate::protocol::HANDSHAKE_MESSAGE.to_string(),
            auth_type,
            session_id: self.session_id,
            connect_key: self.connect_key.clone(),
            buf,
            version: self.version.clone(),
        };
        self.send_frame(Frame {
            channel_id: CHANNEL_HANDSHAKE,
            command: HdcCommand::KernelHandshake,
            payload: encode_session_handshake(&handshake),
        })
    }

    fn send_frame(&mut self, frame: Frame) -> Result<()> {
        write_frame(&mut self.stream, &frame)
    }

    fn recv_frame(&mut self) -> Result<Frame> {
        read_frame(&mut self.stream)
    }

    fn acknowledge_channel_close(&mut self, channel_id: u32, payload: &[u8]) -> Result<()> {
        let Some(&count) = payload.first() else {
            return Ok(());
        };
        if count == 0 {
            return Ok(());
        }
        self.send_frame(Frame {
            channel_id,
            command: HdcCommand::KernelChannelClose,
            payload: vec![count - 1],
        })
    }
}

fn parse_driver_message(payload: &[u8]) -> Result<Option<DriverMessage>> {
    if payload.is_empty() {
        return Ok(None);
    }

    let level = DriverMessageLevel::from_protocol(MessageLevel::from_u8(payload[0]));
    let text = String::from_utf8(payload[1..].to_vec())?;
    Ok(Some(DriverMessage { level, text }))
}

fn generate_session_id() -> u32 {
    let time_bits = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    let mixed = time_bits ^ std::process::id();
    if mixed == 0 { 1 } else { mixed }
}

fn interpret_auth_ok(handshake: &SessionHandshake) -> Result<HandshakeStatus> {
    if handshake.version.as_str() < MIN_AUTH_VERSION {
        return Ok(HandshakeStatus::Authorized);
    }

    let tlv_map = parse_tlv_map(&handshake.buf)?;
    let status = tlv_map
        .get(TAG_DAEMON_AUTHSTATUS)
        .cloned()
        .unwrap_or_default();
    let message = tlv_map
        .get(TAG_EMGMSG)
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "authentication rejected by daemon".to_string());

    if status == DAEMON_AUTH_SUCCESS {
        return Ok(HandshakeStatus::Authorized);
    }
    if status == DAEMON_UNAUTHORIZED {
        return Ok(HandshakeStatus::Pending(message));
    }
    Ok(HandshakeStatus::Rejected(message))
}

#[cfg(test)]
mod tests {
    use super::{HandshakeStatus, interpret_auth_ok, parse_driver_message};
    use crate::protocol::{
        AuthType, DEFAULT_VERSION, SessionHandshake, TAG_DAEMON_AUTHSTATUS, TAG_EMGMSG, append_tlv,
    };
    use crate::types::DriverMessageLevel;

    fn auth_ok_handshake(status: &str, message: &str) -> SessionHandshake {
        let mut buf = String::new();
        append_tlv(&mut buf, TAG_EMGMSG, message);
        append_tlv(&mut buf, TAG_DAEMON_AUTHSTATUS, status);
        SessionHandshake {
            banner: crate::protocol::HANDSHAKE_MESSAGE.to_string(),
            auth_type: AuthType::Ok,
            session_id: 1,
            connect_key: "192.168.8.43:35319".to_string(),
            buf,
            version: DEFAULT_VERSION.to_string(),
        }
    }

    #[test]
    fn auth_ok_with_success_status_is_authorized() {
        let handshake = auth_ok_handshake("SUCCESS", "");

        let status = interpret_auth_ok(&handshake).unwrap();

        assert!(matches!(status, HandshakeStatus::Authorized));
    }

    #[test]
    fn auth_ok_with_unauthorized_status_is_pending() {
        let handshake = auth_ok_handshake(
            "DAEMON_UNAUTH",
            "[E000002]:The device unauthorized.\r\nPlease check for a confirmation dialog on your device.",
        );

        let status = interpret_auth_ok(&handshake).unwrap();

        assert!(matches!(
            status,
            HandshakeStatus::Pending(message) if message.contains("confirmation dialog")
        ));
    }

    #[test]
    fn parse_driver_message_extracts_level_and_text() {
        let message = parse_driver_message(&[0, b'f', b'a', b'i', b'l'])
            .unwrap()
            .unwrap();

        assert_eq!(message.level, DriverMessageLevel::Fail);
        assert_eq!(message.text, "fail");
    }
}
