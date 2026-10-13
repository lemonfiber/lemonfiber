use crate::device_keys::identity::{read_all, SETTING};

pub(crate) const DEVICE_KEY_APPS: &str = SETTING;

pub(crate) const CORE: [&str; 1] = [DEVICE_KEY_APPS];

const OWN_PREFIX: &str = "LEMONFIBER_";

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
