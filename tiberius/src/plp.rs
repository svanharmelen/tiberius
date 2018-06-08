//! Partially Length-Prefixed types handling

use byteorder::{LittleEndian, ReadBytesExt};
use futures::{Async, Poll};

use Error;

/// Mode for type reader.
#[derive(Debug, Clone, Copy)]
pub enum ReadTyMode {
    /// Fixed-size type with given size
    FixedSize(usize),
    /// Partially length-prefixed type
    Plp,
}

impl ReadTyMode {
    /// Determine the mode automatically from size
    pub fn auto(size: usize) -> Self {
        if size < 0xffff {
            ReadTyMode::FixedSize(size)
        } else {
            ReadTyMode::Plp
        }
    }
}

/// A partially read type
#[derive(Debug)]
pub struct ReadTyState {
    mode: ReadTyMode,
    data: Option<Vec<u8>>,
    chunk_data_left: usize,
}

impl ReadTyState {
    /// Initialize a type reader
    pub fn new(mode: ReadTyMode) -> Self {
        ReadTyState {
            mode,
            data: None,
            chunk_data_left: 0,
        }
    }

    /// Read data stream as Plain or PLP
    ///
    /// Returns bytes read or `None` if the value turned out to be NULL
    pub fn read(&mut self, input: &mut impl ReadBytesExt) -> Poll<Option<Vec<u8>>, Error> {
        // If we did not read anything yet, initialize the reader.
        if self.data.is_none() {
            let size = match self.mode {
                ReadTyMode::FixedSize(_) => input.read_u16::<LittleEndian>()? as u64,
                ReadTyMode::Plp => input.read_u64::<LittleEndian>()?,
            };

            self.data = match size {
                0xffffffffffffffff => None, // NULL value
                0xfffffffffffffffe => Some(Vec::new()), // unknown size
                len => Some(Vec::with_capacity(len as usize)), // given size
            };

            // If this is not PLP, treat everything as a single chunk.
            if let ReadTyMode::FixedSize(_) = self.mode {
                self.chunk_data_left = size as usize;
            }
        }

        // If there is a buffer, we have something to read.
        if let Some(ref mut buf) = self.data {
            loop {
                if self.chunk_data_left == 0 {
                    // We have no chunk. Start a new one.
                    let chunk_size = match self.mode {
                        ReadTyMode::FixedSize(_) => 0,
                        ReadTyMode::Plp => input.read_u32::<LittleEndian>()? as usize,
                    };
                    if chunk_size == 0 {
                        break // found a sentinel, we're done
                    } else {
                        self.chunk_data_left = chunk_size
                    }
                } else {
                    // Just read a byte
                    let byte = input.read_u8()?;
                    self.chunk_data_left -= 1;
                    buf.push(byte);
                }
            }
        }

        // If we're here, we're done reading.
        Ok(Async::Ready(self.data.take()))
    }
}