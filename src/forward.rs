use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::error::{HdcError, Result};
use crate::protocol::{CHANNEL_FORWARD, HdcCommand};
use crate::session::{Session, SessionOptions};

pub struct TcpForwardHandle {
    stop: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

struct ForwardConnection {
    stream: TcpStream,
    active: bool,
    pending: VecDeque<Vec<u8>>,
}

impl TcpForwardHandle {
    pub(crate) fn spawn(
        addr: String,
        mut options: SessionOptions,
        local_port: u16,
        remote_port: u16,
    ) -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", local_port))?;
        listener.set_nonblocking(true)?;

        options.timeout = options.timeout.min(Duration::from_millis(100));
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let join_handle = thread::spawn(move || {
            let result = run_forward_loop(listener, stop_thread, &addr, options, remote_port);
            if let Err(error) = result {
                eprintln!("tcp forward listener failed: {error}");
            }
        });

        Ok(Self {
            stop,
            join_handle: Some(join_handle),
        })
    }
}

impl Drop for TcpForwardHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

pub(crate) fn send_file_via_shell(
    session: &mut Session,
    local_path: &Path,
    remote_path: &str,
) -> Result<()> {
    let bytes = std::fs::read(local_path)?;
    let encoded = openssl::base64::encode_block(&bytes);
    let staging_path = format!("{remote_path}.b64");
    let escaped_staging = shell_escape(&staging_path);
    let escaped_remote = shell_escape(remote_path);

    session.exec_checked(&format!("rm -f {escaped_staging} {escaped_remote}"))?;
    for chunk in encoded.as_bytes().chunks(3072) {
        let chunk = std::str::from_utf8(chunk)
            .map_err(|_| HdcError::protocol("base64 chunk is not valid UTF-8"))?;
        session.exec_checked(&format!(
            "printf %s {} >> {}",
            shell_escape(chunk),
            escaped_staging
        ))?;
    }
    session.exec_checked(&format!(
        "(base64 -d {staging} > {remote} 2>/dev/null || toybox base64 -d {staging} > {remote} 2>/dev/null); ret=$?; rm -f {staging}; exit $ret",
        staging = escaped_staging,
        remote = escaped_remote
    ))?;
    Ok(())
}

fn run_forward_loop(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    addr: &str,
    options: SessionOptions,
    remote_port: u16,
) -> Result<()> {
    let mut session = Session::connect(addr, options)?;
    session.authenticate()?;

    let cid = 1_u32;
    let mut current: Option<ForwardConnection> = None;
    let remote_spec = format!("tcp:{remote_port}");
    let mut ready = false;

    session.send_custom(
        CHANNEL_FORWARD,
        HdcCommand::KernelWakeupSlavetask,
        Vec::new(),
    )?;
    session.send_custom(
        CHANNEL_FORWARD,
        HdcCommand::ForwardCheck,
        build_forward_control_payload(cid, &remote_spec),
    )?;

    while !stop.load(Ordering::Relaxed) {
        if ready && current.is_none() {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true)?;
                    session.send_custom(
                        CHANNEL_FORWARD,
                        HdcCommand::ForwardActiveSlave,
                        build_forward_control_payload(cid, &remote_spec),
                    )?;
                    current = Some(ForwardConnection {
                        stream,
                        active: false,
                        pending: VecDeque::new(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error.into()),
            }
        }

        if let Some(connection) = current.as_mut() {
            match read_local_client(connection) {
                Ok(Some(data)) => {
                    if connection.active {
                        session.send_custom(
                            CHANNEL_FORWARD,
                            HdcCommand::ForwardData,
                            build_forward_data_payload(cid, &data),
                        )?;
                    } else {
                        connection.pending.push_back(data);
                    }
                }
                Ok(None) => {}
                Err(ForwardReadState::Closed) => {
                    let _ = session.send_custom(
                        CHANNEL_FORWARD,
                        HdcCommand::ForwardFreeContext,
                        build_forward_free_payload(cid),
                    );
                    let _ = connection.stream.shutdown(Shutdown::Both);
                    current = None;
                }
                Err(ForwardReadState::Io(error)) => return Err(error.into()),
            }
        }

        if let Some(frame) = session.try_recv_custom()? {
            handle_forward_frame(frame, &mut session, cid, &mut ready, &mut current)?;
        } else {
            thread::sleep(Duration::from_millis(10));
        }
    }

    if let Some(connection) = current.as_mut() {
        let _ = session.send_custom(
            CHANNEL_FORWARD,
            HdcCommand::ForwardFreeContext,
            build_forward_free_payload(cid),
        );
        let _ = connection.stream.shutdown(Shutdown::Both);
    }
    Ok(())
}

