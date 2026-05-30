use std::fmt;
use bytes::{Buf, BufMut, Bytes, BytesMut}; 
use thiserror::Error;

pub const PACKET_HEADER_LEN: usize = 20;
pub const MAX_PAYLOAD_SIZE: usize = 1400;

// ------------------------------------

#[derive(Error, Copy, Clone, Debug)]
#[error("invalid uTP version: {0}")]
pub struct InvalidVersion(pub u8);

impl From<u8> for InvalidVersion {
    fn from(value: u8) -> Self {
        InvalidVersion(value)
    }
}

#[derive(Error, Copy, Clone, Debug)]
#[error("invalid uTP packet type: {0}")]
pub struct InvalidPacketType(pub u8);

impl From<u8> for InvalidPacketType {
    fn from(value: u8) -> Self {
        InvalidPacketType(value)
    }
}

#[derive(Error, Clone, Debug)]
pub enum PacketHeaderError {
    #[error(transparent)]
    InvalidPacketType(InvalidPacketType),

    #[error(transparent)]
    InvalidVersion(InvalidVersion),

    #[error("invalid length: {0}")]
    InvalidLen(usize),
}

impl From<InvalidPacketType> for PacketHeaderError {
    fn from(value: InvalidPacketType) -> Self {
        Self::InvalidPacketType(value)
    }
}

impl From<InvalidVersion> for PacketHeaderError {
    fn from(value: InvalidVersion) -> Self {
        Self::InvalidVersion(value)
    }
}

#[derive(Clone, Debug, Error)]
pub enum PacketError {
    #[error(transparent)]
    InvalidHeader(#[from] PacketHeaderError),
}

// ------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PacketType {
    Data,
    Fin,
    State,
    Reset,
    Syn,
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Data => "ST_DATA".to_string(),
            Self::Fin => "ST_FIN".to_string(),
            Self::State => "ST_STATE".to_string(),
            Self::Reset => "ST_RESET".to_string(),
            Self::Syn => "ST_SYN".to_string(),
        };

        write!(f, "{s}")
    }
}

impl TryFrom<u8> for PacketType {
    type Error = InvalidPacketType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Data),
            1 => Ok(Self::Fin),
            2 => Ok(Self::State),
            3 => Ok(Self::Reset),
            4 => Ok(Self::Syn),
            _ => Err(InvalidPacketType(value)),
        }
    }
}

