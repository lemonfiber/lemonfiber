pub(crate) const DEVICE_KEY_APPS: &str = "LEMONFIBER_DEVICE_KEY_APPS";

pub(crate) const CORE: [&str; 1] = [DEVICE_KEY_APPS];

const OWN_PREFIX: &str = "LEMONFIBER_";

const FINGERPRINT_DIGITS: usize = 64;

const TEAM_LENGTH: usize = 10;

pub(crate) fn is_core(name: &str) -> bool {
    CORE.contains(&name)
}

pub(crate) fn is_configs(name: &str) -> bool {
    name.starts_with(OWN_PREFIX) && !is_core(name)
}

pub(crate) fn refusal(name: &str, value: &str) -> Option<String> {
    if name != DEVICE_KEY_APPS {
        return None;
    }
    let wrong: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|identity| !is_android(identity) && !is_ios(identity))
        .collect();
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
                    one
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

fn is_android(identity: &str) -> bool {
    identity
        .strip_prefix("android:")
        .and_then(|rest| rest.split_once('@'))
        .is_some_and(|(package, fingerprint)| {
            is_package(package)
                && fingerprint.len() == FINGERPRINT_DIGITS
                && fingerprint.chars().all(|digit| digit.is_ascii_hexdigit())
        })
}

fn is_package(package: &str) -> bool {
    let parts: Vec<&str> = package.split('.').collect();
    parts.len() > 1
        && parts.iter().all(|part| {
            part.chars()
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic())
                && part
                    .chars()
                    .all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
        })
}

fn is_ios(identity: &str) -> bool {
    identity
        .strip_prefix("ios:")
        .and_then(|rest| rest.split_once('.'))
        .is_some_and(|(team, bundle)| {
            team.len() == TEAM_LENGTH
                && team
                    .chars()
                    .all(|letter| letter.is_ascii_uppercase() || letter.is_ascii_digit())
                && !bundle.is_empty()
                && bundle
                    .chars()
                    .all(|letter| letter.is_ascii_alphanumeric() || letter == '.' || letter == '-')
        })
}
