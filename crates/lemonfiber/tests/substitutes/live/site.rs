//! The one edge an arm fakes: a Newznab or Torznab indexer site, answering every search
//! with the one release the arm's request is for and serving that release's NZB or torrent,
//! and writing down each one it served, which is the first proof of the grab.

use std::fmt::Write as _;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};

/// A Newznab category the site lists: its number, its name, and the subcategories it lists
/// beneath it, each by number and name.
struct Category {
    id: &'static str,
    name: &'static str,
    subcategories: &'static [(&'static str, &'static str)],
}

/// The categories the site files its releases under, as Newznab numbers them.
const CATEGORIES: [Category; 4] = [
    Category {
        id: "2000",
        name: "Movies",
        subcategories: &[],
    },
    Category {
        id: "3000",
        name: "Audio",
        subcategories: &[],
    },
    Category {
        id: "5000",
        name: "TV",
        subcategories: &[],
    },
    Category {
        id: "7000",
        name: "Books",
        subcategories: &[("7020", "EBook")],
    },
];

/// The key the site answers to, which the operator gives the indexer that searches it.
pub const KEY: &str = "fixture-site-key";

/// What the site's release is fetched as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    /// An NZB, over Usenet, as a Newznab site serves it.
    Nzb,
    /// A torrent, from peers, as a Torznab site serves it.
    Torrent,
}

impl Carried {
    /// The path the site serves the release's file under, before its id.
    const fn path(self) -> &'static str {
        match self {
            Self::Nzb => "/nzb/",
            Self::Torrent => "/torrent/",
        }
    }

    /// The media type the release's file is served as.
    const fn media_type(self) -> &'static str {
        match self {
            Self::Nzb => "application/x-nzb",
            Self::Torrent => "application/x-bittorrent",
        }
    }

    /// The namespace the feed's attributes are written in, and the prefix they carry.
    const fn attributes(self) -> (&'static str, &'static str) {
        match self {
            Self::Nzb => (
                "newznab",
                "http://www.newznab.com/DTD/2010/feeds/attributes/",
            ),
            Self::Torrent => ("torznab", "http://torznab.com/schemas/2015/feed"),
        }
    }
}

/// The one release the site holds.
#[derive(Debug, Clone)]
pub struct Release {
    /// Its title, as a release group names it.
    pub title: String,
    /// The Newznab category it is filed under.
    pub category: &'static str,
    /// Its size in bytes, within what its curator takes for a release of its kind.
    pub size: u64,
    /// What it is fetched as.
    pub carried: Carried,
}

/// The site, listening.
pub struct Site {
    /// The port it answers on, on every address of this machine.
    pub port: u16,
    /// Every NZB or torrent it served, by the release's id.
    served: Arc<Mutex<Vec<String>>>,
}

/// The size of one article of it, as a Usenet poster splits a file; a download client
/// refuses an article far larger as malformed.
const ARTICLE: u64 = 768_000;

/// The id the site names its release by.
const ID: &str = "lemonfiber-arm-release";

/// The length of one piece of the release's torrent.
const PIECE: u64 = 16_777_216;

/// The length of one piece's hash in a torrent.
const HASHED: usize = 20;

/// The peers a search of the site finds sharing the release's torrent, above the one a
/// curator asks for at least.
const SEEDERS: u32 = 10;

/// The tracker the release's torrent names, which nothing in the run announces to.
const TRACKER: &str = "http://tracker.lemonfiber.invalid/announce";

impl Site {
    /// The site holding `release`, answering on a port of its own on every address.
    ///
    /// # Panics
    ///
    /// Where no port can be bound.
    #[must_use]
    pub fn serving(release: Release) -> Self {
        let listener = TcpListener::bind(("0.0.0.0", 0));
        assert!(listener.is_ok(), "{listener:?}");
        let Ok(listener) = listener else {
            unreachable!("asserted");
        };
        let port = listener
            .local_addr()
            .map(|at| at.port())
            .unwrap_or_default();
        let served = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&served);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                answer(stream, &release, &kept);
            }
        });
        Self { port, served }
    }

    /// Whether the site has served its release's NZB or torrent.
    #[must_use]
    pub fn grabbed(&self) -> bool {
        self.served
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|one| one == ID)
    }
}

/// Answer one request on `stream`: the capabilities, a search, or the release's file.
fn answer(mut stream: TcpStream, release: &Release, served: &Mutex<Vec<String>>) {
    let mut line = String::new();
    let mut reading = BufReader::new(&stream);
    if reading.read_line(&mut line).is_err() {
        return;
    }
    let mut host = String::new();
    let mut header = String::new();
    while reading.read_line(&mut header).is_ok_and(|read| read > 2) {
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("host") {
                value.trim().clone_into(&mut host);
            }
        }
        header.clear();
    }
    let target = line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let carried = release.carried;
    let (kind, body) = if target.starts_with(&format!("{}{ID}", carried.path())) {
        served
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(ID.to_owned());
        let file = match carried {
            Carried::Nzb => nzb(release).into_bytes(),
            Carried::Torrent => torrent(release),
        };
        (carried.media_type(), file)
    } else if !target.contains(&format!("apikey={KEY}")) {
        ("application/xml", refused().into_bytes())
    } else if target.contains("t=caps") {
        ("application/xml", caps().into_bytes())
    } else {
        ("application/rss+xml", found(release, &host).into_bytes())
    };
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(&body);
}

