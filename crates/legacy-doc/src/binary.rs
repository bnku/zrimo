//! Checked little-endian byte readers.

use crate::{DocError, Result};

pub(crate) struct ByteCursor<'a> {
    data: &'a [u8],
    position: usize,
    structure: &'static str,
}

impl<'a> ByteCursor<'a> {
    pub(crate) const fn new(data: &'a [u8], structure: &'static str) -> Self {
        Self {
            data,
            position: 0,
            structure,
        }
    }

    pub(crate) const fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn skip(&mut self, length: usize) -> Result<()> {
        self.take(length).map(|_| ())
    }

    pub(crate) fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(DocError::OutOfBounds {
                structure: self.structure,
                offset: self.position,
                end: usize::MAX,
                available: self.data.len(),
            })?;
        if end > self.data.len() {
            return Err(DocError::OutOfBounds {
                structure: self.structure,
                offset: self.position,
                end,
                available: self.data.len(),
            });
        }
        let bytes = &self.data[self.position..end];
        self.position = end;
        Ok(bytes)
    }
}

pub(crate) fn checked_slice<'a>(
    data: &'a [u8],
    offset: u32,
    length: u32,
    structure: &'static str,
) -> Result<&'a [u8]> {
    let start = usize::try_from(offset).map_err(|_| DocError::OutOfBounds {
        structure,
        offset: usize::MAX,
        end: usize::MAX,
        available: data.len(),
    })?;
    let length = usize::try_from(length).map_err(|_| DocError::OutOfBounds {
        structure,
        offset: start,
        end: usize::MAX,
        available: data.len(),
    })?;
    let end = start.checked_add(length).ok_or(DocError::OutOfBounds {
        structure,
        offset: start,
        end: usize::MAX,
        available: data.len(),
    })?;
    data.get(start..end).ok_or(DocError::OutOfBounds {
        structure,
        offset: start,
        end,
        available: data.len(),
    })
}
