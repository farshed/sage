use std::io::{self, Read, Write};

pub const READY: u8 = 0;
pub const DATA: u8 = 1;
pub const DONE: u8 = 2;
pub const ERROR: u8 = 3;
pub const METADATA: u8 = 4;
pub const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;

pub fn encode(kind: u8, data: &[u8]) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(5 + data.len());
    write_frame(&mut bytes, kind, data)?;
    Ok(bytes)
}

#[cfg(feature = "async")]
pub async fn read_async(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> io::Result<Option<(u8, Vec<u8>)>> {
    use tokio::io::AsyncReadExt;
    let mut header = [0; 5];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    let length = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Frame is too large",
        ));
    }
    let mut data = vec![0; length];
    reader.read_exact(&mut data).await?;
    Ok(Some((header[0], data)))
}

pub fn read_request(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 5];
    if reader.read(&mut header[..1])? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..])?;
    let length = u32::from_le_bytes(header[1..].try_into().unwrap()) as usize;
    if header[0] != DATA || length > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid request frame",
        ));
    }
    let mut data = vec![0; length];
    reader.read_exact(&mut data)?;
    Ok(Some(data))
}

pub fn write_frame(writer: &mut impl Write, kind: u8, data: &[u8]) -> io::Result<()> {
    if data.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Frame is too large",
        ));
    }
    writer.write_all(&[kind])?;
    writer.write_all(&(data.len() as u32).to_le_bytes())?;
    writer.write_all(data)?;
    writer.flush()
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_multiple_requests_and_clean_eof() {
        let mut bytes = Vec::new();
        write_frame(&mut bytes, DATA, b"first").unwrap();
        write_frame(&mut bytes, DATA, b"second").unwrap();
        let mut reader = Cursor::new(bytes);
        assert_eq!(read_request(&mut reader).unwrap().unwrap(), b"first");
        assert_eq!(read_request(&mut reader).unwrap().unwrap(), b"second");
        assert!(read_request(&mut reader).unwrap().is_none());
    }

    #[test]
    fn rejects_truncated_and_oversized_frames() {
        for bytes in [
            vec![DATA, 1],
            vec![DATA, 2, 0, 0, 0, 1],
            vec![DATA, 255, 255, 255, 255],
        ] {
            assert!(read_request(&mut Cursor::new(bytes)).is_err());
        }
    }
}
