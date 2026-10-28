//! An address reached over HTTPS and nothing else, built only from text that says so.
//!
//! A base is checked once, where it is read, and every address a request is sent to is
//! joined beneath it, so the scheme, the host and the port a request reaches are the ones
//! the base was checked for.

/// An address reached over HTTPS, naming no user, no query and no fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Https(reqwest::Url);

impl Https {
    /// `text` as an address, where it is reached over HTTPS, names a host, and carries no
    /// user, password, query or fragment.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        reqwest::Url::parse(text)
            .ok()
            .filter(|url| {
                url.scheme() == "https"
                    && url.host_str().is_some_and(|host| !host.is_empty())
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none()
            })
            .map(Self)
    }

    /// `path` beneath this address, on the same scheme, host and port: nothing where it
    /// does not begin with a single `/`, or holds a query, a fragment or a step up.
    #[must_use]
    pub fn joined(&self, path: &str) -> Option<Self> {
        let plain = path.starts_with('/')
            && !path.starts_with("//")
            && !path.contains(['?', '#', '\\'])
            && !path.split('/').any(|step| step == "." || step == "..");
        if !plain {
            return None;
        }
        let mut joined = self.0.clone();
        joined.set_path(&format!("{}{path}", self.0.path().trim_end_matches('/')));
        Some(Self(joined))
    }

    /// The address, to hand a client.
    #[must_use]
    pub const fn as_url(&self) -> &reqwest::Url {
        &self.0
    }
}

impl std::fmt::Display for Https {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
mod tests;
