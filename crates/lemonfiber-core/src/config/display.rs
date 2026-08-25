//! Which settings are shown with their values, and why each one is.
//!
//! An allow-list, for the reason [`crate::bundle::allowed`] is one: the alternative is a
//! rule that guesses from a name, and a guess is wrong in whichever direction nobody
//! chose. Reading `KEY`, `PASS`, `TOKEN` out of a name catches `INDEXER_APIKEY` and
//! misses `OPENVPN_USER`, where a provider's account number *is* the credential — and it
//! misses every setting nobody has thought of yet, which is the case that matters,
//! because the setting that leaks is by definition the one nobody thought about.
//!
//! So the question is turned around. A value is shown because somebody named it and
//! wrote down what makes showing it safe; everything else is withheld, including
//! settings that do not exist yet. A stack that gains a field tomorrow withholds it
//! tomorrow, with nobody having to notice.
//!
//! What this costs is real and worth saying: an operator who adds a setting of their own
//! sees `(set, not shown)` for it in `config show` and at `/api/config`, and has to open
//! the file to read their own value. That is the trade — the surface is one a browser
//! renders, a screenshot captures and a screen-share broadcasts, and losing a lookup
//! there is cheaper than publishing a key from it.
//!
//! An account name is the case worth stating, because the list answers it twice and
//! differently. `QBITTORRENT_USERNAME` is shown: it names an account on a local service
//! lemonfiber itself sets up, and the password beside it is what signs in. `USENET_USER`
//! is not: a Usenet provider issues it, several of them issue an account number as the
//! username, and it is half of a paid login. Nothing diagnosable needs it — the host, the
//! port and whether the login was proven are all shown beside it. A keyword rule cannot
//! tell those two apart, because it is not a fact about the name.
//!
//! The counterpart rule is in [`lemonfiber_ports::withheld`], which handles text that has
//! no field names for a list to work from. Two surfaces, two rules, balanced opposite
//! ways on purpose: here a name is read against a list and anything unvouched-for is
//! withheld; there every rule is a guess about somebody else's sentence, so only the
//! narrow ones fire.

/// The settings displayed with their values, and why each one is.
///
/// A setting that arrives displayed and is not written down here is a bug, and the second
/// half of the entry is where somebody has to say what makes displaying it safe. The list
/// is the decision; this is where it is reviewable.
///
/// lemonfiber's own settings come first, then the stack's. The two halves are one list
/// because they reach an operator through one surface: `/api/config` serves whatever is
/// in the file, and a file holds both.
pub const SHOWN: &[(&str, &str)] = &[
    // lemonfiber's own settings — the answers setup records, under its own prefix and
    // in the stack's namespace both.
    (
        super::USENET_KEY,
        "whether this stack downloads over Usenet at all, which decides what runs",
    ),
    (
        super::TORRENT_KEY,
        "whether this stack torrents at all, which decides whether a tunnel runs",
    ),
    (
        super::IP_ECHO_KEY,
        "the address the leak check asks each container for its public address",
    ),
    (
        super::EXPLANATIONS_KEY,
        "whether the plain-language explanations are on, which is a display choice",
    ),
    (
        super::JELLYFIN_MODE_KEY,
        "whether the media server runs in a container or on the host machine",
    ),
    (
        super::INDEXER_URL_KEY,
        "where searches are sent, and the first thing to check when none come back",
    ),
    (
        super::INDEXER_VALIDATED_KEY,
        "whether the indexer key was proven before it was kept, never the key itself",
    ),
    (
        super::PROVIDER_HOST_KEY,
        "the Usenet server downloads are fetched from, which a diagnosis has to name",
    ),
    (
        super::PROVIDER_PORT_KEY,
        "the port that server is reached on, which is the other half of reaching it",
    ),
    (
        super::PROVIDER_TLS_KEY,
        "whether that connection is encrypted, which is a setting and not a secret",
    ),
    (
        super::PROVIDER_VALIDATED_KEY,
        "whether the provider login was proven before it was kept, never the login",
    ),
    // The stack's own settings, as its `.env.example` declares them.
    (
        super::DATA_ROOT_KEY,
        "the one path an operator has to get right, and the one they check first",
    ),
    (
        super::PUID_KEY,
        "the user id everything under the data root is owned by",
    ),
    (
        super::PGID_KEY,
        "the group id everything under the data root is owned by",
    ),
    ("TZ", "schedules and log timestamps are read in this zone"),
    (
        "LAN_BIND",
        "the address the household tier is published on, which an operator narrows by hand",
    ),
    (
        super::VPN_PROVIDER_KEY,
        "which provider's servers the tunnel dials, and not the account on them",
    ),
    (
        "VPN_COUNTRIES",
        "where the tunnel comes out, which is changed often and checked after",
    ),
    (
        super::VPN_PORT_FORWARDING_KEY,
        "whether a forwarded port is asked for at all",
    ),
    (
        "QBITTORRENT_USERNAME",
        "the name of an account on a local service lemonfiber itself sets up; the password \
         beside it is what signs in, and that is withheld",
    ),
    (
        "UMASK",
        "the file mode extracted downloads land on disk with",
    ),
    (
        "FLARESOLVERR_LOG_LEVEL",
        "how much the challenge solver writes to its own log",
    ),
    (
        "SEERR_LOG_LEVEL",
        "how much the request service writes to its own log",
    ),
    (
        "RECYCLARR_CRON",
        "when the quality sync runs, which explains a profile that has not moved",
    ),
    (
        "HOMEPAGE_ALLOWED_HOSTS",
        "the addresses the dashboard answers for, and a wrong one is why it refuses",
    ),
    (
        "HOMEPAGE_VAR_LAN_HOST",
        "the address the dashboard's household links point at",
    ),
    (
        "HOMEPAGE_VAR_QBITTORRENT_USER",
        "the account name the dashboard widget signs in as, beside a withheld password",
    ),
    (
        "UN_SONARR_0_URL",
        "where the extractor reaches the television service",
    ),
    (
        "UN_RADARR_0_URL",
        "where the extractor reaches the film service",
    ),
    (
        "UN_LIDARR_0_URL",
        "where the extractor reaches the music service",
    ),
    (
        "DOMAIN",
        "the hostname real certificates are obtained for, which is public in the certificate",
    ),
    ("NAS_HOST", "the machine the network mount is exported from"),
    (
        "NAS_EXPORT",
        "the share on that machine the data root lives on",
    ),
];

