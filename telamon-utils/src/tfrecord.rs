extern crate byteorder;
extern crate crc;

use self::byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use self::crc::crc32::checksum_castagnoli;
use std::io::{Read, Write};

/// The error type for errors occuring while reading a tfrecord file.
#[derive(Debug, Fail)]
pub enum ReadError {
    /// An I/O error occured.
    #[fail(display = "{}", _0)]
    IOError(#[cause] ::std::io::Error),
    /// The underlying data was shorter than advertised in the
    /// header's length field. If this happens because the end-of-file
    /// was reached, an I/O error will be raised instead.
    #[fail(display = "truncated record")]
    TruncatedRecord,
    /// Either the header or the data was corrupted and failed the CRC
    /// check.
    #[fail(display = "corrupted record")]
    CorruptedRecord,
}

/// For usage with ? when creating `ReadError`s.
impl From<::std::io::Error> for ReadError {
    #[inline]
    fn from(error: ::std::io::Error) -> ReadError {
        ReadError::IOError(error)
    }
}

/// The error type for errors occuring while writing a tfrecord file.
#[derive(Debug, Fail)]
pub enum WriteError {
    /// An I/O error occured.
    #[fail(display = "{}", _0)]
    IOError(#[cause] ::std::io::Error),
}

/// For usage with ? when creating `WriteError`s.
impl From<::std::io::Error> for WriteError {
    fn from(error: ::std::io::Error) -> WriteError {
        WriteError::IOError(error)
    }
}

/// Compute a masked CRC32. See module documentation for details.
fn masked_crc32(bytes: &[u8]) -> u32 {
    // https://www.tensorflow.org/api_guides/python/python_io
    let crc = checksum_castagnoli(bytes);
    ((crc >> 15) | (crc << 17)).wrapping_add(0xa282ead8u32)
}

pub trait RecordReader: Read {
    /// Read a single record. Returns `None` if no data is available.
    fn try_read_record(&mut self) -> Result<Option<Vec<u8>>, ReadError> {
        let len = {
            let mut len_bytes = [0u8; 8];
            let nread = self.read(&mut len_bytes)?;
            if nread != len_bytes.len() {
                if nread == 0 {
                    return Ok(None);
                } else {
                    return Err(ReadError::IOError(::std::io::Error::new(
                        ::std::io::ErrorKind::UnexpectedEof,
                        "failed to fill whole buffer",
                    )));
                }
            }
            if self.read_u32::<LittleEndian>()? != masked_crc32(&len_bytes) {
                return Err(ReadError::CorruptedRecord);
            }
            // We `unwrap` here because reading from the on-stack
            // buffer cannnot fail.
            len_bytes.as_ref().read_u64::<LittleEndian>().unwrap()
        };

        let mut record_bytes = Vec::with_capacity(len as usize);
        let nread = self.take(len).read_to_end(&mut record_bytes)? as u64;
        if nread != len {
            return Err(ReadError::TruncatedRecord);
        }
        if self.read_u32::<LittleEndian>()? != masked_crc32(&record_bytes) {
            return Err(ReadError::CorruptedRecord);
        }
        Ok(Some(record_bytes))
    }

    /// Read a single record.
    fn read_record(&mut self) -> Result<Vec<u8>, ReadError> {
        self.try_read_record().and_then(|record| {
            record.ok_or(ReadError::IOError(::std::io::Error::new(
                ::std::io::ErrorKind::UnexpectedEof,
                "failed to read any data",
            )))
        })
    }
}

impl<R: Read + ?Sized> RecordReader for R {}

pub trait RecordWriter: Write {
    fn write_record(&mut self, bytes: &[u8]) -> Result<(), WriteError> {
        // We use a temporary buffer on the stack for the header
        // because we need to compute its crc32. We `unwrap` here
        // because writing to the on-stack buffer cannot fail.
        let mut len_bytes = [0u8; 8];
        len_bytes
            .as_mut()
            .write_u64::<LittleEndian>(bytes.len() as u64)
            .unwrap();

        self.write_all(&len_bytes)?;
        self.write_u32::<LittleEndian>(masked_crc32(&len_bytes))?;
        self.write_all(bytes)?;
        self.write_u32::<LittleEndian>(masked_crc32(bytes))?;
        Ok(())
    }
}

impl<W: Write + ?Sized> RecordWriter for W {}