fn handle_forward_frame(
    frame: crate::codec::Frame,
    session: &mut Session,
    cid: u32,
    ready: &mut bool,
    current: &mut Option<ForwardConnection>,
) -> Result<()> {
    if frame.channel_id != CHANNEL_FORWARD {
        return Ok(());
    }
    match frame.command {
        HdcCommand::ForwardCheckResult => {
            if parse_forward_cid(&frame.payload)? == cid && frame.payload.len() >= 5 {
                *ready = true;
            } else {
                return Err(HdcError::protocol("forward check failed"));
            }
        }
        HdcCommand::ForwardActiveMaster => {
            if parse_forward_cid(&frame.payload)? == cid {
                if let Some(connection) = current.as_mut() {
                    connection.active = true;
                    while let Some(data) = connection.pending.pop_front() {
                        session.send_custom(
                            CHANNEL_FORWARD,
                            HdcCommand::ForwardData,
                            build_forward_data_payload(cid, &data),
                        )?;
                    }
                }
            }
        }
        HdcCommand::ForwardData => {
            let (data_cid, data) = parse_forward_data(&frame.payload)?;
            if data_cid == cid {
                if let Some(connection) = current.as_mut() {
                    write_local_client(&mut connection.stream, data)?;
                }
            }
        }
        HdcCommand::ForwardFreeContext => {
            if parse_forward_cid(&frame.payload)? == cid {
                if let Some(connection) = current.as_mut() {
                    let _ = connection.stream.shutdown(Shutdown::Both);
                    *current = None;
                }
            }
        }
        HdcCommand::KernelEcho => {
            if let Some(message) = crate::session::parse_driver_message(&frame.payload)? {
                if message.level == crate::types::DriverMessageLevel::Fail {
                    return Err(HdcError::protocol(message.text));
                }
            }
        }
        HdcCommand::HeartbeatMsg => {}
        other => {
            return Err(HdcError::protocol(format!(
                "unexpected forward command: {:?}",
                other
            )));
        }
    }
    Ok(())
}

enum ForwardReadState {
    Closed,
    Io(std::io::Error),
}

fn read_local_client(connection: &mut ForwardConnection) -> std::result::Result<Option<Vec<u8>>, ForwardReadState> {
    let mut buffer = [0_u8; 8192];
    match connection.stream.read(&mut buffer) {
        Ok(0) => Err(ForwardReadState::Closed),
        Ok(size) => Ok(Some(buffer[..size].to_vec())),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(ForwardReadState::Io(error)),
    }
}

fn write_local_client(stream: &mut TcpStream, data: Vec<u8>) -> Result<()> {
    stream.write_all(&data)?;
    Ok(())
}

fn build_forward_control_payload(cid: u32, remote_spec: &str) -> Vec<u8> {
    let mut payload = vec![0_u8; 13 + remote_spec.len()];
    payload[..4].copy_from_slice(&cid.to_be_bytes());
    payload[12..12 + remote_spec.len()].copy_from_slice(remote_spec.as_bytes());
    payload
}

fn build_forward_data_payload(cid: u32, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + data.len());
    payload.extend_from_slice(&cid.to_be_bytes());
    payload.extend_from_slice(data);
    payload
}

fn build_forward_free_payload(cid: u32) -> Vec<u8> {
    cid.to_be_bytes().to_vec()
}

fn parse_forward_cid(payload: &[u8]) -> Result<u32> {
    if payload.len() < 4 {
        return Err(HdcError::protocol("forward payload missing cid"));
    }
    let mut cid = [0_u8; 4];
    cid.copy_from_slice(&payload[..4]);
    Ok(u32::from_be_bytes(cid))
}

fn parse_forward_data(payload: &[u8]) -> Result<(u32, Vec<u8>)> {
    let cid = parse_forward_cid(payload)?;
    Ok((cid, payload[4..].to_vec()))
}

fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::{
        build_forward_control_payload, build_forward_data_payload, parse_forward_cid,
        parse_forward_data, shell_escape,
    };

    #[test]
    fn shell_escape_handles_single_quotes() {
        assert_eq!(shell_escape("it's"), "'it'\"'\"'s'");
    }

    #[test]
    fn forward_control_payload_matches_daemon_layout() {
        let payload = build_forward_control_payload(0x01020304, "tcp:8012");
        assert_eq!(&payload[..4], &[1, 2, 3, 4]);
        assert_eq!(&payload[12..20], b"tcp:8012");
        assert_eq!(payload[20], 0);
    }

    #[test]
    fn forward_data_roundtrip_preserves_cid_and_bytes() {
        let payload = build_forward_data_payload(7, b"abc");
        let (cid, data) = parse_forward_data(&payload).unwrap();
        assert_eq!(cid, 7);
        assert_eq!(data, b"abc");
    }

    #[test]
    fn parse_forward_cid_rejects_short_payloads() {
        assert!(parse_forward_cid(&[1, 2, 3]).is_err());
    }
}
