//! Which app to use on which device, and where the answer is to use something else.
//!
//! One entry per device category, each carrying how well served it is, what to use,
//! what is worth knowing before starting, and where support is poor what to do
//! instead. The same on every machine: nothing here is read from disk or asked of
//! the engine.
//!
//! A device marked [`Support::Poor`] carries an alternative, and a test refuses one
//! that does not. The browser is present as [`Support::Fallback`] — no installation,
//! any screen — so no device category is without an answer.
//!
//! Nothing is installed from here. A link or a store page is named where one exists;
//! the rest is instructions.

use serde::Serialize;

/// How well a device is served.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    /// An official app that works. Most people, most of the time.
    Good,
    /// It works, with something worth knowing before starting.
    Workable,
    /// Poorly served. Said plainly, with somewhere else to go.
    Poor,
    /// No installation, works anywhere, and is never unavailable.
    Fallback,
}

impl Support {
    /// Whether somebody should be told to try something else first.
    #[must_use]
    pub const fn wants_an_alternative(self) -> bool {
        matches!(self, Self::Poor)
    }
}

/// A kind of device somebody in the house might watch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Device {
    /// What somebody would call the device they are holding.
    pub device: &'static str,
    /// How well served it is.
    pub support: Support,
    /// What to use on it.
    pub client: &'static str,
    /// What is worth knowing before starting, where anything is.
    pub caution: Option<&'static str>,
    /// What to do instead where this is a bad device to be stuck with.
    pub instead: Option<&'static str>,
}

/// Every device this product has something to say about.
///
/// Ordered by how common the device is, not alphabetically. Surfaces render this
/// order as given.
pub const DEVICES: &[Device] = &[
    Device {
        device: "Android phone or tablet",
        support: Support::Good,
        client: "the official Jellyfin app, from Google Play",
        caution: None,
        instead: None,
    },
    Device {
        device: "iPhone or iPad",
        support: Support::Good,
        client: "the official Jellyfin app, from the App Store",
        caution: None,
        instead: None,
    },
    Device {
        device: "Android TV or Fire TV",
        support: Support::Good,
        client: "the official Jellyfin app, from the device's own store",
        caution: None,
        instead: None,
    },
    Device {
        device: "Apple TV",
        support: Support::Workable,
        client: "the official Jellyfin app",
        caution: Some(
            "Other apps are widely used here and some people prefer them. The official one \
             is the answer if you do not already have an opinion.",
        ),
        instead: None,
    },
    Device {
        device: "A web browser",
        support: Support::Fallback,
        client: "no app at all — open the address",
        caution: Some(
            "Nothing to install, and it works on anything with a screen. This is the answer \
             whenever an app for the device is missing, broken, or more trouble than it is worth.",
        ),
        instead: None,
    },
    Device {
        device: "Smart TV (LG, Samsung)",
        support: Support::Poor,
        client: "an app exists for some models",
        caution: Some(
            "This is the weakest part of the landscape. Whether there is a working app depends \
             on the make and on how old the television is, and a set that worked last year can \
             stop after an update nobody asked for.",
        ),
        instead: Some(
            "A streaming stick plugged into the television is about the price of a takeaway and \
             turns this into the well-served case above. Casting from a phone works too, and \
             costs nothing to try first.",
        ),
    },
    Device {
        device: "Kodi",
        support: Support::Workable,
        client: "the Jellyfin plugin",
        caution: Some(
            "For somebody who already runs Kodi and wants to keep it. Not worth installing Kodi \
             in order to reach this stack.",
        ),
        instead: None,
    },
];

/// True of every device. Rendered once per report, never per device.
pub const ONLY_AT_HOME: &str =
    "All of this works on your home network and nowhere else. Away from the house, none of \
     these apps will find the server — that is how it is meant to be for now, not a fault.";

/// What lemonfiber will not do, said where somebody might expect otherwise.
pub const NOTHING_IS_INSTALLED: &str =
    "lemonfiber does not install anything on your device and cannot. What is here is where to \
     look and what to choose; the installing is yours.";

/// The guidance in full, for a surface that shows all of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Guidance {
    /// Every device, in the order somebody is likely to be holding one.
    pub devices: Vec<Device>,
    /// True of every device, said once.
    pub only_at_home: &'static str,
    /// What this will not do for them.
    pub nothing_is_installed: &'static str,
}

/// What to use, for every device this product has an answer for.
#[must_use]
pub fn guidance() -> Guidance {
    Guidance {
        devices: DEVICES.to_vec(),
        only_at_home: ONLY_AT_HOME,
        nothing_is_installed: NOTHING_IS_INSTALLED,
    }
}

#[cfg(test)]
mod tests {
    use super::{guidance, Support, DEVICES, NOTHING_IS_INSTALLED, ONLY_AT_HOME};

    /// A browser is present, and it is the entry that needs nothing installed.
    #[test]
    fn a_browser_is_offered_and_needs_nothing_installed() {
        let always: Vec<&super::Device> = DEVICES
            .iter()
            .filter(|one| one.support == Support::Fallback)
            .collect();

        assert_eq!(
            always.len(),
            1,
            "one entry always works, and it is the browser"
        );
        let asks_for_an_app: Vec<&str> = always
            .iter()
            .filter(|one| !one.client.contains("no app"))
            .map(|one| one.device)
            .collect();
        assert!(asks_for_an_app.is_empty(), "{asks_for_an_app:?}");
        let not_a_browser: Vec<&str> = always
            .iter()
            .filter(|one| !one.device.to_lowercase().contains("browser"))
            .map(|one| one.device)
            .collect();
        assert!(not_a_browser.is_empty(), "{not_a_browser:?}");
    }

    /// Every device marked [`Support::Poor`] carries an alternative, and at least
    /// one device is so marked — without the second half this passes on an empty
    /// filter.
    #[test]
    fn a_device_that_is_poorly_served_says_what_to_do_instead() {
        let poor: Vec<&str> = DEVICES
            .iter()
            .filter(|one| one.support.wants_an_alternative())
            .filter(|one| one.instead.is_none())
            .map(|one| one.device)
            .collect();

        assert!(
            poor.is_empty(),
            "these are named as poorly served and offer nothing else to try: {poor:?}"
        );
        assert!(
            DEVICES.iter().any(|one| one.support.wants_an_alternative()),
            "no device is named as poorly served, so the rule above checked nothing"
        );
    }

    /// Every device names something to use.
    #[test]
    fn every_device_says_what_to_use_on_it() {
        let silent: Vec<&str> = DEVICES
            .iter()
            .filter(|one| one.client.is_empty())
            .map(|one| one.device)
            .collect();

        assert!(silent.is_empty(), "these name no client: {silent:?}");
        assert!(
            DEVICES.len() > 5,
            "the table is too short to be a landscape"
        );
    }

    /// Both statements true of every device are present.
    #[test]
    fn the_limits_that_hold_for_everything_are_stated() {
        let all = guidance();

        assert_eq!(all.devices.len(), DEVICES.len());
        assert!(
            all.only_at_home.contains("home network"),
            "the household is told where this works: {ONLY_AT_HOME}"
        );
        assert!(
            all.nothing_is_installed.contains("does not install"),
            "and what lemonfiber will not do: {NOTHING_IS_INSTALLED}"
        );
    }
}
