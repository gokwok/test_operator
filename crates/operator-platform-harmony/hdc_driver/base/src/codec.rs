use std::io::{Read, Write};

use crate::error::{HdcError, Result};
use crate::protocol::{
    AuthType, HdcCommand, PACKET_FLAG, PAYLOAD_HEAD_SIZE, PAYLOAD_VCODE, PayloadProtect,
    SessionHandshake, VER_PROTOCOL,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub channel_id: u32,
    pub command: HdcCommand,
    pub payload: Vec<u8>,
}

pub fn encode_session_handshake(value: &SessionHandshake) -> Vec<u8> {
    let mut out = Vec::new();
    encode_length_field(1, value.banner.as_bytes(), &mut out);
    encode_u8_field(2, value.auth_type.as_u8(), &mut out);
    encode_u32_field(3, value.session_id, &mut out);
    encode_length_field(4, value.connect_key.as_bytes(), &mut out);
    encode_length_field(5, value.buf.as_bytes(), &mut out);
    encode_length_field(6, value.version.as_bytes(), &mut out);
    out
}

pub fn decode_session_handshake(bytes: &[u8]) -> Result<SessionHandshake> {
    let mut index = 0;
    let mut handshake = SessionHandshake::default();
    while index < bytes.len() {
        let tag_key = decode_varint(bytes, &mut index)? as u32;
        let field_number = tag_key >> 3;
        let wire_type = (tag_key & 0x07) as u8;
        match (field_number, wire_type) {
            (1, 2) => handshake.banner = decode_length_string(bytes, &mut index)?,
            (2, 0) => {
                handshake.auth_type = AuthType::from_u8(decode_varint(bytes, &mut index)? as u8)
            }
            (3, 0) => handshake.session_id = decode_varint(bytes, &mut index)? as u32,
            (4, 2) => handshake.connect_key = decode_length_string(bytes, &mut index)?,
            (5, 2) => handshake.buf = decode_length_string(bytes, &mut index)?,
            (6, 2) => handshake.version = decode_length_string(bytes, &mut index)?,
            _ => skip_field(bytes, &mut index, wire_type)?,
        }
    }
    Ok(handshake)
}

pub fn encode_payload_protect(value: &PayloadProtect) -> Vec<u8> {
    let mut out = Vec::new();
    encode_u32_field(1, value.channel_id, &mut out);
    encode_u32_field(2, value.command.as_u32(), &mut out);
    encode_u8_field(3, value.check_sum, &mut out);
    encode_u8_field(4, value.v_code, &mut out);
    out
}

pub fn decode_payload_protect(bytes: &[u8]) -> Result<PayloadProtect> {
    let mut index = 0;
    let mut protect = PayloadProtect {
        channel_id: 0,
        command: HdcCommand::Unknown(0),
        check_sum: 0,
        v_code: 0,
    };
    while index < bytes.len() {
        let tag_key = decode_varint(bytes, &mut index)? as u32;
        let field_number = tag_key >> 3;
        let wire_type = (tag_key & 0x07) as u8;
        match (field_number, wire_type) {
            (1, 0) => protect.channel_id = decode_varint(bytes, &mut index)? as u32,
            (2, 0) => {
                protect.command = HdcCommand::from_u32(decode_varint(bytes, &mut index)? as u32)
            }
            (3, 0) => protect.check_sum = decode_varint(bytes, &mut index)? as u8,
            (4, 0) => protect.v_code = decode_varint(bytes, &mut index)? as u8,
            _ => skip_field(bytes, &mut index, wire_type)?,
        }
    }
    Ok(protect)
}

pub fn encode_frame(frame: &Frame) -> Vec<u8> {
    let protect = PayloadProtect {
        channel_id: frame.channel_id,
        command: frame.command,
        check_sum: 0,
        v_code: PAYLOAD_VCODE,
    };
    let protect_bytes = encode_payload_protect(&protect);
    let mut out = Vec::with_capacity(PAYLOAD_HEAD_SIZE + protect_bytes.len() + frame.payload.len());
    out.extend_from_slice(&PACKET_FLAG);
    out.extend_from_slice(&[0, 0]);
    out.push(VER_PROTOCOL);
    out.extend_from_slice(&(protect_bytes.len() as u16).to_be_bytes());
    out.extend_from_slice(&(frame.payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&protect_bytes);
    out.extend_from_slice(&frame.payload);
    out
}

#[cfg(test)]
pub fn decode_frame(bytes: &[u8]) -> Result<Frame> {
    if bytes.len() < PAYLOAD_HEAD_SIZE {
        return Err(HdcError::protocol("truncated payload head"));
    }
    if bytes[..2] != PACKET_FLAG {
        return Err(HdcError::protocol("invalid packet flag"));
    }
    let head_size = u16::from_be_bytes([bytes[5], bytes[6]]) as usize;
    let data_size = u32::from_be_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]) as usize;
    let expected = PAYLOAD_HEAD_SIZE + head_size + data_size;
    if bytes.len() != expected {
        return Err(HdcError::protocol("payload size mismatch"));
    }
    let protect = decode_payload_protect(&bytes[PAYLOAD_HEAD_SIZE..PAYLOAD_HEAD_SIZE + head_size])?;
    if protect.v_code != PAYLOAD_VCODE {
        return Err(HdcError::protocol("invalid payload vcode"));
    }
    let payload = bytes[PAYLOAD_HEAD_SIZE + head_size..].to_vec();
    Ok(Frame {
        channel_id: protect.channel_id,
        command: protect.command,
        payload,
    })
}