/// What the site answers a key it does not hold.
fn refused() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?><error code=\"100\" description=\"Incorrect user credentials\"/>".to_owned()
}

/// What the site can be searched for.
fn caps() -> String {
    let mut categories = String::new();
    for category in CATEGORIES {
        let _ = write!(
            categories,
            "<category id=\"{}\" name=\"{}\">",
            category.id, category.name
        );
        for (id, name) in category.subcategories {
            let _ = write!(categories, "<subcat id=\"{id}\" name=\"{name}\"/>");
        }
        categories.push_str("</category>");
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><caps><server title=\"lemonfiber arm\"/>\
         <limits max=\"100\" default=\"100\"/><searching>\
         <search available=\"yes\" supportedParams=\"q\"/>\
         <tv-search available=\"yes\" supportedParams=\"q,season,ep\"/>\
         <movie-search available=\"yes\" supportedParams=\"q\"/>\
         <audio-search available=\"yes\" supportedParams=\"q\"/>\
         <book-search available=\"yes\" supportedParams=\"q\"/>\
         </searching><categories>{categories}</categories></caps>"
    )
}

/// The one release, as every search made of the site at `host` finds it.
fn found(release: &Release, host: &str) -> String {
    let carried = release.carried;
    let (prefix, namespace) = carried.attributes();
    let shared = match carried {
        Carried::Nzb => String::new(),
        Carried::Torrent => format!(
            "<{prefix}:attr name=\"seeders\" value=\"{SEEDERS}\"/>\
             <{prefix}:attr name=\"peers\" value=\"{SEEDERS}\"/>"
        ),
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\" \
         xmlns:{prefix}=\"{namespace}\"><channel>\
         <title>lemonfiber arm</title><item><title>{title}</title><guid>{ID}</guid>\
         <link>http://{host}{path}{ID}?apikey={KEY}</link><pubDate>Sun, 01 Nov 2026 00:00:00 +0000</pubDate>\
         <enclosure url=\"http://{host}{path}{ID}?apikey={KEY}\" length=\"{size}\" type=\"{kind}\"/>\
         <{prefix}:attr name=\"category\" value=\"{category}\"/>\
         <{prefix}:attr name=\"size\" value=\"{size}\"/>{shared}</item></channel></rss>",
        title = release.title,
        category = release.category,
        size = release.size,
        path = carried.path(),
        kind = carried.media_type(),
    )
}

/// The release's torrent: one file of its size, in pieces of [`PIECE`] bytes, announced to
/// [`TRACKER`]. No peer holds it, so a client given it waits for one.
fn torrent(release: &Release) -> Vec<u8> {
    let pieces = usize::try_from(release.size.div_ceil(PIECE)).unwrap_or_default();
    let hashes: Vec<u8> = (0..pieces * HASHED)
        .map(|at| u8::try_from(at % 251).unwrap_or_default())
        .collect();
    let name = format!("{}.mkv", release.title);
    let mut file = format!(
        "d8:announce{}:{TRACKER}4:infod6:lengthi{}e4:name{}:{name}12:piece lengthi{PIECE}e6:pieces{}:",
        TRACKER.len(),
        release.size,
        name.len(),
        hashes.len(),
    )
    .into_bytes();
    file.extend(hashes);
    file.extend(b"ee");
    file
}

/// The release's NZB: one file, in articles of [`ARTICLE`] bytes that add up to its size.
fn nzb(release: &Release) -> String {
    let size = release.size;
    let articles = size.div_ceil(ARTICLE);
    let segments = (1..=articles).fold(String::new(), |mut held, number| {
        let bytes = ARTICLE.min(size - (number - 1) * ARTICLE);
        let _ = write!(
            held,
            "<segment bytes=\"{bytes}\" number=\"{number}\">{ID}-{number}@lemonfiber.invalid</segment>"
        );
        held
    });
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><nzb xmlns=\"http://www.newzbin.com/DTD/2003/nzb\">\
         <file poster=\"arm@lemonfiber.invalid\" date=\"1793491200\" subject=\"{title} &quot;{title}.mkv&quot; yEnc (1/{articles})\">\
         <groups><group>alt.binaries.lemonfiber</group></groups>\
         <segments>{segments}</segments>\
         </file></nzb>",
        title = release.title,
    )
}
