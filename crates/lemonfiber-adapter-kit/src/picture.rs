//! A picture an upstream served, passed on only as a raster image its bytes agree with.
//!
//! A poster or backdrop reaches members' apps and the web, which render it. An SVG can
//! carry script, and a body served under one type can be another, so a picture is passed
//! on only where its media type, without its parameters and in any case, is one of the
//! raster types here and its bytes begin as a file of that type does.

use lemonfiber_ports::service::Image;

/// What the first bytes of a file of one raster type hold.
enum Magic {
    /// These bytes, at the start.
    Starts(&'static [&'static [u8]]),
    /// `RIFF`, four bytes of length, then `WEBP`.
    Riff,
    /// An ISO base media file whose `ftyp` box names one of these brands.
    Brand(&'static [&'static [u8; 4]]),
}

/// Each raster type a picture may be, and how its bytes begin.
const RASTERS: [(&str, Magic); 5] = [
    ("image/jpeg", Magic::Starts(&[&[0xFF, 0xD8, 0xFF]])),
    (
        "image/png",
        Magic::Starts(&[&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]]),
    ),
    ("image/gif", Magic::Starts(&[b"GIF87a", b"GIF89a"])),
    ("image/webp", Magic::Riff),
    ("image/avif", Magic::Brand(&[b"avif", b"avis"])),
];

/// The length of the `ftyp` box's own header: its size, its type, its major brand and
/// its minor version.
const FTYP_HEADER: usize = 16;

impl Magic {
    fn begins(&self, bytes: &[u8]) -> bool {
        match self {
            Self::Starts(prefixes) => prefixes.iter().any(|prefix| bytes.starts_with(prefix)),
            Self::Riff => bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
            Self::Brand(brands) => branded(bytes, brands),
        }
    }
}

/// Whether `bytes` open with an `ftyp` box naming one of `brands`, major or compatible.
fn branded(bytes: &[u8], brands: &[&[u8; 4]]) -> bool {
    if bytes.get(4..8) != Some(b"ftyp") {
        return false;
    }
    let size = bytes
        .get(..4)
        .and_then(|size| <[u8; 4]>::try_from(size).ok())
        .and_then(|size| usize::try_from(u32::from_be_bytes(size)).ok())
        .unwrap_or(0)
        .min(bytes.len());
    let named = |brand: &[u8]| brands.iter().any(|wanted| wanted.as_slice() == brand);
    bytes.get(8..12).is_some_and(named)
        || bytes
            .get(FTYP_HEADER..size)
            .is_some_and(|compatible| compatible.chunks_exact(4).any(named))
}

/// The raster type `content_type` names, without its parameters and in lower case, where
/// it names one of those a picture may be.
#[must_use]
pub fn raster(content_type: &str) -> Option<&'static str> {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    RASTERS
        .iter()
        .find(|(name, _)| *name == essence)
        .map(|(name, _)| *name)
}

/// `bytes` as the picture they are, where `content_type` names a raster type and the bytes
/// begin as a file of that type does; nothing otherwise, an SVG included.
#[must_use]
pub fn image(content_type: Option<&str>, bytes: Vec<u8>) -> Option<Image> {
    let named = raster(content_type?)?;
    let (_, magic) = RASTERS.iter().find(|(name, _)| *name == named)?;
    magic.begins(&bytes).then(|| Image {
        media_type: named.to_owned(),
        bytes,
    })
}

#[cfg(test)]
mod tests;
