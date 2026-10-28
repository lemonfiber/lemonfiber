//! The cases an adapter's recordings and its live pass must answer for one capability.
//!
//! Every operation answers inside its contract, and every adapter refuses a call made
//! without its key.

use serde_json::Value;

use crate::{Capability, Operation};

/// The statuses an adapter refuses a call without its key with.
const UNKEYED: &[u16] = &[401];

/// The name of the case that refuses a call made without the key.
pub const REFUSES_WITHOUT_THE_KEY: &str = "refuses-without-the-key";

/// What a placeholder's text opens with.
const OPENS: char = '{';

/// What a placeholder's text closes with.
const CLOSES: char = '}';

/// The placeholder for a credential minted for the case alone.
const MINTED: &str = "minted";

/// The placeholder for where the upstream's container holds the internal certificate.
const CERTIFICATE: &str = "certificate";

/// What a placeholder naming a field of `upstream.json` begins with.
const UPSTREAM: &str = "upstream:";

/// What separates an earlier case from the pointer into its answer.
const POINTER: char = '#';

/// A string in a case's ask that is filled before the case is posted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placeholder {
    /// A credential minted for this case alone.
    Minted,
    /// This field of the `upstream.json` the adapter holds.
    Upstream(String),
    /// A credential minted for this case, written to `upstream.json` as this field once
    /// the adapter answers the case with a success.
    MintedUpstream(String),
    /// The path, inside the container of the upstream the adapter fronts, of the PKCS#12
    /// holding the internal certificate, written for the case with an empty password.
    Certificate,
    /// The string at a JSON pointer in the answer to an earlier case.
    Answered {
        /// The case, as `<capability>@<major>/<case>`.
        case: String,
        /// Where in its answer.
        pointer: String,
    },
}

impl Placeholder {
    /// The string at `pointer` in the answer to `operation` of `capability`'s major.
    #[must_use]
    pub fn answer(capability: &str, major: u32, operation: &str, pointer: &str) -> Self {
        Self::Answered {
            case: format!("{capability}@{major}/{}", answers(operation)),
            pointer: pointer.to_owned(),
        }
    }

    /// The placeholder as an ask holds it.
    #[must_use]
    pub fn written(&self) -> String {
        let inner = match self {
            Self::Minted => MINTED.to_owned(),
            Self::Certificate => CERTIFICATE.to_owned(),
            Self::Upstream(field) => format!("{UPSTREAM}{field}"),
            Self::MintedUpstream(field) => format!("{MINTED}:{UPSTREAM}{field}"),
            Self::Answered { case, pointer } => format!("{case}{POINTER}{pointer}"),
        };
        format!("{OPENS}{inner}{CLOSES}")
    }

    /// The placeholder `text` is, where it is exactly one.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let inner = text.strip_prefix(OPENS)?.strip_suffix(CLOSES)?;
        if inner.contains([OPENS, CLOSES]) {
            return None;
        }
        if inner == MINTED {
            return Some(Self::Minted);
        }
        if inner == CERTIFICATE {
            return Some(Self::Certificate);
        }
        if let Some(field) = inner
            .strip_prefix(MINTED)
            .and_then(|rest| rest.strip_prefix(':'))
            .and_then(|rest| rest.strip_prefix(UPSTREAM))
        {
            return named(field).map(Self::MintedUpstream);
        }
        if let Some(field) = inner.strip_prefix(UPSTREAM) {
            return named(field).map(Self::Upstream);
        }
        let (case, pointer) = inner.split_once(POINTER)?;
        let (contract, name) = case.split_once('/')?;
        let (capability, major) = contract.split_once('@')?;
        let sound = !capability.is_empty()
            && major.parse::<u32>().is_ok()
            && !name.is_empty()
            && (pointer.is_empty() || pointer.starts_with('/'));
        sound.then(|| Self::Answered {
            case: case.to_owned(),
            pointer: pointer.to_owned(),
        })
    }
}

impl From<Placeholder> for String {
    fn from(placeholder: Placeholder) -> Self {
        placeholder.written()
    }
}

/// `field`, where it names one.
fn named(field: &str) -> Option<String> {
    (!field.is_empty() && !field.contains(':')).then(|| field.to_owned())
}

/// The name of the case that asks `operation` under the key.
#[must_use]
pub fn answers(operation: &str) -> String {
    format!("{operation}-answers")
}

/// What a case holds an answer to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// Its status is one of these.
    Status(&'static [u16]),
    /// It is an answer the operation's contract declares.
    Conforms,
}

/// One case of a capability's conformance.
#[derive(Debug, Clone)]
pub struct Case<'c> {
    /// Its name, which its recording is filed under.
    pub case: String,
    /// The operation it asks.
    pub operation: &'c Operation,
    /// Whether it is asked under the plugin's key.
    pub keyed: bool,
    /// What its answer is held to.
    pub expect: Expect,
    /// Whether install asks it of the running adapter as well.
    pub live: bool,
    /// What it posts, its placeholders still to be filled.
    pub asks: Value,
    /// The media type it is asked for, where its operation is bound to one.
    pub media_type: Option<&'static str>,
}

/// Every case of `capability`, in the order they are published.
#[must_use]
pub fn cases(capability: &Capability) -> Vec<Case<'_>> {
    let unkeyed = capability.operations.first().map(|operation| Case {
        case: REFUSES_WITHOUT_THE_KEY.to_owned(),
        operation,
        keyed: false,
        expect: Expect::Status(UNKEYED),
        live: true,
        asks: Value::Object(serde_json::Map::new()),
        media_type: None,
    });
    unkeyed
        .into_iter()
        .chain(capability.operations.iter().flat_map(keyed))
        .collect()
}

/// Every case of `capability` an adapter whose upstream files `files` is asked: every
/// case of an operation bound to no kind of media, and those of the media types it files.
#[must_use]
pub fn filed<'c>(capability: &'c Capability, files: &[String]) -> Vec<Case<'c>> {
    cases(capability)
        .into_iter()
        .filter(|case| {
            case.media_type
                .is_none_or(|media_type| files.iter().any(|filed| filed == media_type))
        })
        .collect()
}

/// The cases asking `operation` under the key: one, or one per media type it is bound to.
fn keyed(operation: &Operation) -> Vec<Case<'_>> {
    let asked = |case: String, asks: &Value, media_type| Case {
        case,
        operation,
        keyed: true,
        expect: Expect::Conforms,
        live: false,
        asks: asks.clone(),
        media_type,
    };
    if operation.filed.is_empty() {
        return vec![asked(answers(operation.name), &operation.asks, None)];
    }
    operation
        .filed
        .iter()
        .map(|(media_type, asks)| {
            asked(
                format!("{}-{media_type}", answers(operation.name)),
                asks,
                Some(*media_type),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
