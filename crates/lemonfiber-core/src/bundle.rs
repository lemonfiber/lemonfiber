//! What may leave the machine in a support bundle, and what must not.
//!
//! An operator who cannot diagnose their own stack posts on a forum, and what they need to
//! share is genuinely sensitive: configuration holding API keys, logs holding indexer URLs
//! with the key inside them, a VPN's credentials. What they actually do is paste a config
//! file with the parts they *recognised* as secret taken out — and the ones people miss
//! are exactly the ones that matter, because a key inside a query string in a log line
//! does not look like a key.
//!
//! So nothing is taken out here. Things are *let through*. Every field not named safe is
//! replaced, which is the whole difference between a bundle that leaks whatever nobody
//! anticipated and one that leaks nothing: a list of what is secret is only as good as the
//! last time somebody thought about it, and new fields arrive with every service release.
//! Being wrong about the allow-list costs a missing diagnostic; being wrong about a
//! deny-list costs a published credential.
//!
//! A replaced value keeps a stand-in derived from it, so the same key reads the same
//! everywhere in one bundle — someone helping can see that two services point at the same
//! account without ever seeing which one. The derivation is salted per bundle, so that
//! likeness holds inside a bundle and says nothing across two.
//!
//! Everything here is pure, which is deliberate: this is the one place in lemonfiber where
//! a bug publishes a secret, so all of it runs in a test with no filesystem, no services
//! and nothing to stand up. The collecting and the writing live above it.

use std::collections::hash_map::DefaultHasher;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use serde::Serialize;

use crate::ports::random::Random;

/// How many bytes of salt a bundle's stand-ins are derived with.
///
/// Sixteen is far past guessing, and the salt is the whole of what makes a stand-in
/// meaningless outside its own bundle — the derivation carries no secret of its own.
pub const SALT_BYTES: usize = 16;

/// The settings whose values may be shown as they are.
///
/// Named one at a time, deliberately. Everything absent from this list is replaced,
/// including settings that do not exist yet — which is the property being bought: a
/// service that adds a field tomorrow is redacted tomorrow, with nobody having to notice.
///
/// What is here is what a person helping actually needs: which provider, which mode, which
/// ports, which identities the containers run as, and whether a credential was ever
/// validated — never the credential. The two host-shaped values, the data root and the
/// Usenet host, are here because a stack cannot be diagnosed without knowing where its
/// files are and who it talks to; both are offered for redaction to an operator who would
/// rather not say. An indexer's address is here for the same reason, and is the case that
/// makes the query-string rule below necessary rather than tidy: the address is worth
/// sharing and the key riding in its query is not, and they arrive as one string.
const SHOWN: [&str; 15] = [
    "DATA_ROOT",
    "PUID",
    "PGID",
    "TZ",
    "VPN_PROVIDER",
    "VPN_PORT_FORWARDING",
    "JELLYFIN_MODE",
    "LEMONFIBER_USENET",
    "LEMONFIBER_TORRENT",
    "LEMONFIBER_IP_ECHO",
    "INDEXER_URL",
    "USENET_HOST",
    "USENET_PORT",
    "USENET_TLS",
    "USENET_VALIDATED",
];

/// Settings whose *names* end in something a service uses for a validated flag rather
/// than for a secret — `INDEXER_VALIDATED` is safe where `INDEXER_APIKEY` is not, and both
/// begin the same way.
const SHOWN_SUFFIX: &str = "_VALIDATED";

/// Whether a setting's value may be shown as it is.
#[must_use]
pub fn shown(name: &str) -> bool {
    let name = name.trim();
    SHOWN.contains(&name) || name.ends_with(SHOWN_SUFFIX)
}

/// The stand-ins one bundle uses for the values it will not show.
///
/// Salted per bundle so the likeness a reader can see — these two services point at the
/// same account — holds only inside the bundle it was read from. Without that, the same
/// key would produce the same mark in every bundle ever posted, which is a fingerprint
/// that outlives the redaction it was meant to be.
pub struct Marks {
    salt: Vec<u8>,
}

