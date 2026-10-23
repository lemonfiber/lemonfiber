//! Where an adapter listens, where its upstream is, and where its configuration directory is
//! mounted, as the arguments it is started with say.

use std::path::PathBuf;

/// Where an adapter answers the core, inside the stack's network.
pub const LISTEN: &str = "0.0.0.0:8080";

/// Where an adapter's configuration directory is mounted.
pub const CONFIGURATION: &str = "/config";

/// What an adapter was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// Answer the core until stopped.
    Serve(Settings),
    /// Say whether the adapter on this machine answers, as a container health check.
    Health(Settings),
}

/// Where it listens, where its upstream is, and its configuration directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The address it listens on.
    pub listen: String,
    /// The upstream's base address.
    pub upstream: String,
    /// Its configuration directory.
    pub configuration: PathBuf,
}

impl Settings {
    /// The stack's defaults for an adapter whose upstream answers at `upstream`.
    #[must_use]
    pub fn reaching(upstream: &str) -> Self {
        Self {
            listen: LISTEN.to_owned(),
            upstream: upstream.to_owned(),
            configuration: PathBuf::from(CONFIGURATION),
        }
    }
}

/// What `arguments`, the process's own name left off, ask of an adapter whose upstream
/// answers at `upstream` unless they say otherwise.
///
/// # Errors
///
/// What was not understood.
pub fn asked(arguments: impl IntoIterator<Item = String>, upstream: &str) -> Result<Asked, String> {
    let mut arguments = arguments.into_iter().peekable();
    let health = arguments.next_if(|first| first == "health").is_some();
    let mut settings = Settings::reaching(upstream);
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--listen" => settings.listen = value,
            "--upstream" => settings.upstream = value,
            "--configuration" => settings.configuration = PathBuf::from(value),
            _ => {
                return Err(format!(
                    "{flag} is not a flag this takes; it takes --listen, --upstream and \
                     --configuration, after an optional health"
                ))
            }
        }
    }
    Ok(if health {
        Asked::Health(settings)
    } else {
        Asked::Serve(settings)
    })
}

#[cfg(test)]
mod tests;