pub fn write_frame<W: Write>(writer: &mut W, frame: &Frame) -> Result<()> {
    writer.write_all(&encode_frame(frame))?;
    writer.flush()?;
    Ok(())
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<Frame> {
    let mut head = [0u8; PAYLOAD_HEAD_SIZE];
    reader.read_exact(&mut head)?;
    if head[..2] != PACKET_FLAG {
        return Err(HdcError::protocol("invalid packet flag"));
    }
    let head_size = u16::from_be_bytes([head[5], head[6]]) as usize;
    let data_size = u32::from_be_bytes([head[7], head[8], head[9], head[10]]) as usize;

    let mut protect_bytes = vec![0u8; head_size];
    reader.read_exact(&mut protect_bytes)?;
    let protect = decode_payload_protect(&protect_bytes)?;
    if protect.v_code != PAYLOAD_VCODE {
        return Err(HdcError::protocol("invalid payload vcode"));
    }

    let mut payload = vec![0u8; data_size];
    reader.read_exact(&mut payload)?;
    Ok(Frame {
        channel_id: protect.channel_id,
        command: protect.command,
        payload,
    })
}

fn encode_u8_field(tag: u32, value: u8, out: &mut Vec<u8>) {
    encode_varint((tag << 3) as u64, out);
    encode_varint(value as u64, out);
}

fn encode_u32_field(tag: u32, value: u32, out: &mut Vec<u8>) {
    encode_varint((tag << 3) as u64, out);
    encode_varint(value as u64, out);
}

fn encode_length_field(tag: u32, value: &[u8], out: &mut Vec<u8>) {
    encode_varint(((tag << 3) | 2) as u64, out);
    encode_varint(value.len() as u64, out);
    out.extend_from_slice(value);
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn decode_varint(bytes: &[u8], index: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *bytes
            .get(*index)
            .ok_or_else(|| HdcError::protocol("truncated varint"))?;
        *index += 1;
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err(HdcError::protocol("varint overflow"));
        }
    }
}

fn decode_length_string(bytes: &[u8], index: &mut usize) -> Result<String> {
    let len = decode_varint(bytes, index)? as usize;
    let end = index
        .checked_add(len)
        .ok_or_else(|| HdcError::protocol("length overflow"))?;
    let slice = bytes
        .get(*index..end)
        .ok_or_else(|| HdcError::protocol("truncated length-delimited field"))?;
    *index = end;
    Ok(String::from_utf8(slice.to_vec())?)
}

fn skip_field(bytes: &[u8], index: &mut usize, wire_type: u8) -> Result<()> {
    match wire_type {
        0 => {
            let _ = decode_varint(bytes, index)?;
            Ok(())
        }
        1 => {
            *index = index
                .checked_add(8)
                .ok_or_else(|| HdcError::protocol("fixed64 overflow"))?;
            if *index > bytes.len() {
                return Err(HdcError::protocol("truncated fixed64"));
            }
            Ok(())
        }
        2 => {
            let len = decode_varint(bytes, index)? as usize;
            *index = index
                .checked_add(len)
                .ok_or_else(|| HdcError::protocol("length overflow"))?;
            if *index > bytes.len() {
                return Err(HdcError::protocol("truncated length-delimited field"));
            }
            Ok(())
        }
        5 => {
            *index = index
                .checked_add(4)
                .ok_or_else(|| HdcError::protocol("fixed32 overflow"))?;
            if *index > bytes.len() {
                return Err(HdcError::protocol("truncated fixed32"));
            }
            Ok(())
        }
        other => Err(HdcError::protocol(format!("unsupported wire type {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{
        Frame, decode_frame, decode_session_handshake, encode_frame, encode_session_handshake,
    };
    use crate::protocol::{AuthType, HdcCommand, SessionHandshake};

    #[test]
    fn session_handshake_roundtrip_preserves_all_fields() {
        let handshake = SessionHandshake {
            banner: "OHOS HDC".into(),
            auth_type: AuthType::None,
            session_id: 7,
            connect_key: "192.168.8.43:35319".into(),
            buf: "payload".into(),
            version: "Ver: 3.1.0e".into(),
        };

        let encoded = encode_session_handshake(&handshake);
        let decoded = decode_session_handshake(&encoded).unwrap();

        assert_eq!(decoded, handshake);
    }

    #[test]
    fn frame_roundtrip_preserves_head_and_payload() {
        let frame = Frame {
            channel_id: 1,
            command: HdcCommand::KernelHandshake,
            payload: b"abc".to_vec(),
        };

        let encoded = encode_frame(&frame);
        let decoded = decode_frame(&encoded).unwrap();

        assert_eq!(decoded, frame);
    }
}