impl Marks {
    /// Stand-ins for one bundle, or nothing where the machine could not provide the
    /// randomness — which is not something to paper over with a fixed salt, because a
    /// predictable stand-in is a way back to the value it stands for.
    #[must_use]
    pub fn new(random: &dyn Random) -> Option<Self> {
        Some(Self {
            salt: random.bytes(SALT_BYTES)?,
        })
    }

    /// What `value` is shown as instead of itself.
    ///
    /// Four characters, which is what makes it readable in a config file an operator is
    /// scanning; it carries at most sixteen bits about a value nobody can invert without
    /// the salt, and nothing is ever checked against it — it is a label, not a proof.
    #[must_use]
    pub fn of(&self, value: &str) -> String {
        let mut hasher = DefaultHasher::new();
        self.salt.hash(&mut hasher);
        value.hash(&mut hasher);
        format!("<redacted:{:04x}>", hasher.finish() & 0xffff)
    }
}

/// Whether media filenames are shown as they are.
///
/// Replaced unless asked otherwise. A library's contents are not a credential, but they
/// are the one thing in a bundle that says something about the person rather than about
/// the machine, and replacing them costs a diagnostic a reader can still follow — the
/// marks keep two mentions of one file recognisable as one file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum Filenames {
    /// Replaced by their marks.
    #[default]
    Replaced,
    /// Shown, because the operator asked for them.
    Shown,
}

/// How a bundle was made: what was bounded, what was replaced, and what its operator asked
/// to have shown as it is.
///
/// Carried in the bundle rather than known only to the command that wrote it, because the
/// person who reads one is usually not the person who chose any of this.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Terms {
    /// How much of the logs was taken, said as it would be said aloud.
    pub window: String,
    /// Whether media filenames were shown.
    pub filenames: Filenames,
    /// The settings the operator asked to have shown as they are.
    pub revealed: Vec<String>,
}

impl Terms {
    /// Whether `name` is a setting the operator asked to have shown.
    #[must_use]
    pub fn reveals(&self, name: &str) -> bool {
        let name = name.trim();
        self.revealed.iter().any(|field| field == name)
    }

    /// What was done to this bundle, for its own first page.
    ///
    /// An extract of logs that does not say what it is an extract of reads as the whole
    /// story, and a bundle holding one credential in the clear has to say so where nobody
    /// can miss it. That notice is what makes the consent mean anything once the file is
    /// somewhere public and the operator is not there to explain it.
    fn stated(&self) -> String {
        let filenames = match self.filenames {
            Filenames::Replaced => "replaced",
            Filenames::Shown => "shown, because this bundle's operator asked",
        };
        let said = format!("\nLogs: {}.\nMedia filenames: {filenames}.\n", self.window);
        if self.revealed.is_empty() {
            return said;
        }
        let listed = self.revealed.iter().fold(String::new(), |mut page, field| {
            let _ = writeln!(page, "  {field}");
            page
        });
        format!(
            "{said}\nSHOWN AS THEY ARE, BECAUSE THIS BUNDLE'S OPERATOR ASKED:\n{listed}\
             This bundle holds a credential in the clear. Treat it as the credential it holds.\n"
        )
    }
}

