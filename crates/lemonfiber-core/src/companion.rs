//! Pairing a phone with this stack: what it is handed, and what that rests on.
//!
//! A phone is not on this machine, so the network is in the trust path, and the one
//! moment it can learn which machine is the right one is when somebody hands it
//! something across the gap — a code on this machine's screen, read by its camera, or
//! the same text typed. What it is handed is **pairing material**: where to reach the
//! stack, the certificate that address will present, when the material stops being
//! good, and the stack's own name for itself. It carries no credential. Being handed it
//! admits nobody; the phone still signs in with the operator's own password.
//!
//! Three things are kept beside the configuration for it, in their own directory: the
//! [`certificate`] the surface presents when it is served encrypted, the stack's
//! [`identifier`], and how the surface was last [`served`] — the port being the one part
//! of the address nothing but a serving run knows.

pub mod certificate;
pub mod identifier;
pub mod served;

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use serde::Serialize;

use crate::app::Ctx;
use crate::error::codes::pair::{NOT_SERVED, NOWHERE, NO_ADDRESS, NO_CERTIFICATE, UNNAMED};
use crate::error::{Problem, Remedy, State};
use crate::PRODUCT;

/// What was asked of pairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// Make pairing material a phone reads to find this stack and know it. It carries
    /// no credential, and being handed it admits nobody.
    Pair,
    /// Replace the certificate the web surface presents to a paired phone. Every phone
    /// paired against the one it replaces refuses this machine until it is paired again,
    /// so unconfirmed it says that and replaces nothing.
    Certificate {
        /// Replace it, having been told what replacing it costs.
        confirm: bool,
    },
}

/// Carry out what was asked of pairing.
///
/// A rehearsal of a replacement is the replacement unconfirmed, which is already the
/// account of what it would cost.
///
/// # Errors
///
/// The [`Problem`] either of the two refuses with.
pub async fn asked(ctx: &Ctx, asked: Asked) -> Result<crate::app::Outcome, Box<Problem>> {
    match asked {
        Asked::Pair => paired(ctx).await.map(crate::app::Outcome::Pairing),
        Asked::Certificate { confirm } => {
            replacing(ctx, confirm && !ctx.dry_run).map(crate::app::Outcome::Certificate)
        }
    }
}

/// How long material is good for once it is made.
///
/// Long enough to read a code off a screen and walk to wherever the phone is; short
/// enough that a photograph of the screen tomorrow pairs nothing.
pub const LASTS: Duration = Duration::from_secs(10 * 60);

/// What a phone is handed, exactly as it reads it.
///
/// Four fields and no more: a reader refuses one it does not know, which is what keeps a
/// credential from ever riding along under a name nobody thought to forbid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PairingMaterial")]
pub struct Material {
    /// Where the phone reaches the stack: an `https` address on the household network.
    pub address: String,
    /// The certificate that address presents: SHA-256 over its DER encoding, in
    /// lower-case hex. Not the digest of its public key.
    pub fingerprint: String,
    /// When the material stops being good, in seconds since the Unix epoch.
    pub expires: u64,
    /// The stack's own identifier: opaque, minted once from nothing and kept, and the
    /// same across every issue of the material, a change of address and a replacement of
    /// the certificate. Not the stack's name and not its version, and it carries nothing
    /// about the household or anybody in it.
    pub stack: String,
}

/// Pairing material, and what the operator is told beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "Pairing")]
pub struct Pairing {
    /// The material itself.
    pub material: Material,
    /// The material as the one line a code carries and a person types.
    pub written: String,
    /// The fingerprint in the short form a person compares with what the phone shows
    /// after typing the line in, as [`comparable`] derives it.
    pub compare: String,
    /// When it stops being good, as a date and a time of day.
    pub until: String,
    /// What would make every paired phone refuse this machine, said now rather than
    /// discovered then. In words any surface can show: how the certificate is replaced
    /// is each surface's own to say, so this names no command.
    pub replacing: String,
    /// What is worth knowing about the address itself, where anything is.
    pub caution: Option<String>,
}

/// What asking for the certificate to be replaced came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "CertificateReport")]
pub struct Replacement {
    /// Whether it was replaced. Unconfirmed, it is not, and what replacing it costs is
    /// what is said.
    pub replaced: bool,
    /// What a phone would pin now: the new certificate where it was replaced, the one
    /// kept where it was not, and nothing where none has been made.
    pub fingerprint: Option<String>,
    /// What replacing it means for every phone already paired.
    pub consequence: String,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    ///
    /// Said in a field of its own so that a rehearsal is never told from the real run by
    /// its wording alone.
    pub rehearsed: bool,
}

