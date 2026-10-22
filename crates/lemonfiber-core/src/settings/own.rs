use crate::device_keys::identity::{read_all, SETTING};

pub(crate) const DEVICE_KEY_APPS: &str = SETTING;

/// Where the dashboard's search box sends a search, the terms appended to it.
pub(crate) const DASHBOARD_SEARCH: &str = "LEMONFIBER_DASHBOARD_SEARCH";

/// Where the dashboard's search box sends a search where nothing is recorded.
pub(crate) const DEFAULT_SEARCH: &str = "https://duckduckgo.com/?q=";

pub(crate) const CORE: [&str; 2] = [DEVICE_KEY_APPS, DASHBOARD_SEARCH];

const OWN_PREFIX: &str = "LEMONFIBER_";

pub(crate) fn is_core(name: &str) -> bool {
    CORE.contains(&name)
}

pub(crate) fn is_configs(name: &str) -> bool {
    name.starts_with(OWN_PREFIX) && !is_core(name)
}

pub(crate) fn refusal(name: &str, value: &str) -> Option<String> {
    if name == DASHBOARD_SEARCH {
        return (!is_web_address(value)).then(|| {
            format!(
                "{name} is where the dashboard's search box sends a search, an absolute https \
                 or http address the terms are appended to, and {value} is not one"
            )
        });
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

/// Whether `value` is an absolute `https` or `http` address with a host.
pub(crate) fn is_web_address(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "https" | "http")
            && url.host_str().is_some_and(|host| !host.is_empty())
    }) && !value
        .chars()
        .any(|letter| letter.is_whitespace() || letter.is_control())
}
