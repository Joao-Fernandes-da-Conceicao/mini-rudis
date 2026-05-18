/// RESP（Redis Serialization Protocol）协议解析与序列化
///
/// 支持的数据类型：
/// - `+` Simple String
/// - `-` Error
/// - `:` Integer
/// - `$` Bulk String（含 Null Bulk `$-1\r\n`）
/// - `*` Array（含 Null Array `*-1\r\n`）
use bytes::{Buf, Bytes, BytesMut};
use std::io::Cursor;

use crate::error::{RedisError, Result};

/// RESP 协议帧
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// `+OK\r\n`
    Simple(String),
    /// `-ERR message\r\n`
    Error(String),
    /// `:42\r\n`
    Integer(i64),
    /// `$6\r\nfoobar\r\n`
    Bulk(Bytes),
    /// `$-1\r\n` 或 `*-1\r\n`
    Null,
    /// `*3\r\n...`
    Array(Vec<Frame>),
}

impl Frame {
    /// 将帧序列化为 RESP 字节串
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode(&mut buf);
        buf
    }

    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Frame::Simple(s) => {
                buf.push(b'+');
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            Frame::Error(s) => {
                buf.push(b'-');
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            Frame::Integer(n) => {
                buf.push(b':');
                buf.extend_from_slice(n.to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            Frame::Bulk(data) => {
                buf.push(b'$');
                buf.extend_from_slice(data.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                buf.extend_from_slice(data);
                buf.extend_from_slice(b"\r\n");
            }
            Frame::Null => {
                buf.extend_from_slice(b"$-1\r\n");
            }
            Frame::Array(frames) => {
                buf.push(b'*');
                buf.extend_from_slice(frames.len().to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
                for f in frames {
                    f.encode(buf);
                }
            }
        }
    }

    /// 快捷构造：OK 响应
    pub fn ok() -> Frame {
        Frame::Simple("OK".to_string())
    }

    /// 快捷构造：PONG 响应
    pub fn pong() -> Frame {
        Frame::Simple("PONG".to_string())
    }

    /// 快捷构造：将字符串包装为 Bulk
    pub fn bulk_str(s: impl Into<String>) -> Frame {
        Frame::Bulk(Bytes::from(s.into()))
    }

    /// 快捷构造：将字节包装为 Bulk
    pub fn bulk_bytes(b: Vec<u8>) -> Frame {
        Frame::Bulk(Bytes::from(b))
    }

    /// 快捷构造：整数帧
    pub fn int(n: i64) -> Frame {
        Frame::Integer(n)
    }

    /// 快捷构造：错误帧
    pub fn error(msg: impl Into<String>) -> Frame {
        Frame::Error(msg.into())
    }

    /// 从 RedisError 构造错误帧
    pub fn from_error(e: &RedisError) -> Frame {
        Frame::Error(e.to_string())
    }
}

/// 尝试从缓冲区解析一个完整的 RESP 帧。
///
/// - 返回 `Ok(Some(frame))` 表示解析成功，并消费对应字节
/// - 返回 `Ok(None)` 表示数据不足，等待更多数据
/// - 返回 `Err` 表示协议错误
pub fn parse_frame(buf: &mut BytesMut) -> Result<Option<Frame>> {
    if buf.is_empty() {
        return Ok(None);
    }

    let mut cursor = Cursor::new(&buf[..]);
    match decode_frame(&mut cursor) {
        Ok(frame) => {
            let consumed = cursor.position() as usize;
            buf.advance(consumed);
            Ok(Some(frame))
        }
        Err(RedisError::Protocol(ref msg)) if msg == "incomplete" => Ok(None),
        Err(e) => Err(e),
    }
}

/// 递归解析一个 RESP 帧
fn decode_frame(cursor: &mut Cursor<&[u8]>) -> Result<Frame> {
    if !cursor.has_remaining() {
        return Err(RedisError::Protocol("incomplete".into()));
    }
    let byte = cursor.get_u8();
    match byte {
        b'+' => {
            let line = read_line(cursor)?;
            Ok(Frame::Simple(line))
        }
        b'-' => {
            let line = read_line(cursor)?;
            Ok(Frame::Error(line))
        }
        b':' => {
            let line = read_line(cursor)?;
            let n = line
                .parse::<i64>()
                .map_err(|_| RedisError::Protocol(format!("invalid integer: {line}")))?;
            Ok(Frame::Integer(n))
        }
        b'$' => {
            let line = read_line(cursor)?;
            let len = line
                .parse::<i64>()
                .map_err(|_| RedisError::Protocol(format!("invalid bulk length: {line}")))?;
            if len == -1 {
                return Ok(Frame::Null);
            }
            if len < 0 {
                return Err(RedisError::Protocol(format!("invalid bulk length: {len}")));
            }
            let len = len as usize;
            let needed = len + 2; // data + \r\n
            if cursor.remaining() < needed {
                return Err(RedisError::Protocol("incomplete".into()));
            }
            let pos = cursor.position() as usize;
            let data = cursor.get_ref()[pos..pos + len].to_vec();
            cursor.advance(needed);
            Ok(Frame::Bulk(Bytes::from(data)))
        }
        b'*' => {
            let line = read_line(cursor)?;
            let count = line
                .parse::<i64>()
                .map_err(|_| RedisError::Protocol(format!("invalid array length: {line}")))?;
            if count == -1 {
                return Ok(Frame::Null);
            }
            if count < 0 {
                return Err(RedisError::Protocol(format!(
                    "invalid array length: {count}"
                )));
            }
            let mut frames = Vec::with_capacity(count as usize);
            for _ in 0..count {
                frames.push(decode_frame(cursor)?);
            }
            Ok(Frame::Array(frames))
        }
        // 支持内联命令格式（如 telnet 手动输入 "PING\r\n"）
        _ => {
            // 把已读的 byte 放回去（通过往前退一位再 read_line）
            cursor.set_position(cursor.position() - 1);
            let line = read_line(cursor)?;
            // 内联命令解析为 Bulk 数组
            let parts: Vec<Frame> = line
                .split_ascii_whitespace()
                .map(|s| Frame::Bulk(Bytes::from(s.to_owned())))
                .collect();
            if parts.is_empty() {
                return Err(RedisError::Protocol("empty inline command".into()));
            }
            Ok(Frame::Array(parts))
        }
    }
}

/// 读取一行（以 `\r\n` 结尾），返回去掉 `\r\n` 后的字符串
fn read_line(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let start = cursor.position() as usize;
    let slice = cursor.get_ref();
    let end = slice.len();

    for i in start..end.saturating_sub(1) {
        if slice[i] == b'\r' && slice[i + 1] == b'\n' {
            let line = std::str::from_utf8(&slice[start..i])
                .map_err(|_| RedisError::Protocol("invalid UTF-8".into()))?
                .to_owned();
            cursor.set_position((i + 2) as u64);
            return Ok(line);
        }
    }
    Err(RedisError::Protocol("incomplete".into()))
}

/// 从帧数组中提取命令参数（所有元素必须是 Bulk）
pub fn frame_to_cmd_args(frame: Frame) -> Result<Vec<Bytes>> {
    match frame {
        Frame::Array(parts) => {
            let mut args = Vec::with_capacity(parts.len());
            for part in parts {
                match part {
                    Frame::Bulk(b) => args.push(b),
                    Frame::Simple(s) => args.push(Bytes::from(s)),
                    _ => {
                        return Err(RedisError::Protocol(
                            "expected bulk string in command".into(),
                        ))
                    }
                }
            }
            Ok(args)
        }
        _ => Err(RedisError::Protocol("expected array frame".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_simple() {
        let f = Frame::Simple("OK".into());
        assert_eq!(f.serialize(), b"+OK\r\n");
    }

    #[test]
    fn test_serialize_bulk() {
        let f = Frame::bulk_str("hello");
        assert_eq!(f.serialize(), b"$5\r\nhello\r\n");
    }

    #[test]
    fn test_serialize_null() {
        assert_eq!(Frame::Null.serialize(), b"$-1\r\n");
    }

    #[test]
    fn test_parse_simple() {
        let mut buf = BytesMut::from("+PONG\r\n");
        let frame = parse_frame(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Simple("PONG".into()));
        assert!(buf.is_empty());
    }

    #[test]
    fn test_parse_integer() {
        let mut buf = BytesMut::from(":42\r\n");
        let frame = parse_frame(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Integer(42));
    }

    #[test]
    fn test_parse_bulk() {
        let mut buf = BytesMut::from("$5\r\nhello\r\n");
        let frame = parse_frame(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Bulk(Bytes::from("hello")));
    }

    #[test]
    fn test_parse_null_bulk() {
        let mut buf = BytesMut::from("$-1\r\n");
        let frame = parse_frame(&mut buf).unwrap().unwrap();
        assert_eq!(frame, Frame::Null);
    }

    #[test]
    fn test_parse_array() {
        let mut buf = BytesMut::from("*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");
        let frame = parse_frame(&mut buf).unwrap().unwrap();
        match frame {
            Frame::Array(parts) => {
                assert_eq!(parts.len(), 3);
                assert_eq!(parts[0], Frame::Bulk(Bytes::from("SET")));
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn test_incomplete_returns_none() {
        let mut buf = BytesMut::from("$5\r\nhel");
        let result = parse_frame(&mut buf).unwrap();
        assert!(result.is_none());
    }
}