/// A settings file as it may be shared: every value not named safe replaced by its mark.
///
/// The names stay, always. A reader helping needs to know that an indexer key is set at
/// all — an empty setting and a set one are different faults — and the name of a setting
/// is not a secret in any stack anyone has ever run.
#[must_use]
pub fn settings(body: &str, marks: &Marks, terms: &Terms) -> String {
    body.lines()
        .map(|line| setting(line, marks, terms))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One settings line, shown or marked.
fn setting(line: &str, marks: &Marks, terms: &Terms) -> String {
    let Some((name, value)) = line.split_once('=') else {
        return line.to_owned();
    };
    if value.trim().is_empty() {
        return line.to_owned();
    }
    // Named by its operator, one field at a time and confirmed on the same breath. The
    // allow-list decides what is safe to share; this is somebody deciding, for one field,
    // that they would rather be understood than careful — which the first page then says
    // out loud, because they will not be there when it is read.
    if terms.reveals(name) {
        return line.to_owned();
    }
    if shown(name) {
        return format!("{name}={}", url(value.trim(), marks));
    }
    format!("{name}={}", marks.of(value.trim()))
}

/// Free text as it may be shared: a log line, a finding, anything with no field names for
/// an allow-list to work from.
///
/// The allow-list cannot help here, so this is the narrower rule the same reasoning gives.
/// A query string goes wholesale — that is where the key nobody spotted actually lives,
/// riding inside something that looks like an address. Then any run long and dense enough
/// to read as a key is replaced whatever it sits next to, using the very tokeniser the
/// scan uses, so that what this writes is what the scan will accept.
///
/// Not the same rule as [`crate::config::store::withheld_text`], which faults already pass
/// through on their way to being remembered: that one reads names and withholds what
/// follows them, and this one reads values and knows no names at all. Two rules, two jobs.
/// What both leave is a key broken up by characters no key alphabet uses — several short
/// runs to the tokeniser, and to the scan as well.
#[must_use]
pub fn prose(text: &str, marks: &Marks, terms: &Terms) -> String {
    text.lines()
        .map(|line| said(line, marks, terms))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One line of free text, split on spaces rather than on whitespace so it comes back spaced
/// as it was written — a log line read by a person is half indentation.
fn said(line: &str, marks: &Marks, terms: &Terms) -> String {
    line.split(' ')
        .map(|word| spoken(word, marks, terms))
        .collect::<Vec<_>>()
        .join(" ")
}

/// One word of free text.
fn spoken(word: &str, marks: &Marks, terms: &Terms) -> String {
    if terms.filenames == Filenames::Replaced && names_media(word) {
        return marks.of(word);
    }
    match word.split_once('?') {
        // A question mark with parameters after it is a query string wherever it turns up;
        // one without is somebody asking a question in a log line. The address in front of
        // it still goes through the key rule, because a path can carry a key too, and
        // anything left whole here is something the scan would refuse the bundle over.
        Some((address, query)) if query.contains('=') => {
            format!("{}?{}", keys(address, marks), marks.of(query))
        }
        _ => keys(word, marks),
    }
}

/// Every key-shaped run in a word, replaced by its mark.
fn keys(word: &str, marks: &Marks) -> String {
    let mut spoken = String::new();
    let mut run = String::new();
    for character in word.chars() {
        if key_shaped(character) {
            run.push(character);
            continue;
        }
        spoken.push_str(&spelled(&run, marks));
        run.clear();
        spoken.push(character);
    }
    spoken.push_str(&spelled(&run, marks));
    spoken
}

/// One run, marked where it reads as a key and left alone where it does not.
fn spelled(run: &str, marks: &Marks) -> String {
    if reads_as_key(run) {
        return marks.of(run);
    }
    run.to_owned()
}

/// What a media file is called at the end.
///
/// The name rather than the path: what makes a filename worth replacing is the title in
/// it, and the title is in the name whatever directory it is sitting in.
const MEDIA: [&str; 8] = [
    ".mkv", ".mp4", ".avi", ".m4v", ".mov", ".srt", ".nfo", ".iso",
];

/// Whether a word names a media file.
fn names_media(word: &str) -> bool {
    let word = word.to_ascii_lowercase();
    MEDIA.iter().any(|extension| word.ends_with(extension))
}

/// A URL with everything after its question mark marked.
///
/// The shape that catches people out: an indexer's address is worth sharing and the key
/// riding in its query string is not, and the two arrive as one string. So a value that is
/// allowed through keeps its address and loses its parameters wholesale — a query nobody
/// reads is a smaller loss than a key everybody can.
fn url(value: &str, marks: &Marks) -> String {
    match value.split_once('?') {
        None => value.to_owned(),
        Some((address, query)) => format!("{address}?{}", marks.of(query)),
    }
}

/// One file inside a bundle: the name it will carry, and what it holds.
///
/// Held in memory rather than written as it is gathered, because everything is read back
/// before anything is written. A bundle that had already put one file on disk when it
/// found a credential in the next would have to be unwritten, and unwriting is the kind of
/// thing that half-works.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Piece {
    /// What it is called inside the bundle.
    pub name: String,
    /// What it holds, already redacted.
    pub body: String,
}

/// What a reader needs to know before reading a word of the bundle.
///
/// An operator pasting last week's bundle into this week's thread is the commonest way one
/// of those threads goes wrong, and nothing in the contents tells either of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Taken {
    /// The lemonfiber that wrote it.
    pub lemonfiber: String,
    /// The stack it was written from.
    pub stack: String,
    /// When, as a service writes a moment.
    pub at: String,
}

