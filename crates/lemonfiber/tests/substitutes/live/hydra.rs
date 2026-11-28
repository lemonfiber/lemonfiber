//! `NZBHydra2` standing in for the stack's indexer, given the site as its operator gives it
//! one: in its own configuration, signed in as the administrator lemonfiber minted for it.

use super::core::{strings_under, Core};
use super::http;

/// The plugin, and its service whose configuration holds the indexers it searches.
pub const PLUGIN: &str = "nzbhydra2";

/// The credential the name of the administrator lemonfiber minted for it is held under.
const ADMINISTRATOR_NAME: &str = "nzbhydra2:admin-username";

/// The credential that administrator's password is held under.
const ADMINISTRATOR_PASSWORD: &str = "nzbhydra2:admin-password";

/// Where its whole configuration is read and written.
const CONFIGURATION: &str = "/internalapi/config";

/// Every id it hands the site as the site's own, so a curator's search by an id reaches the
/// site as it was asked rather than as a query made from a lookup outside the house.
const IDS: [&str; 5] = ["TVDB", "TVRAGE", "TVMAZE", "IMDB", "TMDB"];

/// Every kind of search it sends the site.
const SEARCHES: [&str; 5] = ["SEARCH", "TVSEARCH", "MOVIE", "AUDIO", "BOOK"];

/// What the site is listed under.
const LISTED: &str = "lemonfiber arm";

/// The administrator's name and password, as `core` reveals them.
///
/// # Errors
///
/// Where either is not revealed.
fn administrator(core: &Core) -> Result<(String, String), String> {
    let revealed = |credential: &str| {
        let ran = core.run(&["credentials", "--reveal", credential, "--confirm"], "");
        strings_under(&ran.said, "value")
            .into_iter()
            .next()
            .ok_or_else(|| format!("{credential} is not revealed:\n{}", ran.printed))
    };
    Ok((
        revealed(ADMINISTRATOR_NAME)?,
        revealed(ADMINISTRATOR_PASSWORD)?,
    ))
}

/// The site at `site`, answering `key`, added to the indexers `NZBHydra2` searches, enabled
/// for every search.
///
/// # Errors
///
/// Where its configuration cannot be read or written, or it refuses the site.
pub fn site_added(core: &Core, site: &str, key: &str) -> Result<(), String> {
    let account = administrator(core)?;
    let port = super::published(PLUGIN, PLUGIN)
        .ok_or_else(|| format!("the bundle names no port for {PLUGIN}"))?;
    let read = http::call_as("GET", port, CONFIGURATION, (&account.0, &account.1), None)?;
    if read.status != 200 {
        return Err(format!("{}: {}", read.status, read.body));
    }
    let mut configuration: serde_json::Value =
        serde_json::from_str(&read.body).map_err(|why| format!("{why}: {}", read.body))?;
    let indexers = configuration
        .get_mut("indexers")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| format!("no indexers in {}", read.body))?;
    indexers.push(serde_json::json!({
        "name": LISTED,
        "host": site,
        "apiKey": key,
        "searchModuleType": "NEWZNAB",
        "state": "ENABLED",
        "enabledForSearchSource": "BOTH",
        "supportedSearchIds": IDS,
        "supportedSearchTypes": SEARCHES,
        "allCapsChecked": true,
        "configComplete": true,
    }));
    let written = http::call_as(
        "PUT",
        port,
        CONFIGURATION,
        (&account.0, &account.1),
        Some(&configuration.to_string()),
    )?;
    let saved: serde_json::Value = serde_json::from_str(&written.body)
        .map_err(|why| format!("{}: {why}: {}", written.status, written.body))?;
    if saved.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(format!("{}: {}", written.status, written.body))
    }
}
