//! A plugin installed by name: the catalogue's index fetched, its signature verified
//! against the key this build carries, the name resolved through it, and the commit it
//! names installed as any git source's is.
//!
//! **Nothing is resolved through an index that did not verify.** An index with no
//! signature, one whose signature does not hold and one read by a build that carries no
//! key are refused alike, before the name is looked up, and nothing is fetched from any
//! origin it names.
//!
//! **What is installed is what was reviewed.** The index names one commit and the
//! digest of the manifest there; the commit is fetched and its manifest held to that
//! digest before the install reads a line of it. What the origin serves by default
//! today is not asked.
//!
//! **Nothing else depends on the catalogue.** Installing from a directory or a git
//! source never reaches this file, and what is installed is never asked about it again.

use crate::app::Ctx;
use crate::config::REACH_CATALOGUE_KEY;
use crate::error::codes::plugin::{
    CATALOGUE_OFF, CATALOGUE_UNREACHABLE, CATALOGUE_UNREADABLE, NOT_AS_REVIEWED, NOT_CATALOGUED,
    SIGNATURE_UNVERIFIED,
};
use crate::error::{Problem, Remedy, Severity, State};
use crate::plugin::catalogue::{self, Entry, Refused};
use crate::plugin::{Installs, Register};
use crate::ports::http::{Method, Request};

use super::fetching::{self, Vouched};

/// How many times the release address may hand a file on to another before the fetch
/// is given up.
///
/// GitHub answers a release asset's address with the one its download host serves the
/// file from, which is one hop; a few more is room for that arrangement to change, and
/// a bound is what keeps an address that hands on to itself from being asked for ever.
const HOPS: usize = 3;

/// Carry an errand out over the plugin the catalogue registers under `name`, at the
/// commit it reviewed.
///
/// # Errors
///
/// Where asking the catalogue is switched off, where its index or signature cannot be
/// fetched, where the signature does not verify or there is no key to verify it
/// against, where the index cannot be read, where it holds no plugin by that name,
/// where the commit it names holds a manifest other than the one it reviewed, and every
/// refusal the errand makes over a git source.
pub(super) async fn resolved(
    ctx: &Ctx,
    held: Register,
    name: &str,
    errand: super::Errand<'_>,
    consent: &super::Consent,
) -> Result<Installs, Box<Problem>> {
    if !ctx.settings.reaching.allows(REACH_CATALOGUE_KEY) {
        return Err(Box::new(switched_off(name)));
    }
    let index = asset(ctx, catalogue::INDEX)
        .await
        .map_err(|why| Box::new(unreachable(&why)))?
        .ok_or_else(|| {
            Box::new(unreachable(
                "the catalogue has published no release, so there is no index to read",
            ))
        })?;
    let signature = asset(ctx, catalogue::SIGNATURE)
        .await
        .map_err(|why| Box::new(unreachable(&why)))?
        .unwrap_or_default();
    let read = catalogue::verified(&index, &signature, ctx.catalogue_key.as_deref()).map_err(
        |refused| {
            Box::new(match refused {
                Refused::Unverified(why) => unverified(&why),
                Refused::Unreadable(why) => unreadable(&why),
            })
        },
    )?;
    let entry = read
        .entry(name)
        .ok_or_else(|| Box::new(not_catalogued(name)))?;
    if !entry.pins_a_commit() {
        return Err(Box::new(unreadable(&format!(
            "its entry for {name} names {}, which is not one whole commit",
            entry.revision
        ))));
    }
    let signed = ctx
        .catalogue_key
        .as_deref()
        .map_or_else(String::new, catalogue::signer);
    let vouched = Vouched {
        entry,
        signed: &signed,
    };
    let source = fetching::Fetching {
        url: &entry.origin,
        revision: Some(&entry.revision),
        vouched: Some(&vouched),
    };
    fetching::fetched(ctx, held, &source, errand, consent).await
}

