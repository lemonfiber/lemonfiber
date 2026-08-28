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
//! Beside it, [`TROUBLE`] keys what to do when it does not work by the symptom
//! somebody reports rather than by the cause, because the cause is the thing they
//! cannot yet say. Where a symptom has more than one cause, each carries how to tell
//! it from the others: a cause offered without that is a guess presented as an
//! answer, and the requirements here ask that the causes be told apart.
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

/// One thing that could be behind a symptom, and how to tell it from the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Cause {
    /// What is wrong.
    pub because: &'static str,
    /// How to tell this cause from the others under the same symptom.
    pub tell: &'static str,
    /// What to do about it.
    pub fix: &'static str,
}

/// Something somebody reports, and what is likely behind it.
///
/// Keyed by the symptom rather than the cause: the person asking has the symptom,
/// and which cause it is is the thing they cannot yet say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Trouble {
    /// What somebody says is happening, in their words.
    pub symptom: &'static str,
    /// What is likely behind it, most likely first.
    pub causes: &'static [Cause],
}

/// Every symptom this product has an answer for.
pub const TROUBLE: &[Trouble] = &[
    Trouble {
        symptom: "The app cannot find the server",
        causes: &[
            Cause {
                because: "The address is wrong, or it has changed.",
                tell: "Every device fails the same way, including one that worked yesterday.",
                fix: "Check it against what `lemonfiber front-door` says now. That address is \
                      read from this machine at the moment of asking, so a machine that was \
                      renamed answers differently from the note somebody wrote down.",
            },
            Cause {
                because: "The device is on a different network from the server.",
                tell: "The device has working internet, and another device in the house can \
                       reach the server.",
                fix: "Put it back on the home Wi-Fi. A phone that fell back to mobile data is \
                      the common one, and it looks like a broken address rather than a network \
                      it is on.",
            },
            Cause {
                because: "The server is not running.",
                tell: "No device can reach it, and this is the one you can answer without \
                       leaving your chair.",
                fix: "`lemonfiber status` says whether it is, and `lemonfiber up` starts it.",
            },
        ],
    },
    Trouble {
        symptom: "The device is on the guest Wi-Fi",
        causes: &[Cause {
            because: "Guest networks keep devices from reaching each other, which is what \
                      they are for.",
            tell: "The device has working internet and the address is right, and nothing \
                   answers — which looks exactly like a wrong address.",
            fix: "Join the ordinary home network. There is nothing to change on the server: a \
                  guest network is doing its job.",
        }],
    },
    Trouble {
        symptom: "The library is empty after signing in",
        causes: &[
            Cause {
                because: "Nothing has been scanned yet.",
                tell: "Nobody sees anything, including you.",
                fix: "A new library, or one whose files moved, has nothing until it is \
                      scanned. Jellyfin scans on a schedule and can be told to now.",
            },
            Cause {
                because: "The account has been given access to no library.",
                tell: "Somebody else sees content and this person does not.",
                fix: "The account exists and can sign in — what it lacks is permission. Give \
                      it the libraries it should see in Jellyfin's user settings.",
            },
        ],
    },
    Trouble {
        symptom: "It worked and now the app cannot connect",
        causes: &[Cause {
            because: "The address changed and the app remembers the old one.",
            tell: "It stopped for everybody at once, and a browser opened at the current \
                   address still works.",
            fix: "Change the saved server in the app to what `lemonfiber front-door` says \
                  now. Where an app offers no way to edit it, remove the server and add it \
                  again.",
        }],
    },
];

/// The guidance in full, for a surface that shows all of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Guidance {
    /// Every device, in the order somebody is likely to be holding one.
    pub devices: Vec<Device>,
    /// What to do when it does not work, keyed by the symptom.
    pub trouble: Vec<Trouble>,
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
        trouble: TROUBLE.to_vec(),
        only_at_home: ONLY_AT_HOME,
        nothing_is_installed: NOTHING_IS_INSTALLED,
    }
}

#[cfg(test)]
mod tests {
    use super::{guidance, Support, DEVICES, NOTHING_IS_INSTALLED, ONLY_AT_HOME, TROUBLE};

    /// Every cause says how to tell it from the others under its symptom.
    ///
    /// The requirements ask that the causes be told apart, not listed. A cause with
    /// no way to tell it from its neighbour is a guess offered as an answer, and a
    /// reader given three of those is no better off than with none.
    #[test]
    fn every_cause_says_how_to_tell_it_from_the_others() {
        let silent: Vec<&str> = TROUBLE
            .iter()
            .flat_map(|one| one.causes)
            .filter(|cause| cause.tell.split_whitespace().count() < 4)
            .map(|cause| cause.because)
            .collect();

        assert!(
            silent.is_empty(),
            "these give no way to tell them apart: {silent:?}"
        );
        assert!(
            !TROUBLE.is_empty(),
            "no symptom is answered, so this checked nothing"
        );
    }

    /// Every cause says what is wrong and what to do about it.
    #[test]
    fn every_cause_carries_a_fix() {
        let thin: Vec<&str> = TROUBLE
            .iter()
            .flat_map(|one| one.causes)
            .filter(|cause| cause.fix.is_empty() || cause.because.is_empty())
            .map(|cause| cause.because)
            .collect();

        assert!(thin.is_empty(), "these name no fix: {thin:?}");
    }

    /// A symptom with one cause needs no telling apart, and one with several does.
    ///
    /// Both shapes are here on purpose: a guest network has one cause and an app that
    /// cannot find the server has three. What would be wrong is a symptom with none.
    #[test]
    fn every_symptom_offers_at_least_one_cause() {
        let empty: Vec<&str> = TROUBLE
            .iter()
            .filter(|one| one.causes.is_empty())
            .map(|one| one.symptom)
            .collect();

        assert!(empty.is_empty(), "these answer nothing: {empty:?}");
        assert!(
            TROUBLE.iter().any(|one| one.causes.len() > 1),
            "no symptom has causes to tell apart, so the rule above is checking nothing"
        );
    }

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
