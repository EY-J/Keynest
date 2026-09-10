//! Bounded byte-stream frames for Windows local IPC. No logging or disk writes.
use std::io::{self, Read, Write};
use zeroize::Zeroizing;

use super::protocol::MAX_MESSAGE_BYTES;

pub(crate) fn read_frame(reader: &mut impl Read) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut prefix = [0; 4];
    reader.read_exact(&mut prefix)?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length == 0 || length > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid Autofill frame length",
        ));
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn write_frame(writer: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid Autofill frame length",
        ));
    }
    writer.write_all(&(bytes.len() as u32).to_le_bytes())?;
    writer.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragmented_frames_round_trip_and_leave_the_next_frame_unread() {
        struct Fragmented<R>(R);
        impl<R: Read> Read for Fragmented<R> {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                let length = bytes.len().min(2);
                self.0.read(&mut bytes[..length])
            }
        }
        let mut wire = Vec::new();
        write_frame(&mut wire, b"fixture-one").unwrap();
        write_frame(&mut wire, b"fixture-two").unwrap();
        assert_eq!(&wire[..4], &11_u32.to_le_bytes());
        let mut reader = Fragmented(wire.as_slice());
        assert_eq!(&**read_frame(&mut reader).as_ref().unwrap(), b"fixture-one");
        assert_eq!(&**read_frame(&mut reader).as_ref().unwrap(), b"fixture-two");
    }

    #[test]
    fn empty_oversized_and_truncated_frames_are_rejected() {
        for length in [0_u32, MAX_MESSAGE_BYTES as u32 + 1, u32::MAX] {
            assert_eq!(
                read_frame(&mut length.to_le_bytes().as_slice())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
        for wire in [vec![], vec![1], vec![1, 0, 0], vec![2, 0, 0, 0, b'a']] {
            assert_eq!(
                read_frame(&mut wire.as_slice()).unwrap_err().kind(),
                io::ErrorKind::UnexpectedEof
            );
        }
        for bytes in [vec![], vec![0; MAX_MESSAGE_BYTES + 1]] {
            let mut output = Vec::new();
            assert!(write_frame(&mut output, &bytes).is_err());
            assert!(output.is_empty());
        }
        let mut wire = Vec::new();
        write_frame(&mut wire, &vec![b'x'; MAX_MESSAGE_BYTES]).unwrap();
        assert_eq!(
            read_frame(&mut wire.as_slice()).unwrap().len(),
            MAX_MESSAGE_BYTES
        );
    }
}