/// Everything gathered for a bundle, and everything that could not be.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Contents {
    /// The files, in the order a reader would want them.
    pub pieces: Vec<Piece>,
    /// What could not be collected, named.
    ///
    /// Named rather than passed over: a bundle from a machine whose diagnostics will not
    /// run is exactly the bundle worth having, and a gap nobody mentions reads as an
    /// absence of trouble rather than as an absence of information.
    pub missing: Vec<String>,
    /// Where and when it came from.
    pub taken: Taken,
    /// How it was made, and what its operator chose.
    pub terms: Terms,
}

/// What every bundle says about its own redaction, so a reader knows what the marks in it
/// mean without having to be told separately.
const NOTE: &str = "Every value not named safe has been replaced. A replacement reads the same wherever the same value appeared in this bundle, and means nothing in any other.";

/// The name the bundle's own first page carries.
pub const MANIFEST: &str = "README.txt";

impl Contents {
    /// The bundle's first page: what it is, where it came from, what is in it, and what is
    /// not. Written into the bundle rather than printed once, because the person who reads
    /// it is usually not the person who made it.
    #[must_use]
    pub fn manifest(&self) -> String {
        let holds = self.pieces.iter().fold(String::new(), |mut page, piece| {
            let _ = writeln!(page, "  {}", piece.name);
            page
        });
        let gaps = self.gaps();
        let terms = self.terms.stated();
        let Taken {
            lemonfiber,
            stack,
            at,
        } = &self.taken;
        format!(
            "lemonfiber support bundle\n\nlemonfiber {lemonfiber}\nstack {stack}\ntaken {at}\n\nHolds:\n{holds}{gaps}{terms}\n{NOTE}\n"
        )
    }

    /// What could not be read, where anything could not — and nothing at all where
    /// everything answered, so a complete bundle does not carry an empty heading that
    /// reads as a list somebody forgot to fill in.
    fn gaps(&self) -> String {
        if self.missing.is_empty() {
            return String::new();
        }
        let listed = self.missing.iter().fold(String::new(), |mut page, gap| {
            let _ = writeln!(page, "  {gap}");
            page
        });
        format!("\nCould not be read:\n{listed}")
    }

    /// Every file the bundle would hold, its own first page included — which is what the
    /// scan reads, because the first page is written from the same values as the rest.
    #[must_use]
    pub fn files(&self) -> Vec<(String, String)> {
        let mut files = vec![(MANIFEST.to_owned(), self.manifest())];
        files.extend(
            self.pieces
                .iter()
                .map(|piece| (piece.name.clone(), piece.body.clone())),
        );
        files
    }
}

/// Where a bundle was found to still hold something that reads as a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residual {
    /// The file it was found in, so a refusal can name what produced it.
    pub source: String,
    /// Which line of that file, counted from one as an operator would read it.
    pub line: usize,
}

