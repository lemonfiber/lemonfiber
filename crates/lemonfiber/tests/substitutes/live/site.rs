//! The one edge an arm fakes: a Newznab indexer site, answering every search with the one
//! release the arm's request is for and serving that release's NZB, and writing down
//! each NZB it served, which is the first proof of the grab.

use std::fmt::Write as _;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};

/// The categories the site files its releases under, as Newznab numbers them.
const CATEGORIES: [(&str, &str); 4] = [
    ("2000", "Movies"),
    ("3000", "Audio"),
    ("5000", "TV"),
    ("7000", "Books"),
];

/// The key the site answers to, which the operator gives the indexer that searches it.
pub const KEY: &str = "fixture-site-key";

/// The one release the site holds.
#[derive(Debug, Clone)]
pub struct Release {
    /// Its title, as a release group names it.
    pub title: String,
    /// The Newznab category it is filed under.
    pub category: &'static str,
}

/// The site, listening.
pub struct Site {
    /// The port it answers on, on every address of this machine.
    pub port: u16,
    /// Every NZB it served, by the release's id.
    served: Arc<Mutex<Vec<String>>>,
}

/// The release's size in bytes.
const SIZE: u64 = 1_073_741_824;

/// The size of one article of it, as a Usenet poster splits a file; a download client
/// refuses an article far larger as malformed.
const ARTICLE: u64 = 768_000;

/// The id the site names its release by.
const ID: &str = "lemonfiber-arm-release";

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

    /// Whether the site has served its release's NZB.
    #[must_use]
    pub fn grabbed(&self) -> bool {
        self.served
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|one| one == ID)
    }
}

/// Answer one request on `stream`: the capabilities, a search, or the NZB.
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
    let (status, kind, body) = if target.starts_with(&format!("/nzb/{ID}")) {
        served
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(ID.to_owned());
        ("200 OK", "application/x-nzb", nzb(release))
    } else if !target.contains(&format!("apikey={KEY}")) {
        ("200 OK", "application/xml", refused())
    } else if target.contains("t=caps") {
        ("200 OK", "application/xml", caps())
    } else {
        ("200 OK", "application/rss+xml", found(release, &host))
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

/// What the site answers a key it does not hold.
fn refused() -> String {
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?><error code=\"100\" description=\"Incorrect user credentials\"/>".to_owned()
}

/// What the site can be searched for.
fn caps() -> String {
    let categories = CATEGORIES
        .iter()
        .fold(String::new(), |mut held, (id, name)| {
            let _ = write!(held, "<category id=\"{id}\" name=\"{name}\"/>");
            held
        });
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
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\" \
         xmlns:newznab=\"http://www.newznab.com/DTD/2010/feeds/attributes/\"><channel>\
         <title>lemonfiber arm</title><item><title>{title}</title><guid>{ID}</guid>\
         <link>http://{host}/nzb/{ID}?apikey={KEY}</link><pubDate>Sun, 01 Nov 2026 00:00:00 +0000</pubDate>\
         <enclosure url=\"http://{host}/nzb/{ID}?apikey={KEY}\" length=\"{size}\" type=\"application/x-nzb\"/>\
         <newznab:attr name=\"category\" value=\"{category}\"/>\
         <newznab:attr name=\"size\" value=\"{size}\"/></item></channel></rss>",
        title = release.title,
        category = release.category,
        size = SIZE,
    )
}

/// The release's NZB: one file, in articles of [`ARTICLE`] bytes that add up to [`SIZE`].
fn nzb(release: &Release) -> String {
    let articles = SIZE.div_ceil(ARTICLE);
    let segments = (1..=articles).fold(String::new(), |mut held, number| {
        let bytes = ARTICLE.min(SIZE - (number - 1) * ARTICLE);
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