impl From<PacketType> for u8 {
    fn from(value: PacketType) -> u8 {
        match value {
            PacketType::Data => 0,
            PacketType::Fin => 1,
            PacketType::State => 2,
            PacketType::Reset => 3,
            PacketType::Syn => 4,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Version {
    V1,
}

impl TryFrom<u8> for Version {
    type Error = InvalidVersion;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            _ => Err(InvalidVersion(value)),
        }
    }
}

impl From<Version> for u8 {
    fn from(value: Version) -> u8 {
        match value {
            Version::V1 => 1,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Extension {
    None,
    SelectiveAck,
    Unknown(u8),
}

impl From<u8> for Extension {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::SelectiveAck,
            unknown => Self::Unknown(unknown),
        }
    }
}

impl From<Extension> for u8 {
    fn from(value: Extension) -> u8 {
        match value {
            Extension::None => 0,
            Extension::SelectiveAck => 1,
            Extension::Unknown(ext) => ext,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketHeader {
    packet_type: PacketType,
    version: Version,
    extension: Extension,
    conn_id: u16,
    timestamp: u32,
    timestamp_diff: u32,
    wnd_size: u32,
    seq_nr: u16,
    ack_nr: u16,
}

impl PacketHeader {
    pub fn decode<B: Buf>(buf: &mut B) -> Result<Self, PacketHeaderError> {
        let len = buf.remaining();
        if buf.remaining() < PACKET_HEADER_LEN {
            return Err(PacketHeaderError::InvalidLen(len));
        }

        let type_ver = buf.get_u8();
        let packet_type = PacketType::try_from(type_ver >> 4)?;
        let version = Version::try_from(type_ver & 0x0F)?;
        let extension = buf.get_u8().into();
        let conn_id = buf.get_u16();
        let timestamp = buf.get_u32();
        let timestamp_diff = buf.get_u32();
        let wnd_size = buf.get_u32();
        let seq_nr = buf.get_u16();
        let ack_nr = buf.get_u16();

        Ok(PacketHeader {
            packet_type,
            version,
            extension,
            conn_id,
            timestamp,
            timestamp_diff,
            wnd_size,
            seq_nr,
            ack_nr,
        })
    }

    pub fn encode_to<B: BufMut>(&self, buf: &mut B) {
        let type_ver = (u8::from(self.packet_type) << 4) | u8::from(self.version);
        buf.put_u8(type_ver);
        buf.put_u8(self.extension.into());
        buf.put_u16(self.conn_id);
        buf.put_u32(self.timestamp);
        buf.put_u32(self.timestamp_diff);
        buf.put_u32(self.wnd_size);
        buf.put_u16(self.seq_nr);
        buf.put_u16(self.ack_nr);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    header: PacketHeader,
    payload: Bytes,
}

impl Packet {
    pub fn new(header: PacketHeader, payload: Bytes) -> Self {
        Self { header, payload }
    }

    // getter -----------

    pub fn packet_type(&self) -> PacketType { self.header.packet_type }
    pub fn version(&self) -> Version { self.header.version }
    pub fn extension(&self) -> Extension { self.header.extension }
    pub fn conn_id(&self) -> u16 { self.header.conn_id }
    pub fn timestamp(&self) -> u32 { self.header.timestamp }
    pub fn timestamp_diff(&self) -> u32 { self.header.timestamp_diff }
    pub fn wnd_size(&self) -> u32 { self.header.wnd_size }
    pub fn seq_nr(&self) -> u16 { self.header.seq_nr }
    pub fn ack_nr(&self) -> u16 { self.header.ack_nr }

    // method -----------

    pub fn decode<B: Buf>(buf: &mut B) -> Result<Self, PacketError>{
        let header = PacketHeader::decode(buf)?;

        // buf 零拷贝
        let payload = buf.copy_to_bytes(buf.remaining());


        // TODO: 这里应该根据 header.extension 的值来决定是否还需要解析扩展部分，目前先忽略扩展部分的解析

        Ok(Packet { header, payload })
    }

    pub fn encode_to<B: BufMut>(&self, buf: &mut B) {
        self.header.encode_to(buf);

        // TODO: 写入扩展头部 (如果有)

        if !self.payload.is_empty() {
            buf.put_slice(&self.payload);
        }
    }

    pub fn encode(&self) -> impl AsRef<[u8]> {
        let mut buffer = BytesMut::with_capacity(PACKET_HEADER_LEN);
        self.encode_to(&mut buffer);

        buffer.freeze()
    }
}

#[derive(Clone, Debug)]
pub struct PacketBuilder {
    packet_type: PacketType,
    conn_id: u16,
    timestamp: u32,
    timestamp_diff: u32,
    wnd_size: u32,
    seq_nr: u16,
    ack_nr: u16,
    payload: Option<Bytes>,
}

impl PacketBuilder {
    pub fn new(
        packet_type: PacketType,
        conn_id: u16,
        timestamp: u32,
        wnd_size: u32,
        seq_nr: u16,
    ) -> Self {
        Self {
            packet_type,
            conn_id,
            timestamp,
            timestamp_diff: 0,
            wnd_size,
            seq_nr,
            ack_nr: 0,
            payload: None,
        }
    }

    pub fn timestamp(mut self, timestamp: u32) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn timestamp_diff(mut self, timestamp_diff: u32) -> Self {
        self.timestamp_diff = timestamp_diff;
        self
    }

    pub fn wnd_size(mut self, wnd_size: u32) -> Self {
        self.wnd_size = wnd_size;
        self
    }

    pub fn seq_nr(mut self, seq_nr: u16) -> Self {
        self.seq_nr = seq_nr;
        self
    }

    pub fn ack_nr(mut self, ack_nr: u16) -> Self {
        self.ack_nr = ack_nr;
        self
    }

    pub fn payload(mut self, payload: Bytes) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn build(self) -> Packet {
        Packet {
            header: PacketHeader {
                packet_type: self.packet_type,
                version: Version::V1,
                extension: Extension::None,
                conn_id: self.conn_id,
                timestamp: self.timestamp,
                timestamp_diff: self.timestamp_diff,
                wnd_size: self.wnd_size,
                seq_nr: self.seq_nr,
                ack_nr: self.ack_nr,
            },
            payload: self.payload.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use bytes::BytesMut;

    // 为 PacketType 生成策略
    fn packet_type_strategy() -> impl Strategy<Value = PacketType> {
        prop::sample::select(vec![
            PacketType::Data,
            PacketType::Fin,
            PacketType::State,
            PacketType::Reset,
            PacketType::Syn,
        ])
    }

    // 为 Extension 生成策略
    fn extension_strategy() -> impl Strategy<Value = Extension> {
        prop_oneof![
            Just(Extension::None),
            Just(Extension::SelectiveAck),
            (2u8..255).prop_map(|v| Extension::Unknown(v)),
        ]
    }

    // 为完整的 PacketHeader 生成策略（any 生成伪随机数，其中必含边界值）
    fn packet_header_strategy() -> impl Strategy<Value = PacketHeader> {
        (
            packet_type_strategy(),
            extension_strategy(),
            any::<u16>(),      // conn_id
            any::<u32>(),      // timestamp
            any::<u32>(),      // timestamp_diff
            any::<u32>(),      // wnd_size
            any::<u16>(),      // seq_nr
            any::<u16>(),      // ack_nr
        ).prop_map(|(packet_type, extension, conn_id, timestamp, timestamp_diff, wnd_size, seq_nr, ack_nr)| {
            PacketHeader {
                packet_type,
                version: Version::V1,
                extension,
                conn_id,
                timestamp,
                timestamp_diff,
                wnd_size,
                seq_nr,
                ack_nr,
            }
        })
    }

    // 为有效的编码数据生成策略，作为构造异常数据（模糊测试/变异测试）的基准
    fn valid_encoded_header_strategy() -> impl Strategy<Value = BytesMut> {
        packet_header_strategy().prop_map(|header| {
            let mut bytes = BytesMut::with_capacity(PACKET_HEADER_LEN);
            header.encode_to(&mut bytes);
            bytes
        })
    }

    // 为 payload 生成策略
    fn payload_strategy() -> impl Strategy<Value = Bytes> {
        prop::collection::vec(any::<u8>(), 0..MAX_PAYLOAD_SIZE)
            .prop_map(|vec| Bytes::from(vec))
    }

    // 为完整的 Packet 生成策略
    fn packet_strategy() -> impl Strategy<Value = Packet> {
        (packet_header_strategy(), payload_strategy())
            .prop_map(|(header, payload)| Packet { header, payload })
    }

    // ------------------------------------

    // 正常 header 编码/解码 应该成功
    proptest! {
        #[test]
        fn test_header_roundtrip(header in packet_header_strategy()) {
            let mut buf = BytesMut::with_capacity(PACKET_HEADER_LEN);

            // 1. 编码：Header -> BytesMut
            header.encode_to(&mut buf);
            prop_assert_eq!(buf.len(), PACKET_HEADER_LEN);

            // 2. 解码：BytesMut -> Header
            let decoded = PacketHeader::decode(&mut buf);
            prop_assert!(decoded.is_ok());

            // 3. 断言：解码后的数据应该和原始数据完全一致
            prop_assert_eq!(decoded.unwrap(), header);
        }
    }

    // 测试无效长度（边界情况）
    // truncated_len: 从 0 到 PACKET_HEADER_LEN-1，确保每个长度都被测试到
    proptest! {
        #[test]
        fn test_decode_invalid_length(
            mut encoded_bytes in valid_encoded_header_strategy(),
            truncated_len in 0..PACKET_HEADER_LEN,
        ) {
            // 截断字节流
            let mut truncated_bytes = encoded_bytes.split_to(truncated_len);
            
            let result = Packet::decode(&mut truncated_bytes);
            
            // 应该返回 InvalidLen 错误
            prop_assert!(matches!(result, Err(PacketError::InvalidHeader(PacketHeaderError::InvalidLen(_)))));
        }
    }

    // 测试无效的 packet_type
    proptest! {
        #[test]
        fn test_decode_invalid_packet_type(mut encoded_bytes in valid_encoded_header_strategy()) {
            // encoded_bytes[0] 是 type_ver 字节：高4位是 type，低4位是 version

            // 1. 提取低4位（保留原来的 Version=1），高4位清零
            let original_ver = encoded_bytes[0] & 0x0F; 
            
            // 2. 强行把高4位设为 5 (无效类型，因为合法类型只有 0~4)
            encoded_bytes[0] = original_ver | 0x50; 
            
            let result = Packet::decode(&mut encoded_bytes);
            
            // 3. 断言：应该报 InvalidPacketType 错误
            prop_assert!(matches!(
                result, 
                Err(PacketError::InvalidHeader(PacketHeaderError::InvalidPacketType(_)))
            ));
        }
    }

    // 测试无效的 version
    proptest! {
        #[test]
        fn test_decode_invalid_version(mut encoded_bytes in valid_encoded_header_strategy()) {
            // 1. 提取高4位（保留原来的 Type），低4位清零
            let original_type = encoded_bytes[0] & 0xF0; 
            
            // 2. 强行把低4位设为 0 (无效版本，因为合法版本只有1)
            encoded_bytes[0] = original_type | 0x00; 
            
            let result = Packet::decode(&mut encoded_bytes);
            
            // 3. 断言：应该报 InvalidVersion 错误
            prop_assert!(matches!(
                result, 
                Err(PacketError::InvalidHeader(PacketHeaderError::InvalidVersion(_)))
            ));
        }
    }

    // 正常 packet 编码/解码 应该成功
    proptest! {
        #[test]
        fn test_packet_roundtrip(packet in packet_strategy()) {
            let mut bytes = BytesMut::new();

            packet.encode_to(&mut bytes);

            let decoded = Packet::decode(&mut bytes);
            prop_assert!(decoded.is_ok());
            
            let decoded = decoded.unwrap();
            prop_assert_eq!(decoded, packet);
        }
    }

    // 测试 encode 方法 ------------
    proptest! {
        #[test]
        fn test_encode_packet(packet in packet_strategy()) {
            let mut buf = bytes::BytesMut::new();

            packet.encode_to(&mut buf);

            let expected_len = PACKET_HEADER_LEN + packet.payload.len();
            prop_assert_eq!(buf.len(), expected_len);
            
            // 解码验证
            let decoded = Packet::decode(&mut buf);
            prop_assert!(decoded.is_ok());
            assert_eq!(decoded.unwrap(), packet);
        }
    }
}