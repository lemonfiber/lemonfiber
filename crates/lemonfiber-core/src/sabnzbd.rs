//! Reading `SABnzbd`'s own configuration.
//!
//! `SABnzbd` is a download client the Servarr apps are told about, not a service
//! lemonfiber sends wiring commands to, so unlike the Servarr shape it has no
//! client here — only the one value seed needs to register it: the API key it
//! generates for itself on first start.
//!
//! The key lives in `sabnzbd.ini`, a plain INI file, under a single `api_key`
//! entry. It is read as text rather than through an INI dependency: one known key
//! is wanted from a fixed format, so a parser would be weight for nothing — the
//! same reasoning as the Servarr key reader ([`crate::servarr::api_key`]). An
//! absent or empty entry reads as "not generated yet" — a service still
//! completing its first start, to be skipped and picked up on a later run — which
//! is `None`, never a fault.

/// The API key `SABnzbd` wrote to its configuration, if it has written one yet.
///
/// The `api_key` entry is matched by its exact name so a neighbouring `nzb_key`
/// or a `#`-commented line is not read as the key. An entry that is present but
/// empty is a first start not yet finished, and is `None` like an absent one.
#[must_use]
pub fn api_key(config_ini: &str) -> Option<String> {
    config_ini.lines().find_map(read_api_key)
}

/// One line as the API key it sets, where it is the `api_key` entry with a value.
fn read_api_key(line: &str) -> Option<String> {
    let (name, value) = line.split_once('=')?;
    if name.trim() != "api_key" {
        return None;
    }
    let key = value.trim();
    (!key.is_empty()).then(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::api_key;

    /// A minimal `sabnzbd.ini` as `SABnzbd` writes it, with the key under `[misc]`
    /// alongside a similarly-named entry the reader must not mistake for it.
    const CONFIG: &str = "\
[misc]
host = 0.0.0.0
api_key = the-key
nzb_key = ffffffffffff
";

    #[test]
    fn the_generated_key_is_read_from_its_entry() {
        assert_eq!(api_key(CONFIG).as_deref(), Some("the-key"));
    }

    #[test]
    fn a_neighbouring_key_entry_is_not_mistaken_for_it() {
        // `nzb_key` shares the suffix but is a different value; only `api_key`
        // is the download client's credential.
        let only_nzb = "[misc]\nnzb_key = ffffffffffff\n";
        assert_eq!(api_key(only_nzb), None);
    }

    #[test]
    fn a_commented_entry_is_not_read_as_the_key() {
        // A commented-out line keeps the `#` on the name, so it does not match.
        assert_eq!(api_key("#api_key = the-key"), None);
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        assert_eq!(api_key("api_key =   the-key  ").as_deref(), Some("the-key"));
    }

    #[test]
    fn a_key_not_generated_yet_is_absent_not_a_fault() {
        // Present but empty until first start completes.
        assert_eq!(api_key("[misc]\napi_key =\n"), None);
        // Only whitespace after the separator is also not-yet.
        assert_eq!(api_key("api_key =    "), None);
        // Or the entry is not there at all yet.
        assert_eq!(api_key("[misc]\nhost = 0.0.0.0\n"), None);
    }

    #[test]
    fn a_section_header_is_not_read_as_a_key() {
        // A line with no separator, such as a section header, is passed over.
        assert_eq!(api_key("[misc]"), None);
    }

    #[test]
    fn a_multibyte_value_survives_intact() {
        assert_eq!(api_key("api_key = café☃clé").as_deref(), Some("café☃clé"));
    }
}