/// The shortest run of unbroken key-shaped characters that reads as a credential.
///
/// Deliberately a different mechanism from the allow-list: two checks that fail the same
/// way are one check. This one knows nothing about names — it reads the values themselves,
/// and anything long enough and dense enough to be a key trips it whatever it is called.
const KEY_LENGTH: usize = 20;

/// Whether a bundle still holds anything that reads as a credential, and where.
///
/// Run over the assembled bundle rather than each piece as it is collected, because the
/// question is about what would actually be shared. A hit is not a warning: the bundle is
/// not written at all, and the file that produced it is named, because the one failure
/// this whole module exists to prevent is a bundle that looked fine and was not.
///
/// `terms` is read for one reason: a setting its operator named and confirmed is not a
/// residual. A scan that refused the bundle over the one value somebody deliberately put
/// in it would make the consent worth nothing.
#[must_use]
pub fn residual(files: &[(String, String)], terms: &Terms) -> Option<Residual> {
    for (source, body) in files {
        for (index, line) in body.lines().enumerate() {
            if chosen(line, terms) {
                continue;
            }
            // A line is read whole, marks and all. Skipping the ones that carry a mark
            // would let a line that holds both a mark and a leak pass on the strength of
            // the half that was handled — and a mark trips nothing here anyway: it is
            // four characters, and a key is twenty.
            if line.split(|c: char| !key_shaped(c)).any(reads_as_key) {
                return Some(Residual {
                    source: source.clone(),
                    line: index + 1,
                });
            }
        }
    }
    None
}

/// Whether this line is a setting the operator asked to have shown.
///
/// Matched as a setting rather than as a value, deliberately. What was consented to was
/// showing a *field*; the same key turning up in a log line was nobody's decision, and is
/// still a leak.
fn chosen(line: &str, terms: &Terms) -> bool {
    line.split_once('=')
        .is_some_and(|(name, _)| terms.reveals(name))
}

/// Whether a character could be part of a key: the alphabet every service in the stack
/// mints its own with, and nothing else.
fn key_shaped(character: char) -> bool {
    character.is_ascii_alphanumeric()
}

