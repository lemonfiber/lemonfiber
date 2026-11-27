//! A title written into a library as a household keeps one on disk, generated here: a
//! tagged track of silence for music and for an audiobook, and a film's container holding no
//! pictures for movies.

use std::path::{Path, PathBuf};

/// What every fixture is called: its title, and a track's album and artist too.
pub const TITLE: &str = "Lemonfiber Arm Fixture";

/// The year a film fixture is filed under.
const YEAR: u16 = 2026;

/// How many frames of silence a track holds, about a second of it.
const FRAMES: usize = 40;

/// The header of each frame: MPEG-1 Layer III, 128 kbit/s, 44.1 kHz, joint stereo.
const FRAME_HEADER: [u8; 4] = [0xFF, 0xFB, 0x90, 0x64];

/// The length of a frame under that header, the header included.
const FRAME_LENGTH: usize = 417;

/// The file [`TITLE`] is written as beneath `folder`, the library for `media`.
///
/// # Errors
///
/// Where no fixture is written for `media`, or the file cannot be written.
pub fn written(folder: &Path, media: &str) -> Result<PathBuf, String> {
    let (at, bytes) = match media {
        "music" | "audiobooks" => (
            folder
                .join(TITLE)
                .join(TITLE)
                .join(format!("01 - {TITLE}.mp3")),
            track(),
        ),
        "movies" => {
            let named = format!("{TITLE} ({YEAR})");
            (folder.join(&named).join(format!("{named}.mkv")), film())
        }
        other => return Err(format!("no fixture is written for {other}")),
    };
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent).map_err(|why| format!("{}: {why}", parent.display()))?;
    }
    std::fs::write(&at, bytes).map_err(|why| format!("{}: {why}", at.display()))?;
    Ok(at)
}

/// A track of silence tagged with [`TITLE`] as its title, album and artist.
fn track() -> Vec<u8> {
    let frames: Vec<u8> = [*b"TIT2", *b"TALB", *b"TPE1"]
        .into_iter()
        .flat_map(|id| tagged(id, TITLE))
        .chain(tagged(*b"TRCK", "1"))
        .collect();
    let mut written = b"ID3\x03\x00\x00".to_vec();
    written.extend(syncsafe(frames.len()));
    written.extend(frames);
    for _ in 0..FRAMES {
        written.extend(FRAME_HEADER);
        written.resize(written.len() + FRAME_LENGTH - FRAME_HEADER.len(), 0);
    }
    written
}

/// One `ID3v2.3` text frame `id` holding `text`.
fn tagged(id: [u8; 4], text: &str) -> Vec<u8> {
    let length = u32::try_from(text.len() + 1).unwrap_or(u32::MAX);
    let mut frame = id.to_vec();
    frame.extend(length.to_be_bytes());
    frame.extend([0, 0, 0]);
    frame.extend(text.as_bytes());
    frame
}

/// `length` as an `ID3v2` tag writes its size: seven bits to a byte.
fn syncsafe(length: usize) -> [u8; 4] {
    [21, 14, 7, 0].map(|shift| u8::try_from((length >> shift) & 0x7F).unwrap_or(0))
}

/// A Matroska film a second long, with one video track and no pictures.
fn film() -> Vec<u8> {
    let header = element(
        &[0x1A, 0x45, 0xDF, 0xA3],
        &[
            element(&[0x42, 0x86], &[1]),
            element(&[0x42, 0xF7], &[1]),
            element(&[0x42, 0xF2], &[4]),
            element(&[0x42, 0xF3], &[8]),
            element(&[0x42, 0x82], b"matroska"),
            element(&[0x42, 0x87], &[4]),
            element(&[0x42, 0x85], &[2]),
        ]
        .concat(),
    );
    let info = element(
        &[0x15, 0x49, 0xA9, 0x66],
        &[
            element(&[0x2A, 0xD7, 0xB1], &1_000_000_u32.to_be_bytes()),
            element(&[0x44, 0x89], &1000.0_f32.to_be_bytes()),
            element(&[0x4D, 0x80], TITLE.as_bytes()),
            element(&[0x57, 0x41], TITLE.as_bytes()),
        ]
        .concat(),
    );
    let video = element(
        &[0xE0],
        &[element(&[0xB0], &[16]), element(&[0xBA], &[16])].concat(),
    );
    let tracks = element(
        &[0x16, 0x54, 0xAE, 0x6B],
        &element(
            &[0xAE],
            &[
                element(&[0xD7], &[1]),
                element(&[0x73, 0xC5], &[1]),
                element(&[0x83], &[1]),
                element(&[0x86], b"V_MPEG4/ISO/AVC"),
                video,
            ]
            .concat(),
        ),
    );
    let segment = element(&[0x18, 0x53, 0x80, 0x67], &[info, tracks].concat());
    [header, segment].concat()
}

/// One EBML element `id` holding `body`, its size written in eight bytes.
fn element(id: &[u8], body: &[u8]) -> Vec<u8> {
    let size = u64::try_from(body.len()).unwrap_or(0).to_be_bytes();
    let mut written = id.to_vec();
    written.push(0x01);
    written.extend(&size[1..]);
    written.extend(body);
    written
}
