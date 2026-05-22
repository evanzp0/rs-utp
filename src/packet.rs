use std::{fmt, io};
use bytes::{Buf, BytesMut}; 
use tokio_util::codec::{Decoder};
use thiserror::Error;

const PACKET_HEADER_LEN: usize = 20;

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

    #[error("invalid length")]
    InvalidLen,
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

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    
    #[error("Packet header error: {0}")]
    Header(#[from] PacketHeaderError),
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
enum Version {
    One,
}

impl TryFrom<u8> for Version {
    type Error = InvalidVersion;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::One),
            _ => Err(InvalidVersion(value)),
        }
    }
}

impl From<Version> for u8 {
    fn from(value: Version) -> u8 {
        match value {
            Version::One => 1,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Extension {
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
        if buf.remaining() < PACKET_HEADER_LEN {
            return Err(PacketHeaderError::InvalidLen.into());
        }

        let type_ver = buf.get_u8();
        let packet_type = PacketType::try_from(type_ver >> 4)
            .map_err(PacketHeaderError::from)?;
        let version = Version::try_from(type_ver & 0x0F)
            .map_err(PacketHeaderError::from)?;
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

    pub fn encode<B: bytes::BufMut>(&self, buf: &mut B) {
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
}

#[derive(Debug, Clone, Default)]
pub struct PacketCodec;

impl Decoder for PacketCodec {
    type Item = Packet;
    type Error = DecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < PACKET_HEADER_LEN {
            return Err(DecodeError::Header(PacketHeaderError::InvalidLen));
        }

        let header = PacketHeader::decode(src)?;

        // TODO: 这里应该根据 header.extension 的值来决定是否还需要解析扩展部分，目前先忽略扩展部分的解析

        Ok(Some(Packet {
            header,
        }))
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

    // 为完整的 PacketHeader 生成策略
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
                version: Version::One,
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

    // 为有效的编码数据生成策略（不包含长度不足的情况）
    fn _valid_encoded_header_strategy() -> impl Strategy<Value = BytesMut> {
        packet_header_strategy().prop_map(|header| {
            let mut bytes = BytesMut::with_capacity(PACKET_HEADER_LEN);
            
            // 编码 type 和 version
            let type_ver = (u8::from(header.packet_type) << 4) | u8::from(header.version);
            bytes.extend_from_slice(&type_ver.to_be_bytes());
            
            // 编码 extension
            bytes.extend_from_slice(&u8::from(header.extension).to_be_bytes());
            
            // 编码其他字段
            bytes.extend_from_slice(&header.conn_id.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp_diff.to_be_bytes());
            bytes.extend_from_slice(&header.wnd_size.to_be_bytes());
            bytes.extend_from_slice(&header.seq_nr.to_be_bytes());
            bytes.extend_from_slice(&header.ack_nr.to_be_bytes());
            
            bytes
        })
    }

    // 为完整的 Packet 生成策略
    fn packet_strategy() -> impl Strategy<Value = Packet> {
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
            let header = PacketHeader {
                packet_type,
                version: Version::One,
                extension,
                conn_id,
                timestamp,
                timestamp_diff,
                wnd_size,
                seq_nr,
                ack_nr,
            };

            Packet {
                header,
            }
        })
    }

    // 正常解码应该成功
    proptest! {
        #[test]
        fn test_decode_valid_header(header in packet_header_strategy()) {
            let mut bytes = BytesMut::new();

            // 手动编码 PacketHeader
            let type_ver = (u8::from(header.packet_type) << 4) | u8::from(header.version);
            bytes.extend_from_slice(&type_ver.to_be_bytes());
            bytes.extend_from_slice(&u8::from(header.extension).to_be_bytes());
            bytes.extend_from_slice(&header.conn_id.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp_diff.to_be_bytes());
            bytes.extend_from_slice(&header.wnd_size.to_be_bytes());
            bytes.extend_from_slice(&header.seq_nr.to_be_bytes());
            bytes.extend_from_slice(&header.ack_nr.to_be_bytes());
            
            let result = PacketHeader::decode(&mut bytes);
            
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), header);
        }
    }

    // 测试无效长度（边界情况）
    proptest! {
        #[test]
        fn test_decode_invalid_length(len in 0..=19usize) {
            let mut bytes = BytesMut::with_capacity(len);
            bytes.resize(len, 0u8);
            
            let result = PacketHeader::decode(&mut bytes);
            
            prop_assert!(result.is_err());
            let err = result.unwrap_err();
            match err {
                PacketHeaderError::InvalidLen => {}
                _ => panic!("Expected InvalidLen error, got {:?}", err),
            }
        }
    }