/// Whether a run of characters reads as a credential rather than as a word.
///
/// Long, and mixed: prose is long too, but a word is letters, and a key of any length
/// anybody generates carries digits. Neither test alone is worth much and both together
/// have never yet called a sentence a secret.
fn reads_as_key(run: &str) -> bool {
    run.len() >= KEY_LENGTH
        && run.chars().any(|character| character.is_ascii_digit())
        && run.chars().any(|character| character.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::{
        prose, residual, settings, shown, Contents, Filenames, Marks, Piece, Residual, Taken,
        Terms, SALT_BYTES,
    };
    use crate::ports::random::Random;

    /// Salt built rather than written, for the reason every credential-shaped fixture in
    /// this repository is built: a literal that reaches a derivation is a hard-coded
    /// cryptographic value, and a scanner reading it cannot tell a fixture from a mistake.
    fn salt() -> Vec<u8> {
        ('a'..='p').map(|letter| letter as u8).collect()
    }

    /// A second bundle's salt — the same bytes the other way round, so the two differ
    /// without either being written down.
    fn other_salt() -> Vec<u8> {
        salt().into_iter().rev().collect()
    }

    /// A value shaped the way a generated key is: long, and carrying both letters and
    /// digits. Built from character ranges rather than written, so that neither the
    /// secret scanner reading this repository's history nor the analyser reading its
    /// dataflow finds a credential-shaped literal to object to.
    fn key_shaped() -> String {
        ('a'..='j').chain('0'..='9').cycle().take(32).collect()
    }

    /// A source of bytes that answers with what it was built from.
    struct Fixed(Vec<u8>);

    impl Random for Fixed {
        fn bytes(&self, n: usize) -> Option<Vec<u8>> {
            Some(self.0.iter().copied().cycle().take(n).collect())
        }
    }

    /// A source that cannot answer at all.
    struct Empty;

    impl Random for Empty {
        fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
            None
        }
    }

    /// Stand-ins over a built salt. Made through the same door the real ones are, so the
    /// path that reads randomness is the path under test.
    fn marks(salt: Vec<u8>) -> Marks {
        // Built directly where the source will not answer: a fallback nothing can reach
        // is a branch no test can cover, and this file is held to covering all of them.
        Marks::new(&Fixed(salt.clone())).unwrap_or(Marks { salt })
    }

    /// Terms that hold what a test is about and default for the rest.
    fn terms(revealed: &[&str], filenames: Filenames) -> Terms {
        Terms {
            window: "the last 200 lines of each service".to_owned(),
            filenames,
            revealed: revealed.iter().map(|field| (*field).to_owned()).collect(),
        }
    }

    /// The case the whole free-text rule exists for. An address is worth sharing and the
    /// key riding in its query is not, and in a log line they arrive as one word with no
    /// field name in front of them for an allow-list to recognise.
    #[test]
    fn a_key_riding_in_a_log_line_loses_the_query_and_keeps_the_address() {
        let line = format!(
            "prowlarr | GET https://indexer.example.com/api?apikey={}&t=search done",
            key_shaped()
        );
        let said = prose(&line, &marks(salt()), &Terms::default());

        assert!(said.contains("https://indexer.example.com/api?"), "{said}");
        assert!(!said.contains(&key_shaped()), "{said}");
        assert!(
            said.contains("prowlarr | GET"),
            "the line still reads as a line"
        );
    }

    /// A key with nothing in front of it to name it is still a key. The allow-list has
    /// nothing to work from in free text, so the rule reads values rather than names.
    #[test]
    fn a_key_with_no_name_in_front_of_it_is_replaced_anyway() {
        let said = prose(
            &format!("sonarr | using {}", key_shaped()),
            &marks(salt()),
            &Terms::default(),
        );
        assert!(!said.contains(&key_shaped()), "{said}");
    }

    /// The diagnostic the marks preserve: two mentions of one key read alike, so a reader
    /// can see that two services point at the same account without seeing which.
    #[test]
    fn one_key_reads_the_same_wherever_it_appears_in_free_text() {
        let bundle = marks(salt());
        let key = key_shaped();
        let said = prose(
            &format!("first {key}\nsecond {key}"),
            &bundle,
            &Terms::default(),
        );
        let marked = bundle.of(&key);

        assert_eq!(said, format!("first {marked}\nsecond {marked}"));
    }

    /// A question in a log line is a question, not a query string — the parameters are
    /// what make it one, and a rule that read every question mark as a URL would replace
    /// half of what a reader came for.
    #[test]
    fn a_question_in_a_log_line_is_not_mistaken_for_a_query() {
        let said = prose("sonarr | why? no idea", &marks(salt()), &Terms::default());
        assert_eq!(said, "sonarr | why? no idea");
    }

    /// A line comes back spaced as it was written. A log line read by a person is half
    /// indentation, and re-spacing one is the sort of tidying that loses the shape of a
    /// stack trace.
    #[test]
    fn a_line_comes_back_spaced_as_it_was_written() {
        let said = prose(
            "sonarr |    indented   twice",
            &marks(salt()),
            &Terms::default(),
        );
        assert_eq!(said, "sonarr |    indented   twice");
    }

    /// Some operators consider their library their own business, so a filename is replaced
    /// unless it was asked for — and the mark still says the two mentions are one file.
    #[test]
    fn a_media_filename_is_replaced_unless_it_was_asked_for() {
        let line = "sonarr | imported /media/tv/Some.Show.S01E01.mkv";
        let replaced = prose(line, &marks(salt()), &terms(&[], Filenames::Replaced));
        let shown = prose(line, &marks(salt()), &terms(&[], Filenames::Shown));

        assert!(!replaced.contains("Some.Show"), "{replaced}");
        assert!(shown.contains("/media/tv/Some.Show.S01E01.mkv"), "{shown}");
    }

    /// The invariant the two halves are built on: the redactor and the scan share a
    /// tokeniser, so whatever one writes the other accepts. Without this they are free to
    /// drift, and a bundle would start refusing itself over text it had just redacted.
    #[test]
    fn whatever_the_free_text_rule_writes_the_scan_accepts() {
        let key = key_shaped();
        let awkward = format!(
            "prowlarr | GET https://x.example.com/api?apikey={key}&t=search\nsonarr | bare {key}\nradarr | /media/f.mkv"
        );
        let said = prose(&awkward, &marks(salt()), &Terms::default());

        assert_eq!(
            residual(&[("logs.txt".to_owned(), said)], &Terms::default()),
            None
        );
    }

    /// Consent means the scan cannot refuse the bundle over the one value its operator
    /// deliberately put in it — and means nothing anywhere else, because what was agreed
    /// to was showing a field.
    #[test]
    fn a_revealed_setting_is_shown_and_is_not_a_residual() {
        let key = key_shaped();
        let asked = terms(&["SONARR_API_KEY"], Filenames::Replaced);
        let body = settings(&format!("SONARR_API_KEY={key}"), &marks(salt()), &asked);
        assert_eq!(body, format!("SONARR_API_KEY={key}"));
        assert_eq!(
            residual(&[("configuration.env".to_owned(), body)], &asked),
            None
        );

        // The same value in a log line was nobody's decision, and is still a leak.
        assert!(residual(
            &[("logs.txt".to_owned(), format!("sonarr | {key}"))],
            &asked
        )
        .is_some());
    }

    /// The reader is not the operator who chose any of this, so the first page says what
    /// was bounded, what was replaced, and — where it applies — that the bundle holds a
    /// credential in the clear.
    #[test]
    fn the_first_page_states_what_was_done_to_the_bundle() {
        let contents = Contents {
            pieces: vec![Piece {
                name: "logs.txt".to_owned(),
                body: String::new(),
            }],
            missing: Vec::new(),
            taken: Taken {
                lemonfiber: "0.7.0".to_owned(),
                stack: "1.2.0".to_owned(),
                at: "2026-08-18T00:00:00Z".to_owned(),
            },
            terms: terms(&[], Filenames::Replaced),
        };
        let page = contents.manifest();
        assert!(
            page.contains("Logs: the last 200 lines of each service."),
            "{page}"
        );
        assert!(page.contains("Media filenames: replaced."), "{page}");
        assert!(
            !page.contains("IN THE CLEAR"),
            "nothing was revealed: {page}"
        );

        let revealed = Contents {
            terms: terms(&["SONARR_API_KEY"], Filenames::Shown),
            ..contents
        };
        let page = revealed.manifest();
        assert!(page.contains("SHOWN AS THEY ARE"), "{page}");
        assert!(page.contains("  SONARR_API_KEY"), "{page}");
        assert!(page.contains("credential in the clear"), "{page}");
        assert!(page.contains("Media filenames: shown"), "{page}");
    }

    /// The property the whole design turns on: a setting nobody has thought about is
    /// redacted, rather than shown because no rule named it.
    #[test]
    fn a_setting_nobody_named_is_redacted_rather_than_shown() {
        assert!(shown("PUID"));
        assert!(shown("USENET_HOST"));
        assert!(shown("INDEXER_VALIDATED"));
        assert!(!shown("INDEXER_APIKEY"));
        assert!(!shown("USENET_PASS"));
        // The one that matters: a field this build has never heard of.
        assert!(!shown("SOME_SERVICE_TOKEN_ADDED_NEXT_YEAR"));
    }

    #[test]
    fn a_shown_setting_keeps_its_value_and_a_hidden_one_keeps_only_its_name() {
        let secret = key_shaped();
        let shown = settings(
            &format!("PUID=1000\nUSENET_PASS={secret}"),
            &marks(salt()),
            &Terms::default(),
        );
        assert!(shown.contains("PUID=1000"));
        assert!(shown.contains("USENET_PASS=<redacted:"));
        assert!(!shown.contains(&secret));
    }

    /// The same secret reads the same throughout one bundle — that likeness is the whole
    /// diagnostic point — and differently in the next, which is what keeps a mark from
    /// becoming a fingerprint that outlives it.
    #[test]
    fn one_secret_reads_the_same_within_a_bundle_and_differently_across_two() {
        let bundle = marks(salt());
        let another = marks(other_salt());
        let value = key_shaped();
        let other = format!("{value}-elsewhere");
        assert_eq!(bundle.of(&value), bundle.of(&value));
        assert_ne!(bundle.of(&value), bundle.of(&other));
        assert_ne!(bundle.of(&value), another.of(&value));
    }

    /// The shape that catches people out: the address is worth sharing and the key riding
    /// in its query string is not, and they arrive as one string.
    #[test]
    fn a_key_riding_in_a_query_string_does_not_ride_out_with_the_address() {
        let key = key_shaped();
        let redacted = settings(
            &format!("INDEXER_URL=https://indexer.example.com/api?apikey={key}\nPUID=1000"),
            &marks(salt()),
            &Terms::default(),
        );
        assert!(redacted.contains("https://indexer.example.com/api?<redacted:"));
        assert!(!redacted.contains(&key));
    }

    /// A name with nothing after it is a setting that is not set, which is worth seeing:
    /// an empty credential and a wrong one are different faults.
    #[test]
    fn a_setting_with_no_value_is_left_as_it_is() {
        let redacted = settings(
            "INDEXER_APIKEY=\nnot a setting at all",
            &marks(salt()),
            &Terms::default(),
        );
        assert!(redacted.contains("INDEXER_APIKEY="));
        assert!(!redacted.contains("<redacted:"));
        assert!(redacted.contains("not a setting at all"));
    }

    /// Randomness that cannot be had is not something to paper over: a fixed salt would
    /// make every mark the same in every bundle anyone ever posted.
    #[test]
    fn marks_are_not_made_without_randomness() {
        assert!(Marks::new(&Empty).is_none());
        assert_eq!(salt().len(), SALT_BYTES);
    }

    /// The belt-and-braces check, and the reason it is a different mechanism: it knows
    /// nothing about names, so a value that slipped through under a name nobody listed is
    /// still caught by what it looks like.
    #[test]
    fn a_credential_that_survived_the_allow_list_is_still_caught() {
        let leaked = vec![(
            "sonarr/config.xml".to_owned(),
            format!("<ApiKey>{}</ApiKey>", key_shaped()),
        )];
        assert_eq!(
            residual(&leaked, &Terms::default()),
            Some(Residual {
                source: "sonarr/config.xml".to_owned(),
                line: 1,
            })
        );
    }

    #[test]
    fn a_bundle_holding_only_marks_and_prose_is_let_through() {
        let clean = vec![
            (
                "env".to_owned(),
                "INDEXER_APIKEY=<redacted:a3f1>\nPUID=1000".to_owned(),
            ),
            (
                "notes".to_owned(),
                "the download client refused the login twice this afternoon".to_owned(),
            ),
        ];
        assert_eq!(residual(&clean, &Terms::default()), None);
    }

    /// Long is not enough and mixed is not enough: a version number is short, a run of
    /// digits carries no letters, and a long word carries no digits. All three have to
    /// hold at once before anything is called a key, and each of those three is a way a
    /// bundle would otherwise be refused over a sentence.
    #[test]
    fn prose_and_versions_are_not_mistaken_for_credentials() {
        let ordinary = vec![(
            "report".to_owned(),
            "sonarr 4.0.15.2941 finished importing 37 episodes without incident\n\
             the download identifier 12345678901234567890 was retried once\n\
             recyclarr synchronised the qualityprofilesconfiguration definitions"
                .to_owned(),
        )];
        assert_eq!(residual(&ordinary, &Terms::default()), None);
    }
}
