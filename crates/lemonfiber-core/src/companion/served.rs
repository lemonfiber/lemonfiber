//! How the web surface was last served, written down by the run that served it.
//!
//! Pairing material names the address a phone should reach, and the port is the one
//! part of that address nothing but a serving run knows. So every run writes down the
//! port it took and whether it took it encrypted and on the network, and pairing reads
//! that back — from the terminal while nothing is being served as well as from the
//! surface itself — rather than either of them guessing.

use std::path::Path;

use serde::Deserialize;

/// Where it is written, beside the certificate.
const FILE: &str = "served.json";

/// How the web surface was last served.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Served {
    /// The port it listened on.
    pub port: u16,
    /// Whether it presented the certificate kept beside this, rather than plain text.
    pub encrypted: bool,
    /// Whether it was offered to the network, rather than to this machine alone.
    pub network: bool,
}

/// Write down how the surface is being served.
///
/// # Errors
///
/// What stopped it being written, in words.
pub fn record(directory: &Path, served: Served) -> Result<(), String> {
    // Written out rather than serialised: three scalars cannot fail to be written, and a
    // serialiser's failure branch here would be one nothing could reach.
    let text = format!(
        "{{\n  \"port\": {},\n  \"encrypted\": {},\n  \"network\": {}\n}}\n",
        served.port, served.encrypted, served.network
    );
    crate::config::store::write(&directory.join(FILE), &text).map_err(|why| why.to_string())
}

/// How the surface was last served, or nothing where no run wrote it down or what was
/// written is not this record.
#[must_use]
pub fn last(directory: &Path) -> Option<Served> {
    let text = std::fs::read_to_string(directory.join(FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// What a run serving the surface presents, answers to and wrote down.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct ServedReport {
    /// What a phone pins for the certificate presented, where the run is encrypted.
    pub fingerprint: Option<String>,
    /// The name a request has to carry beyond this machine, where the run is encrypted on
    /// a network.
    pub named: Option<String>,
    /// Why where it is served could not be written down, where it could not.
    pub unrecorded: Option<String>,
    /// The certificate and key presented, held in this process and never written out.
    #[serde(skip)]
    #[schemars(skip)]
    pub presented: Option<crate::certificate::Kept>,
}

/// Settle how the surface is served: the certificate it presents where `served` is
/// encrypted, kept or made beside the configuration; the name it answers to on a network;
/// and the record pairing reads back, which a rehearsal leaves unwritten.
///
/// # Errors
///
/// The [`Problem`] where it is encrypted and no certificate can be kept, read or made.
pub(crate) async fn serving(
    ctx: &crate::app::Ctx,
    served: Served,
) -> Result<ServedReport, Box<crate::error::Problem>> {
    let directory = ctx.settings.companion.as_deref();
    let presented = if served.encrypted {
        let kept_in = directory.ok_or_else(|| {
            Box::new(uncertified(
                "there is no configuration directory to keep one in",
            ))
        })?;
        Some(
            crate::certificate::kept_or_made(ctx.seams.confined.as_ref(), kept_in)
                .map_err(|why| Box::new(uncertified(&why.to_string())))?,
        )
    } else {
        None
    };
    let named = if served.network && served.encrypted {
        super::answers_to(ctx, served.port).await
    } else {
        None
    };
    let unrecorded = match directory {
        Some(directory) if !ctx.dry_run => record(directory, served).err(),
        _ => None,
    };
    Ok(ServedReport {
        fingerprint: presented.as_ref().map(|kept| kept.fingerprint.clone()),
        named,
        unrecorded,
        presented,
    })
}

/// The certificate to present could not be had, for the reason `why` gives.
#[must_use]
pub fn uncertified(why: &str) -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::serve::NO_CERTIFICATE,
        format!("{} could not serve encrypted", crate::PRODUCT),
        format!("It presents a certificate it keeps beside its configuration, and {why}."),
        crate::error::Remedy::new(
            "Replace the certificate, knowing every paired phone will need pairing again",
        )
        .with_detail(format!(
            "{} companion certificate --confirm",
            crate::PRODUCT
        )),
    )
}

#[cfg(test)]
mod tests;
