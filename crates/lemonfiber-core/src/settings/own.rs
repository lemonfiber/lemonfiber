use crate::device_keys::identity::{read_all, SETTING};

pub(crate) const DEVICE_KEY_APPS: &str = SETTING;

/// Where the dashboard's search box sends a search, the terms appended to it.
pub(crate) const DASHBOARD_SEARCH: &str = "LEMONFIBER_DASHBOARD_SEARCH";

/// Where the dashboard's search box sends a search where nothing is recorded.
pub(crate) const DEFAULT_SEARCH: &str = "https://duckduckgo.com/?q=";

/// How many hours from now the live TV guide answers each channel's programmes across.
pub(crate) const GUIDE_HOURS: &str = "LEMONFIBER_GUIDE_HOURS";

/// How many hours the live TV guide answers across where nothing is recorded.
pub(crate) const DEFAULT_GUIDE_HOURS: u32 = 12;

/// How many codes to sign a member in by one source may have waiting at once.
pub(crate) const CODES_WAITING: &str = "LEMONFIBER_SIGN_IN_CODES_WAITING";

/// How many codes to sign a member in by one source may ask for in each period.
pub(crate) const CODES_PER_PERIOD: &str = "LEMONFIBER_SIGN_IN_CODES_PER_PERIOD";

/// That period, in whole minutes.
pub(crate) const CODES_PERIOD: &str = "LEMONFIBER_SIGN_IN_CODES_PERIOD";

/// Whether finishing a sign-in by a code must come from the network address that asked.
pub(crate) const CODES_BY_ADDRESS: &str = "LEMONFIBER_SIGN_IN_CODES_BY_ADDRESS";

/// What each whole-number setting of sign-ins by a code holds, the range it takes, and
/// what it is where nothing is recorded.
pub(crate) const CODE_COUNTS: [(&str, &str, std::ops::RangeInclusive<u32>, u32); 3] = [
    (
        CODES_WAITING,
        "how many codes to sign a member in by one source may have waiting at once",
        1..=100,
        3,
    ),
    (
        CODES_PER_PERIOD,
        "how many codes to sign a member in by one source may ask for in each period",
        1..=1000,
        10,
    ),
    (CODES_PERIOD, "that period in whole minutes", 1..=1440, 15),
];

/// How fast a backup writes at most, in MiB a second, or `off`.
pub(crate) const BACKUP_LIMIT: &str = "LEMONFIBER_BACKUP_LIMIT";

/// The limits a backup may be held to, in MiB a second.
pub(crate) const BACKUP_LIMITS: std::ops::RangeInclusive<u32> = 1..=100_000;

/// What `LEMONFIBER_BACKUP_LIMIT` holds where it holds no limit.
pub(crate) const UNLIMITED: &str = "off";

/// How many minutes a running backup or restore may write nothing before it reads stalled.
pub(crate) const COPY_STALLED: (&str, &str, std::ops::RangeInclusive<u32>, u32) = (
    "LEMONFIBER_COPY_STALLED_MINUTES",
    "how many minutes a running backup or restore may write nothing before it reads stalled",
    1..=1440,
    5,
);

/// The domain the household's names are under where nothing is recorded.
pub(crate) const DEFAULT_DOMAIN: &str = "home.local";

/// The domain the household's names are under.
pub(crate) const DOMAIN: &str = lemonfiber_manifest::container::DOMAIN;

/// The house's language and region.
pub(crate) const LOCALE: &str = crate::locale::SETTING;

/// Each core-published setting held under another name, with that name.
pub(crate) const PUBLISHED_FROM: [(&str, &str); 2] = [
    (
        lemonfiber_manifest::container::TIMEZONE,
        crate::config::ZONE_KEY,
    ),
    (lemonfiber_manifest::container::LOCALE, LOCALE),
];

/// Whether finishing a sign-in by a code must come from the address that asked, where
/// nothing is recorded.
pub(crate) const DEFAULT_BY_ADDRESS: bool = false;

/// What one of the core's own settings holds where nothing is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unset {
    /// This text.
    Text(&'static str),
    /// This whole number.
    Number(u32),
    /// This switch.
    Switch(bool),
    /// None of the entries a list of them holds.
    Nothing,
}

impl Unset {
    /// The value it is written as, where it is one.
    pub(crate) fn written(self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text.to_owned()),
            Self::Number(number) => Some(number.to_string()),
            Self::Switch(true) => Some(crate::config::reaching::SWITCHED_ON.to_owned()),
            Self::Switch(false) => Some(crate::config::reaching::SWITCHED_OFF.to_owned()),
            Self::Nothing => None,
        }
    }
}

/// The core's own settings, each with what it holds where nothing is recorded.
pub(crate) const CORE: [(&str, Unset); 12] = [
    (DEVICE_KEY_APPS, Unset::Nothing),
    (DASHBOARD_SEARCH, Unset::Text(DEFAULT_SEARCH)),
    (GUIDE_HOURS, Unset::Number(DEFAULT_GUIDE_HOURS)),
    (CODE_COUNTS[0].0, Unset::Number(CODE_COUNTS[0].3)),
    (CODE_COUNTS[1].0, Unset::Number(CODE_COUNTS[1].3)),
    (CODE_COUNTS[2].0, Unset::Number(CODE_COUNTS[2].3)),
    (CODES_BY_ADDRESS, Unset::Switch(DEFAULT_BY_ADDRESS)),
    (BACKUP_LIMIT, Unset::Text(UNLIMITED)),
    (COPY_STALLED.0, Unset::Number(COPY_STALLED.3)),
    (DOMAIN, Unset::Text(DEFAULT_DOMAIN)),
    (LOCALE, Unset::Text(crate::locale::DEFAULT)),
    (
        crate::plex_link::SETTING,
        Unset::Text(crate::plex_link::Mode::UNSET.word()),
    ),
];