    // 测试无效的 packet_type
    proptest! {
        #[test]
        fn test_decode_invalid_packet_type(invalid_type in 5u8..=15u8) {
            let mut bytes = BytesMut::with_capacity(PACKET_HEADER_LEN);
            
            // 构造一个无效的 packet_type（version 保持有效）
            let type_ver = (invalid_type << 4) | u8::from(Version::One);
            bytes.extend_from_slice(&type_ver.to_be_bytes());
            
            // 填充剩余字段为任意值
            bytes.resize(PACKET_HEADER_LEN, 0u8);
            
            let result = PacketHeader::decode(&mut bytes);
            
            prop_assert!(result.is_err());
            let err = result.unwrap_err();
            match err {
                PacketHeaderError::InvalidPacketType(_) => {}
                _ => panic!("Expected InvalidPacketType error, got {:?}", err),
            }
        }
    }

    // 测试无效的 version
    proptest! {
        #[test]
        fn test_decode_invalid_version(
            invalid_ver in (0u8..=15u8).prop_filter("version must not be 1", |&v| v != 1),
            valid_packet_type in 0u8..=4u8,
        ) {
            let mut bytes = BytesMut::with_capacity(PACKET_HEADER_LEN);
            
            // 构造无效的 version
            let type_ver = (valid_packet_type << 4) | invalid_ver;
            bytes.extend_from_slice(&type_ver.to_be_bytes());
            
            // 填充剩余字段
            bytes.resize(PACKET_HEADER_LEN, 0u8);
            
            let result = PacketHeader::decode(&mut bytes);
            
            prop_assert!(result.is_err());
            let err = result.unwrap_err();
            match err {
                PacketHeaderError::InvalidVersion(_) => {}
                _ => panic!("Expected InvalidVersion error, got {:?}", err),
            }
        }
    }

    // 测试边界值（最大最小值）
    #[test]
    fn test_decode_boundary_values() {
        let test_cases = vec![
            // 最小值
            PacketHeader {
                packet_type: PacketType::Data,
                version: Version::One,
                extension: Extension::None,
                conn_id: 0,
                timestamp: 0,
                timestamp_diff: 0,
                wnd_size: 0,
                seq_nr: 0,
                ack_nr: 0,
            },
            // 最大值
            PacketHeader {
                packet_type: PacketType::Syn,
                version: Version::One,
                extension: Extension::Unknown(255),
                conn_id: u16::MAX,
                timestamp: u32::MAX,
                timestamp_diff: u32::MAX,
                wnd_size: u32::MAX,
                seq_nr: u16::MAX,
                ack_nr: u16::MAX,
            },
        ];
        
        for header in test_cases {
            let mut bytes = BytesMut::new();
            let type_ver = (u8::from(header.packet_type) << 4) | u8::from(header.version);
            bytes.extend_from_slice(&type_ver.to_be_bytes());
            bytes.extend_from_slice(&u8::from(header.extension).to_be_bytes());
            bytes.extend_from_slice(&header.conn_id.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp_diff.to_be_bytes());
            bytes.extend_from_slice(&header.wnd_size.to_be_bytes());
            bytes.extend_from_slice(&header.seq_nr.to_be_bytes());
            bytes.extend_from_slice(&header.ack_nr.to_be_bytes());
            
            let decoded = PacketHeader::decode(&mut bytes).unwrap();
            assert_eq!(decoded, header);
        }
    }

    // 正常 packet 解码应该成功
    proptest! {
        #[test]
        fn test_decode_valid_packet(packet in packet_strategy()) {
            let mut bytes = BytesMut::new();

            let header = &packet.header;
            
            // 手动编码 PacketHeader
            let type_ver = (u8::from(header.packet_type) << 4) | u8::from(header.version);
            bytes.extend_from_slice(&type_ver.to_be_bytes());
            bytes.extend_from_slice(&u8::from(header.extension).to_be_bytes());
            bytes.extend_from_slice(&header.conn_id.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp.to_be_bytes());
            bytes.extend_from_slice(&header.timestamp_diff.to_be_bytes());
            bytes.extend_from_slice(&header.wnd_size.to_be_bytes());
            bytes.extend_from_slice(&header.seq_nr.to_be_bytes());
            bytes.extend_from_slice(&header.ack_nr.to_be_bytes());
            
            let mut codec = PacketCodec;
            let result = codec.decode(&mut bytes);
            
            prop_assert!(result.is_ok());
            let decoded = result.unwrap();
            prop_assert!(decoded.is_some());
            prop_assert_eq!(decoded.unwrap(), packet);
        }
    }

    // 测试 encode 方法 ------------

    proptest! {
        #[test]
        fn test_encode_header(header in packet_header_strategy()) {
            let mut buf = bytes::BytesMut::new();
            header.encode(&mut buf);
            
            // 验证长度
            assert_eq!(buf.len(), PACKET_HEADER_LEN);
            
            // 解码验证
            let decoded = PacketHeader::decode(&mut buf.as_ref()).unwrap();
            assert_eq!(decoded, header);
        }
    }
}