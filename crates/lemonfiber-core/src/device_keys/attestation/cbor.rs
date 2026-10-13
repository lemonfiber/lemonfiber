//! Reading the CBOR an App Attest attestation object is written in, strictly.
//!
//! One header at a time, each of the kind the object holds there, with a length given
//! up front: an indefinite length, a segmented string, a length past what is left of the
//! input, or anything after the object is no object at all.

use ciborium_io::Read as _;
use ciborium_ll::{Decoder, Header};

/// A reader over one CBOR document.
pub(crate) struct Cbor<'a> {
    /// Where it is read from.
    decoder: Decoder<&'a [u8]>,
    /// How many bytes the document holds.
    length: usize,
}

impl<'a> Cbor<'a> {
    /// A reader over `bytes`.
    pub(crate) fn over(bytes: &'a [u8]) -> Self {
        Self {
            decoder: Decoder::from(bytes),
            length: bytes.len(),
        }
    }

    /// How many entries the map read next holds.
    pub(crate) fn map(&mut self) -> Option<usize> {
        match self.decoder.pull().ok()? {
            Header::Map(Some(entries)) => Some(entries),
            _ => None,
        }
    }

    /// How many items the array read next holds.
    pub(crate) fn array(&mut self) -> Option<usize> {
        match self.decoder.pull().ok()? {
            Header::Array(Some(items)) => Some(items),
            _ => None,
        }
    }

    /// The text string read next.
    pub(crate) fn text(&mut self) -> Option<String> {
        match self.decoder.pull().ok()? {
            Header::Text(Some(length)) => String::from_utf8(self.taken(length)?).ok(),
            _ => None,
        }
    }

    /// The byte string read next.
    pub(crate) fn bytes(&mut self) -> Option<Vec<u8>> {
        match self.decoder.pull().ok()? {
            Header::Bytes(Some(length)) => self.taken(length),
            _ => None,
        }
    }

    /// Whether every byte of the document has been read.
    pub(crate) fn finished(&mut self) -> bool {
        self.decoder.offset() == self.length
    }

    /// The next `length` bytes, where that many are left.
    fn taken(&mut self, length: usize) -> Option<Vec<u8>> {
        let left = self.length.checked_sub(self.decoder.offset())?;
        if length > left {
            return None;
        }
        let mut taken = vec![0; length];
        self.decoder.read_exact(&mut taken).ok()?;
        Some(taken)
    }
}

#[cfg(test)]
mod tests;