const OWN_PREFIX: &str = "LEMONFIBER_";

pub(crate) fn is_core(name: &str) -> bool {
    unset(name).is_some()
}

/// What the core's own setting `name` holds where nothing is recorded, where it is one.
pub(crate) fn unset(name: &str) -> Option<Unset> {
    CORE.iter()
        .find(|(core, _)| *core == name)
        .map(|(_, unset)| *unset)
}

pub(crate) fn is_configs(name: &str) -> bool {
    name.starts_with(OWN_PREFIX) && !is_core(name)
}

pub(crate) fn refusal(name: &str, value: &str) -> Option<String> {
    if name == crate::plex_link::SETTING {
        return crate::plex_link::refusal(value);
    }
    if name == DOMAIN {
        return (!is_domain(value)).then(|| {
            format!(
                "{name} is the domain the household's names are under, labels of letters, \
                 digits and hyphens joined by dots, and {value} is not one"
            )
        });
    }
    if name == DASHBOARD_SEARCH {
        return (!is_web_address(value)).then(|| {
            format!(
                "{name} is where the dashboard's search box sends a search, an absolute https \
                 or http address the terms are appended to, and {value} is not one"
            )
        });
    }
    if name == LOCALE {
        return crate::locale::Locale::read(value).err();
    }
    if name == GUIDE_HOURS {
        return guide_hours(value).err();
    }
    if let Some(counted) = CODE_COUNTS.iter().find(|(counted, ..)| *counted == name) {
        return code_count(counted, value).err();
    }
    if name == CODES_BY_ADDRESS {
        return switched(name, value).err();
    }
    if name == BACKUP_LIMIT {
        return backup_limit(value).err();
    }
    if name == COPY_STALLED.0 {
        return code_count(&COPY_STALLED, value).err();
    }
    if name != DEVICE_KEY_APPS {
        return None;
    }
    let (_, wrong) = read_all(value);
    (!wrong.is_empty()).then(|| {
        format!(
            "{name} is a comma-separated list of android:<package>@<the signing certificate's \
             fingerprint, 64 hexadecimal digits> and ios:<team>.<bundle>, and {} is in neither \
             form",
            wrong
                .iter()
                .map(|one| if one.is_empty() {
                    "an empty entry"
                } else {
                    one.as_str()
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// The hours `value` holds, where it is a whole number of hours a guide may be asked
/// across.
///
/// # Errors
///
/// Why `value` is not one, naming the setting.
pub(crate) fn guide_hours(value: &str) -> Result<u32, String> {
    let hours = crate::ports::service::GUIDE_HOURS;
    value
        .parse::<u32>()
        .ok()
        .filter(|held| hours.contains(held))
        .ok_or_else(|| {
            format!(
                "{GUIDE_HOURS} is how many hours from now the live TV guide answers across, a \
                 whole number from {} to {}, and {value} is not one",
                hours.start(),
                hours.end()
            )
        })
}

/// The number `value` holds for the setting of sign-ins by a code `counted` describes,
/// where it is a whole number within its range.
///
/// # Errors
///
/// Why `value` is not one, naming the setting.
/// The limit `value` holds for a backup in MiB a second, none for `off`, or why it holds neither.
pub(crate) fn backup_limit(value: &str) -> Result<Option<u32>, String> {
    if value == UNLIMITED {
        return Ok(None);
    }
    value
        .parse::<u32>()
        .ok()
        .filter(|held| BACKUP_LIMITS.contains(held))
        .map(Some)
        .ok_or_else(|| {
            format!(
                "{BACKUP_LIMIT} is how fast a backup writes at most, a whole number of MiB a \
                 second from {} to {} or {UNLIMITED}, and {value} is neither",
                BACKUP_LIMITS.start(),
                BACKUP_LIMITS.end()
            )
        })
}

pub(crate) fn code_count(
    (name, holds, range, _): &(&str, &str, std::ops::RangeInclusive<u32>, u32),
    value: &str,
) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|held| range.contains(held))
        .ok_or_else(|| {
            format!(
                "{name} is {holds}, a whole number from {} to {}, and {value} is not one",
                range.start(),
                range.end()
            )
        })
}

/// Whether `value` switches the setting `name` on, where it reads as on or as off.
///
/// # Errors
///
/// Why `value` is neither, naming the setting.
pub(crate) fn switched(name: &str, value: &str) -> Result<bool, String> {
    if crate::config::reads_as_on(value) {
        Ok(true)
    } else if crate::config::reads_as_off(value) {
        Ok(false)
    } else {
        Err(format!("{name} is on or off, and {value} is neither"))
    }
}

/// Whether `value` is a domain a name can be put in front of: labels of ASCII letters,
/// digits and hyphens, none empty, none longer than 63, none opening or closing with a
/// hyphen, joined by dots.
pub(crate) fn is_domain(value: &str) -> bool {
    value.len() <= 253
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|letter| letter.is_ascii_alphanumeric() || letter == '-')
        })
}

/// Whether `value` is an absolute `https` or `http` address with a host.
pub(crate) fn is_web_address(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "https" | "http")
            && url.host_str().is_some_and(|host| !host.is_empty())
    }) && !value
        .chars()
        .any(|letter| letter.is_whitespace() || letter.is_control())
}
