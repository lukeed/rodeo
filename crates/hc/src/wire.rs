use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub const CONTROL: u32 = 0;
const HEADER: usize = 9;
const MAX_PAYLOAD: usize = 16 << 20;
const MAX_HELLO: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Data,
    Credit,
    Close,
    Resize,
    Focus,
    Facts,
    AutomaticInput,
    Other(u8),
}

impl Kind {
    const fn code(self) -> u8 {
        match self {
            Self::Data => 0,
            Self::Credit => 1,
            Self::Close => 2,
            Self::Resize => 3,
            Self::Focus => 4,
            Self::Facts => 5,
            Self::AutomaticInput => 6,
            Self::Other(code) => code,
        }
    }

    const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Data,
            1 => Self::Credit,
            2 => Self::Close,
            3 => Self::Resize,
            4 => Self::Focus,
            5 => Self::Facts,
            6 => Self::AutomaticInput,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub channel: u32,
    pub kind: Kind,
    pub payload: Bytes,
}

impl Frame {
    pub fn new(channel: u32, kind: Kind, payload: impl Into<Bytes>) -> Self {
        Self {
            channel,
            kind,
            payload: payload.into(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Incoming {
    Hello(Bytes),
    Frame(Frame),
}

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("hc frame of {0} bytes is over the limit")]
    TooLarge(usize),
    #[error("hc hello reply has no end of line")]
    LongHello,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Default)]
pub struct Codec {
    framed: bool,
}

impl Decoder for Codec {
    type Item = Incoming;
    type Error = WireError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Incoming>, WireError> {
        if !self.framed {
            let Some(end) = src.iter().position(|b| *b == b'\n') else {
                return if src.len() > MAX_HELLO {
                    Err(WireError::LongHello)
                } else {
                    Ok(None)
                };
            };
            let line = src.split_to(end + 1).freeze();
            self.framed = true;
            return Ok(Some(Incoming::Hello(line.slice(..end))));
        }
        if src.len() < HEADER {
            return Ok(None);
        }
        let len = u32::from_le_bytes([src[5], src[6], src[7], src[8]]) as usize;
        if len > MAX_PAYLOAD {
            return Err(WireError::TooLarge(len));
        }
        if src.len() < HEADER + len {
            src.reserve(HEADER + len - src.len());
            return Ok(None);
        }
        let channel = src.get_u32_le();
        let kind = Kind::from_code(src.get_u8());
        src.advance(4);
        let payload = src.split_to(len).freeze();
        Ok(Some(Incoming::Frame(Frame {
            channel,
            kind,
            payload,
        })))
    }
}

impl Encoder<Frame> for Codec {
    type Error = WireError;

    fn encode(&mut self, frame: Frame, dst: &mut BytesMut) -> Result<(), WireError> {
        let len = frame.payload.len();
        if len > MAX_PAYLOAD {
            return Err(WireError::TooLarge(len));
        }
        dst.reserve(HEADER + len);
        dst.put_u32_le(frame.channel);
        dst.put_u8(frame.kind.code());
        dst.put_u32_le(u32::try_from(len).map_err(|_| WireError::TooLarge(len))?);
        dst.extend_from_slice(&frame.payload);
        Ok(())
    }
}

pub fn size(cols: u16, rows: u16) -> [u8; 4] {
    let [c0, c1] = cols.to_le_bytes();
    let [r0, r1] = rows.to_le_bytes();
    [c0, c1, r0, r1]
}

pub fn read_size(payload: &[u8]) -> Option<(u16, u16)> {
    let [c0, c1, r0, r1] = payload.try_into().ok()?;
    Some((u16::from_le_bytes([c0, c1]), u16::from_le_bytes([r0, r1])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(frame: Frame) -> BytesMut {
        let mut out = BytesMut::new();
        Codec::default().encode(frame, &mut out).unwrap();
        out
    }

    #[test]
    fn matches_the_documented_layout() {
        let out = encoded(Frame::new(7, Kind::Resize, &b"ab"[..]));
        assert_eq!(&out[..], &[7, 0, 0, 0, 3, 2, 0, 0, 0, b'a', b'b']);
        let out = encoded(Frame::new(258, Kind::AutomaticInput, &b"x"[..]));
        assert_eq!(&out[..], &[2, 1, 0, 0, 6, 1, 0, 0, 0, b'x']);
    }

    #[test]
    fn every_kind_has_its_code() {
        for code in 0..=255 {
            assert_eq!(Kind::from_code(code).code(), code);
        }
        assert_eq!(Kind::from_code(6), Kind::AutomaticInput);
    }

    #[test]
    fn reads_the_hello_line_then_frames_in_one_buffer() {
        let mut src = BytesMut::from(&b"{\"id\":0}\n"[..]);
        src.extend_from_slice(&encoded(Frame::new(0, Kind::Data, &b"{}"[..])));
        src.extend_from_slice(&encoded(Frame::new(3, Kind::Close, Bytes::new()))[..4]);
        let mut codec = Codec::default();
        assert_eq!(
            codec.decode(&mut src).unwrap(),
            Some(Incoming::Hello(Bytes::from_static(b"{\"id\":0}")))
        );
        assert_eq!(
            codec.decode(&mut src).unwrap(),
            Some(Incoming::Frame(Frame::new(0, Kind::Data, &b"{}"[..])))
        );
        assert_eq!(codec.decode(&mut src).unwrap(), None);
        assert_eq!(src.len(), 4);
    }

    #[test]
    fn a_frame_split_anywhere_comes_out_whole() {
        let whole = encoded(Frame::new(9, Kind::Data, &b"hello world"[..]));
        for cut in 0..whole.len() {
            let mut codec = Codec { framed: true };
            let mut src = BytesMut::from(&whole[..cut]);
            assert_eq!(codec.decode(&mut src).unwrap(), None, "cut at {cut}");
            src.extend_from_slice(&whole[cut..]);
            assert_eq!(
                codec.decode(&mut src).unwrap(),
                Some(Incoming::Frame(Frame::new(
                    9,
                    Kind::Data,
                    &b"hello world"[..]
                )))
            );
        }
    }

    #[test]
    fn unknown_kinds_decode_and_oversized_frames_fail() {
        let mut codec = Codec { framed: true };
        let mut huge = BytesMut::from(&[0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff][..]);
        assert!(matches!(
            codec.decode(&mut huge),
            Err(WireError::TooLarge(_))
        ));
        let mut odd = BytesMut::from(&[1, 0, 0, 0, 9, 1, 0, 0, 0, b'z'][..]);
        assert_eq!(
            codec.decode(&mut odd).unwrap(),
            Some(Incoming::Frame(Frame::new(1, Kind::Other(9), &b"z"[..])))
        );
    }

    #[test]
    fn sizes_are_two_little_endian_words() {
        assert_eq!(size(300, 40), [44, 1, 40, 0]);
        assert_eq!(read_size(&size(300, 40)), Some((300, 40)));
        assert_eq!(read_size(&[1, 2, 3]), None);
    }

    #[test]
    fn a_long_hello_without_an_end_fails() {
        let mut src = BytesMut::from(&vec![b'x'; MAX_HELLO + 1][..]);
        assert!(matches!(
            Codec::default().decode(&mut src),
            Err(WireError::LongHello)
        ));
    }
}