/// Whether a setting's value is displayed as it was written.
///
/// Trimmed, because the name arrives from a file a person edits by hand and a leading
/// space is not a different setting — and a comparison that thought it was would withhold
/// on a name that is on the list, or worse, show on one that is not.
#[must_use]
pub fn in_full(name: &str) -> bool {
    let name = name.trim();
    SHOWN.iter().any(|(shown, _)| *shown == name)
}

/// A displayed value with anything after its question mark withheld.
///
/// The shape that catches people out, and the one the allow-list cannot see: an indexer's
/// address is worth showing and the key riding in its query string is not, and the two
/// arrive as one value. `INDEXER_URL` is the setting an operator pastes from an indexer's
/// own site, and some of those sites hand out the address with the key already in it.
///
/// A value that is allowed through therefore keeps its address and loses its parameters
/// wholesale — the same call [`crate::bundle::allowed`] makes for the same value, for the
/// same reason. A query nobody reads is a smaller loss than a key everybody can.
#[must_use]
pub fn without_query(value: &str) -> String {
    match value.split_once('?') {
        None => value.to_owned(),
        Some((address, _)) => format!("{address}?{}", super::store::REDACTED),
    }
}

#[cfg(test)]
mod tests {
    use super::{in_full, without_query, SHOWN};

    #[test]
    fn a_setting_on_the_list_is_displayed_and_one_beside_it_is_not() {
        assert!(in_full("DATA_ROOT"));
        assert!(in_full("INDEXER_URL"));
        assert!(in_full("USENET_VALIDATED"));
        assert!(!in_full("INDEXER_APIKEY"));
        assert!(!in_full("USENET_PASS"));
    }

    #[test]
    fn a_setting_nobody_has_decided_about_is_not_displayed() {
        // The property the list is bought for, and the one a marker list cannot have:
        // none of these carries a word any keyword rule recognises, and every one of
        // them is a credential somewhere.
        for name in [
            "OPENVPN_USER",
            "PLEX_CLAIM",
            "DISCORD_WEBHOOK",
            "DB_PWD",
            "SESSION_SALT",
            "DATABASE_URL",
            "SOME_SERVICE_ADDED_NEXT_YEAR",
        ] {
            assert!(!in_full(name), "{name} is displayed and nobody said why");
        }
    }

    #[test]
    fn a_name_written_with_spaces_around_it_is_still_the_same_name() {
        assert!(in_full("  DATA_ROOT  "));
    }

    #[test]
    fn a_displayed_address_keeps_its_address_and_loses_its_query() {
        assert_eq!(
            without_query("https://indexer.example/api"),
            "https://indexer.example/api"
        );
        // Assembled rather than written out, and a placeholder rather than a plausible
        // key: what this fixture has to be is withheld, and a run of hex in source is a
        // secret scanner's finding for as long as the commit exists.
        let key = ["the", "indexer", "key"].join("-");
        let shown = without_query(&format!("https://indexer.example/api?apikey={key}"));
        assert!(shown.starts_with("https://indexer.example/api?"), "{shown}");
        assert!(!shown.contains(&key), "{shown}");
    }

    #[test]
    fn no_setting_is_written_down_twice() {
        // Two entries for one name are two reasons, and the second is the one nobody
        // reads when they change the first.
        let mut names: Vec<&str> = SHOWN.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        let mut unique = names.clone();
        unique.dedup();
        assert_eq!(names, unique, "a setting is on the list twice");
    }
}