/// What one of the release's files holds, nothing where the release has no such file,
/// or why it could not be read.
///
/// The address is asked with nothing but the program's name, and a hop to another
/// address is followed only to one that is encrypted too: the request carries no
/// credential for a hop to hand on, and what comes back is checked against a key here
/// rather than trusted for where it came from.
async fn asset(ctx: &Ctx, url: &str) -> Result<Option<String>, String> {
    let mut at = url.to_owned();
    for _ in 0..=HOPS {
        let answer = ctx
            .seams
            .http
            .send(&Request {
                method: Method::Get,
                url: at.clone(),
                headers: vec![("User-Agent".to_owned(), crate::PRODUCT.to_owned())],
                body: None,
            })
            .await
            .map_err(|failure| failure.to_string())?;
        if answer.is_success() {
            return Ok(Some(answer.body));
        }
        if answer.status == 404 {
            return Ok(None);
        }
        match answer.header("location") {
            Some(next) if (300..400).contains(&answer.status) && next.starts_with("https://") => {
                next.clone_into(&mut at);
            }
            _ => return Err(format!("{at} answered {}", answer.status)),
        }
    }
    Err(format!("{url} was handed on more than {HOPS} times"))
}

/// Said where asking the catalogue is switched off.
fn switched_off(name: &str) -> Problem {
    Problem::new(
        CATALOGUE_OFF,
        Severity::Error,
        format!("asking the catalogue is switched off, so {name} was not looked up"),
        "Nothing was fetched and nothing was installed. This machine's settings keep lemonfiber \
         from reading the catalogue's index.",
        Remedy::new(format!(
            "Install it from a git source or a directory you name instead, or allow it with \
             `lemonfiber config set {REACH_CATALOGUE_KEY} on`"
        )),
    )
    .in_state(State::Guided)
}

/// Said where the catalogue's index or its signature could not be fetched.
fn unreachable(why: &str) -> Problem {
    Problem::new(
        CATALOGUE_UNREACHABLE,
        Severity::Error,
        "The catalogue's index could not be read",
        "Nothing was installed. What is installed already is unaffected, and installing from a \
         git source or a directory you name does not ask the catalogue.",
        Remedy::new(
            "Check this machine can reach github.com and install it again, or install it from \
             its git source",
        ),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

/// Said where the index is not tied to what the catalogue signed.
fn unverified(why: &str) -> Problem {
    Problem::new(
        SIGNATURE_UNVERIFIED,
        Severity::Error,
        "The catalogue's index is not signed by the key this build carries",
        "Nothing was resolved through it and nothing was installed. An index nobody can show \
         the catalogue signed is not evidence that anybody reviewed anything.",
        Remedy::new(
            "Install it from its git source, where it is installed as unreviewed, or install a \
             lemonfiber that carries the catalogue's key",
        ),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

/// Said where the index verified and cannot be read.
fn unreadable(why: &str) -> Problem {
    Problem::new(
        CATALOGUE_UNREADABLE,
        Severity::Error,
        "The catalogue's index is signed and this build cannot read it",
        "Nothing was resolved through it and nothing was installed.",
        Remedy::new(
            "Update lemonfiber, whose newer releases read newer indexes, or install it from its \
             git source",
        ),
    )
    .in_state(State::Guided)
    .with_detail(why.to_owned())
}

/// Said where the catalogue holds no plugin by the name given.
fn not_catalogued(name: &str) -> Problem {
    Problem::new(
        NOT_CATALOGUED,
        Severity::Error,
        format!("The catalogue holds no plugin called {name}"),
        "Nothing was fetched and nothing was installed.",
        Remedy::new(format!(
            "Check the name, or write `./{name}` to install the directory of that name"
        )),
    )
    .in_state(State::Guided)
}

/// Said where the commit the catalogue named holds a manifest other than the one it
/// reviewed.
pub(super) fn not_as_reviewed(entry: &Entry) -> Problem {
    Problem::new(
        NOT_AS_REVIEWED,
        Severity::Error,
        format!(
            "{} at {} is not what the catalogue reviewed",
            entry.origin, entry.revision
        ),
        "Nothing was installed. The manifest the commit holds is not the one whose digest the \
         catalogue signed.",
        Remedy::new(
            "Install it from its git source if you mean to run it unreviewed, and tell the \
             catalogue's maintainers",
        ),
    )
    .in_state(State::Guided)
    .with_detail(format!("the catalogue signed {}", entry.manifest))
}