/// What every phone paired with this machine is told when the certificate changes.
const CONSEQUENCE: &str = "Every phone paired with this machine refuses it from then on, as it \
                           should for a certificate it was never shown, until it is paired \
                           again with new material.";

/// Pairing material for this stack, made now.
///
/// # Errors
///
/// A [`Problem`] naming what is missing: somewhere to keep what pairing needs, the
/// surface served encrypted on the network, the certificate it presents, an address a
/// phone could reach, or the stack's identifier.
pub async fn paired(ctx: &Ctx) -> Result<Pairing, Box<Problem>> {
    let directory = kept_in(ctx)?;
    let served = served::last(directory)
        .filter(|served| served.encrypted && served.network)
        .ok_or_else(|| Box::new(not_served()))?;
    let held = certificate::kept(directory)
        .map_err(|why| Box::new(no_certificate(&why.to_string())))?
        .ok_or_else(|| Box::new(no_certificate("none has been made")))?;
    let reached = reached(ctx, served.port)
        .await
        .ok_or_else(|| Box::new(no_address()))?;
    let stack = identifier::kept_or_minted(directory, ctx.seams.random.as_ref())
        .map_err(|why| Box::new(unnamed(&why.to_string())))?;
    let expiring = ctx.seams.clock.now() + LASTS;
    let material = Material {
        address: encrypted(&reached.url),
        fingerprint: held.fingerprint,
        expires: expiring
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
        stack,
    };
    Ok(Pairing {
        written: serde_json::to_string(&material).unwrap_or_default(),
        compare: comparable(&material.fingerprint),
        material,
        until: crate::instant::written(expiring).unwrap_or_default(),
        replacing: format!(
            "The certificate this address presents was made by {PRODUCT} and nothing renews \
             it. It changes only when somebody replaces it. {CONSEQUENCE}"
        ),
        caution: reached.caution,
    })
}

/// The letters and digits nobody reads as another: no `0`, `1`, `I` or `O`.
const COMPARABLE: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// A fingerprint in the form a person compares by eye: sixteen characters in four
/// groups of four.
///
/// SHA-256 over the fingerprint as its sixty-four lower-case hex characters, and the
/// first sixteen bytes of that digest, each modulo thirty-two, as an index into
/// [`COMPARABLE`]. The phone derives it the same way, so the two agree exactly when the
/// fingerprints do.
#[must_use]
pub fn comparable(fingerprint: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, fingerprint.as_bytes());
    let letters: Vec<char> = digest
        .as_ref()
        .iter()
        .take(16)
        .filter_map(|byte| {
            COMPARABLE
                .get(usize::from(byte % 32))
                .copied()
                .map(char::from)
        })
        .collect();
    letters
        .chunks(4)
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// The name pairing material gives a phone for this machine on `port`, where it gives
/// one.
///
/// A run serving encrypted on a network answers to it, because a paired phone reaches
/// the surface by this name and no other.
pub async fn answers_to(ctx: &Ctx, port: u16) -> Option<String> {
    let reached = reached(ctx, port).await?;
    let written = format!(":{port}");
    reached
        .url
        .strip_prefix("http://")
        .and_then(|rest| rest.strip_suffix(written.as_str()))
        .map(str::to_owned)
}

/// What a client on another machine needs to reach this stack and know it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reaching {
    /// The `https` address the surface was last served at on the network, where it has
    /// been.
    pub address: Option<String>,
    /// The certificate that address presents, where one is kept or could be made.
    pub pin: Option<String>,
    /// How to serve the stack so another machine can reach it, where nothing yet does.
    pub caution: Option<String>,
}

/// Where a client on another machine reaches this stack, and the certificate it pins.
///
/// The certificate is made where none is kept yet, as serving encrypted would make it,
/// so a client is handed the one this machine will go on presenting. The address is the
/// one pairing names, and only where the surface has been served encrypted on the
/// network: an address nothing has served is one no client could reach.
pub async fn reaching(ctx: &Ctx) -> Reaching {
    let Some(directory) = ctx.settings.companion.as_deref() else {
        return Reaching {
            caution: Some(UNSERVED.to_owned()),
            ..Reaching::default()
        };
    };
    let pin = certificate::kept_or_made(directory)
        .ok()
        .map(|kept| kept.fingerprint);
    let address = match served::last(directory).filter(|served| served.encrypted && served.network)
    {
        Some(served) => reached(ctx, served.port)
            .await
            .map(|reached| encrypted(&reached.url)),
        None => None,
    };
    Reaching {
        caution: address.is_none().then(|| UNSERVED.to_owned()),
        address,
        pin,
    }
}

/// What a key is handed with where nothing has served the stack to another machine.
const UNSERVED: &str =
    "Nothing has served this stack encrypted on your network yet, so a client on \
    another machine has no address to reach. Serve the web interface on your \
    network, encrypted and on a port that stays the same, and use this key from this \
    machine until then.";

