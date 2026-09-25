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

use std::fmt::Write as _;

use serde::Serialize;

/// How many bytes of salt a bundle's stand-ins are derived with.
///
/// Sixteen is far past guessing, and the salt is the whole of what makes a stand-in
mod allowed;
mod prose;
pub mod run;
mod scan;

pub use allowed::{settings, shown, Marks, SALT_BYTES};
pub use prose::prose;
pub use scan::{residual, Residual};

/// Whether media filenames are shown as they are.
///
/// Replaced unless asked otherwise. A library's contents are not a credential, but they
/// are the one thing in a bundle that says something about the person rather than about
/// the machine, and replacing them costs a diagnostic a reader can still follow — the
/// marks keep two mentions of one file recognisable as one file.
///
/// Read from the bare flag a surface carries rather than from a name of its own,
/// because that is what both surfaces have: `--filenames` on a command line and a
/// `filenames` in a request body are one word that is there or is not. Which way
/// round it reads is decided here, once — a surface that read it the other way
/// round would put a library's contents in a file people post in public.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(from = "bool")]
pub enum Filenames {
    /// Replaced by their marks.
    #[default]
    Replaced,
    /// Shown, because the operator asked for them.
    Shown,
}

impl From<bool> for Filenames {
    fn from(shown: bool) -> Self {
        if shown {
            Self::Shown
        } else {
            Self::Replaced
        }
    }
}

/// How a bundle was made: what was bounded, what was replaced, and what its operator asked
/// to have shown as it is.
///
/// Carried in the bundle rather than known only to the command that wrote it, because the
/// person who reads one is usually not the person who chose any of this.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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

/// One file inside a bundle: the name it will carry, and what it holds.
///
/// Held in memory rather than written as it is gathered, because everything is read back
/// before anything is written. A bundle that had already put one file on disk when it
/// found a credential in the next would have to be unwritten, and unwriting is the kind of
/// thing that half-works.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Taken {
    /// The lemonfiber that wrote it.
    pub lemonfiber: String,
    /// The stack it was written from.
    pub stack: String,
    /// When, as a service writes a moment.
    pub at: String,
}

/// Everything gathered for a bundle, and everything that could not be.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
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

#[cfg(test)]
mod tests;
