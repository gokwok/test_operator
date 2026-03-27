use std::collections::BTreeMap;

use crate::error::{HdcError, Result};

pub const HANDSHAKE_MESSAGE: &str = "OHOS HDC";
pub const PACKET_FLAG: [u8; 2] = *b"HW";
pub const VER_PROTOCOL: u8 = 1;
pub const PAYLOAD_VCODE: u8 = 0x09;
pub const PAYLOAD_HEAD_SIZE: usize = 11;
pub const HDC_HOST_DAEMON_BUF_SEPARATOR: char = '\u{000C}';
pub const DEFAULT_VERSION: &str = "Ver: 3.1.0e";
pub const MIN_AUTH_VERSION: &str = "Ver: 3.0.0b";
pub const CHANNEL_HANDSHAKE: u32 = 0;
pub const CHANNEL_COMMAND: u32 = 1;
pub const CHANNEL_FORWARD: u32 = 3;
pub const TLV_TAG_LEN: usize = 16;
pub const TLV_VAL_LEN: usize = 16;
pub const TAG_AUTH_TYPE: &str = "authtype";
pub const TAG_EMGMSG: &str = "emgmsg";
pub const TAG_DAEMON_AUTHSTATUS: &str = "daemonauthstatus";
pub const DAEMON_AUTH_SUCCESS: &str = "SUCCESS";
pub const DAEMON_UNAUTHORIZED: &str = "DAEMON_UNAUTH";
pub const AUTH_VERIFY_RSA_3072_SHA512: &str = "1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthType {
    None,
    Token,
    Signature,
    PublicKey,
    Ok,
    Fail,
    Unknown(u8),
}

impl AuthType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Token,
            2 => Self::Signature,
            3 => Self::PublicKey,
            4 => Self::Ok,
            5 => Self::Fail,
            other => Self::Unknown(other),
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Token => 1,
            Self::Signature => 2,
            Self::PublicKey => 3,
            Self::Ok => 4,
            Self::Fail => 5,
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdcCommand {
    KernelHelp,
    KernelHandshake,
    KernelChannelClose,
    KernelEcho,
    KernelEchoRaw,
    KernelWakeupSlavetask,
    UnityExecute,
    ShellInit,
    ShellData,
    ForwardInit,
    ForwardCheck,
    ForwardCheckResult,
    ForwardActiveSlave,
    ForwardActiveMaster,
    ForwardData,
    ForwardFreeContext,
    HeartbeatMsg,
    Unknown(u32),
}

impl HdcCommand {
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::KernelHelp,
            1 => Self::KernelHandshake,
            2 => Self::KernelChannelClose,
            9 => Self::KernelEcho,
            10 => Self::KernelEchoRaw,
            12 => Self::KernelWakeupSlavetask,
            1001 => Self::UnityExecute,
            2000 => Self::ShellInit,
            2001 => Self::ShellData,
            2500 => Self::ForwardInit,
            2501 => Self::ForwardCheck,
            2502 => Self::ForwardCheckResult,
            2503 => Self::ForwardActiveSlave,
            2504 => Self::ForwardActiveMaster,
            2505 => Self::ForwardData,
            2506 => Self::ForwardFreeContext,
            5000 => Self::HeartbeatMsg,
            other => Self::Unknown(other),
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            Self::KernelHelp => 0,
            Self::KernelHandshake => 1,
            Self::KernelChannelClose => 2,
            Self::KernelEcho => 9,
            Self::KernelEchoRaw => 10,
            Self::KernelWakeupSlavetask => 12,
            Self::UnityExecute => 1001,
            Self::ShellInit => 2000,
            Self::ShellData => 2001,
            Self::ForwardInit => 2500,
            Self::ForwardCheck => 2501,
            Self::ForwardCheckResult => 2502,
            Self::ForwardActiveSlave => 2503,
            Self::ForwardActiveMaster => 2504,
            Self::ForwardData => 2505,
            Self::ForwardFreeContext => 2506,
            Self::HeartbeatMsg => 5000,
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Fail,
    Info,
    Ok,
    Unknown(u8),
}

impl MessageLevel {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Fail,
            1 => Self::Info,
            2 => Self::Ok,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandshake {
    pub banner: String,
    pub auth_type: AuthType,
    pub session_id: u32,
    pub connect_key: String,
    pub buf: String,
    pub version: String,
}

impl Default for SessionHandshake {
    fn default() -> Self {
        Self {
            banner: String::new(),
            auth_type: AuthType::None,
            session_id: 0,
            connect_key: String::new(),
            buf: String::new(),
            version: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadProtect {
    pub channel_id: u32,
    pub command: HdcCommand,
    pub check_sum: u8,
    pub v_code: u8,
}

pub fn append_tlv(buffer: &mut String, tag: &str, value: &str) {
    let mut tag_field = tag.to_string();
    if tag_field.len() < TLV_TAG_LEN {
        tag_field.push_str(&" ".repeat(TLV_TAG_LEN - tag_field.len()));
    }
    buffer.push_str(&tag_field[..TLV_TAG_LEN]);

    let mut len_field = value.len().to_string();
    if len_field.len() < TLV_VAL_LEN {
        len_field.push_str(&" ".repeat(TLV_VAL_LEN - len_field.len()));
    }
    buffer.push_str(&len_field[..TLV_VAL_LEN]);
    buffer.push_str(value);
}

pub fn parse_tlv_map(tlv: &str) -> Result<BTreeMap<String, String>> {
    let mut remaining = tlv;
    let mut map = BTreeMap::new();

    if remaining.is_empty() {
        return Ok(map);
    }
    if remaining.len() < TLV_TAG_LEN + TLV_VAL_LEN {
        return Err(HdcError::protocol("invalid tlv payload"));
    }

    while !remaining.is_empty() {
        if remaining.len() < TLV_TAG_LEN + TLV_VAL_LEN {
            return Err(HdcError::protocol("truncated tlv header"));
        }
        let tag_raw = &remaining[..TLV_TAG_LEN];
        remaining = &remaining[TLV_TAG_LEN..];

        let len_raw = &remaining[..TLV_VAL_LEN];
        remaining = &remaining[TLV_VAL_LEN..];

        let tag = tag_raw.trim_end().to_string();
        let value_len = len_raw.trim_end().parse::<usize>()?;
        if remaining.len() < value_len {
            return Err(HdcError::protocol("truncated tlv value"));
        }
        let value = remaining[..value_len].to_string();
        remaining = &remaining[value_len..];
        map.insert(tag, value);
    }

    Ok(map)
}