/// The household's address for this machine on `port`, which is where a phone is sent.
async fn reached(ctx: &Ctx, port: u16) -> Option<crate::door::Address> {
    let named = ctx.site.name().await;
    crate::door::address(
        named.as_deref(),
        ctx.settings.household_host.as_deref(),
        ctx.environment,
        port,
    )
}

/// Replace the certificate the surface presents, or say what replacing it would cost.
///
/// Said first and done second: unconfirmed, nothing changes and the consequence is the
/// answer, so nobody replaces a certificate a phone has pinned without having been told
/// what that phone will do next.
///
/// # Errors
///
/// A [`Problem`] where there is nowhere to keep a certificate, or where the one kept or
/// its replacement cannot be read or made.
pub fn replacing(ctx: &Ctx, confirm: bool) -> Result<Replacement, Box<Problem>> {
    let directory = kept_in(ctx)?;
    let held = if confirm {
        Some(certificate::replaced(directory))
    } else {
        certificate::kept(directory).transpose()
    }
    .transpose()
    .map_err(|why| Box::new(no_certificate(&why.to_string())))?;
    Ok(Replacement {
        rehearsed: false,
        replaced: confirm,
        fingerprint: held.map(|held| held.fingerprint),
        consequence: CONSEQUENCE.to_owned(),
    })
}

/// Where what pairing needs is kept, or why there is nowhere.
/// The stack's own identifier, as pairing material carries it, minted and kept first
/// where there is none; nothing where this machine has nowhere to keep one, or what it
/// keeps is not one.
///
/// Read by whatever answers a credential that holds no pairing material, so that a
/// client holding a key for a member and one for the operator can tell both reach one
/// stack.
#[must_use]
pub fn identified(ctx: &Ctx) -> Option<String> {
    let directory = kept_in(ctx).ok()?;
    identifier::kept_or_minted(directory, ctx.seams.random.as_ref()).ok()
}

fn kept_in(ctx: &Ctx) -> Result<&Path, Box<Problem>> {
    ctx.settings
        .companion
        .as_deref()
        .ok_or_else(|| Box::new(nowhere()))
}

/// The address a phone reaches, which is always the encrypted one.
///
/// The household's address is built as `http`, because that is what the services it
/// usually names serve; a phone refuses anything that presents no certificate.
fn encrypted(url: &str) -> String {
    url.strip_prefix("http://")
        .map_or_else(|| url.to_owned(), |rest| format!("https://{rest}"))
}

/// There is no configuration directory to keep a certificate or an identifier in.
fn nowhere() -> Problem {
    Problem::new(
        NOWHERE,
        "there is nowhere on this machine to keep what pairing a phone needs",
        "The certificate a phone pins and the name it knows this stack by are kept beside the \
         configuration, and this machine would not say where its configuration directory is.",
        Remedy::new(
            "Run it as a user with a home directory, so there is a configuration directory",
        ),
    )
}

/// The surface has not been served in the one way a phone can reach.
fn not_served() -> Problem {
    Problem::new(
        NOT_SERVED,
        format!("{PRODUCT} has not been served encrypted on your network, so a phone has nothing to reach"),
        "A phone refuses an address that presents no certificate, and reaches this machine \
         from the network rather than from here. The material names the port the surface \
         was last served on in that way, and it has not been.",
        Remedy::new("Serve the web interface encrypted and on your network, on a port that stays the same"),
    )
    .in_state(State::Guided)
}

/// The certificate the surface presents cannot be read, or none has been made.
fn no_certificate(why: &str) -> Problem {
    Problem::new(
        NO_CERTIFICATE,
        "the certificate this machine presents to a phone could not be read",
        format!(
            "A phone pins the certificate the surface presents, and {why}. It is not made \
             again on its own, because a new one is one every paired phone refuses."
        ),
        Remedy::new("Replace it, knowing every paired phone will need pairing again"),
    )
}

/// This machine answers to no name and has no address written down.
fn no_address() -> Problem {
    Problem::new(
        NO_ADDRESS,
        "this machine has no address a phone could reach it at",
        "Pairing material names the address a phone reaches, and this machine answers to no \
         name on the network and has none written down.",
        Remedy::new("Record the address your household reaches this machine at"),
    )
}

/// The stack's identifier could not be read or made.
fn unnamed(why: &str) -> Problem {
    Problem::new(
        UNNAMED,
        "this stack's own identifier could not be read or made",
        format!(
            "A phone knows this stack by an identifier it keeps whatever else changes, and \
             {why}."
        ),
        Remedy::new("Check that the configuration directory can be written, and try again"),
    )
}

#[cfg(test)]
mod tests;
