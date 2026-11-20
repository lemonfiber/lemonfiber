//! How the web surface was last served, written down by the run that served it.
//!
//! Pairing material names the address a phone should reach, and the port is the one
//! part of that address nothing but a serving run knows. So every run writes down the
//! port it took and whether it took it encrypted and on the network, and pairing reads
//! that back — from the terminal while nothing is being served as well as from the
//! surface itself — rather than either of them guessing.

use std::path::Path;

use serde::Deserialize;

use crate::config::store::Failure;

/// Where it is written, beside the certificate.
const FILE: &str = "served.json";

/// Why it was not written, where the store says nothing more that names no path.
const UNWRITTEN: &str = "the store would not write it";

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
/// What stopped it being written, in the operating system's words and naming no path, so
/// it can be said to whoever asks how the surface is served.
pub fn record(directory: &Path, served: Served) -> Result<(), String> {
    // Written out rather than serialised: three scalars cannot fail to be written, and a
    // serialiser's failure branch here would be one nothing could reach.
    let text = format!(
        "{{\n  \"port\": {},\n  \"encrypted\": {},\n  \"network\": {}\n}}\n",
        served.port, served.encrypted, served.network
    );
    crate::config::store::write(&directory.join(FILE), &text).map_err(|why| match why {
        Failure::NotWritten { reason, .. } | Failure::Unreadable { reason, .. } => reason,
        _ => UNWRITTEN.to_owned(),
    })
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
    /// The port it listens on.
    pub port: u16,
    /// Whether it presents a certificate, rather than plain text.
    pub encrypted: bool,
    /// Whether it is offered to the network, rather than to this machine alone.
    pub network: bool,
    /// The name a request has to carry beyond this machine, where the run is encrypted on
    /// a network.
    pub named: Option<String>,
    /// Why where it is served could not be written down, where it could not.
    pub unrecorded: Option<String>,
    /// The certificate it presents, as it describes itself, where the run is encrypted.
    pub certificate: Option<crate::certificate::described::Described>,
    /// The certificate and key presented, held in this process and never written out.
    #[serde(skip)]
    #[schemars(skip)]
    pub presented: Option<crate::certificate::Kept>,
}

/// How this run serves the surface, as settling it answered, shared by every clone of the
/// run's context and holding no key.
#[derive(Debug, Default)]
pub struct Settled(std::sync::RwLock<Option<ServedReport>>);

impl Settled {
    /// Keep `report` as what this run settled, without the key it presents.
    fn keep(&self, report: &ServedReport) {
        if let Ok(mut held) = self.0.write() {
            *held = Some(ServedReport {
                presented: None,
                ..report.clone()
            });
        }
    }

    /// What this run settled, or nothing where it settled nothing.
    fn read(&self) -> Option<ServedReport> {
        self.0.read().ok().and_then(|held| held.clone())
    }
}

/// Settle how the surface is served: the certificate it presents where `served` is
/// encrypted, kept or made beside the configuration; the name it answers to on a network;
/// and the record pairing reads back, which a rehearsal leaves unwritten. What it settled
/// is kept for the run to be asked.
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
    let certificate = presented
        .as_ref()
        .map(|kept| {
            kept.described()
                .ok_or_else(|| Box::new(uncertified("what is kept does not read as a certificate")))
        })
        .transpose()?;
    let named = if served.network && served.encrypted {
        super::answers_to(ctx, served.port).await
    } else {
        None
    };
    let unrecorded = match directory {
        Some(directory) if !ctx.dry_run => record(directory, served).err(),
        _ => None,
    };
    let report = ServedReport {
        port: served.port,
        encrypted: served.encrypted,
        network: served.network,
        named,
        unrecorded,
        certificate,
        presented,
    };
    ctx.served.keep(&report);
    Ok(report)
}

/// How this run serves the surface, as it settled when it began: read, and never settled
/// again.
///
/// # Errors
///
/// The [`Problem`] where this run settled nothing, because it is not serving the surface.
pub(crate) fn settled(ctx: &crate::app::Ctx) -> Result<ServedReport, Box<crate::error::Problem>> {
    ctx.served.read().ok_or_else(|| Box::new(not_serving()))
}

/// How the surface is served was asked of a run that is not serving it.
fn not_serving() -> crate::error::Problem {
    crate::error::Problem::new(
        crate::error::codes::serve::NOT_SERVING,
        format!(
            "This run of {} is not serving the web interface",
            crate::PRODUCT
        ),
        "How the web interface is served is what the run serving it settled when it began, and \
         this run settled nothing.",
        crate::error::Remedy::new("Ask the run serving the web interface")
            .with_detail(format!("the address {} ui printed", crate::PRODUCT)),
    )
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
