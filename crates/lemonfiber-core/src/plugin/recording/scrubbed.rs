//! Taking every credential a recording run made or read, the container's identity and the
//! recording machine's own, out of what it writes and prints.

use base64::Engine as _;
use serde_json::Value;

use crate::plugin::recorded::Answer;
use crate::ports::serving::Served;

/// What each scrubbed value is written as.
pub(super) const REDACTED: &str = "<redacted>";

/// The fewest characters a value has to have to be scrubbed, so that an empty or a
/// one-letter value never stands for every place that letter appears.
const SHORTEST: usize = 4;

/// The characters of a container's id its short form keeps, which is also the name it
/// knows itself by unless it is given another.
const SHORT_ID: usize = 12;

/// Every spelling of every value a recording run must not write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Scrub {
    spellings: Vec<String>,
}

impl Scrub {
    /// Every credential in `credentials`, and the identity of the container `served`, in
    /// each encoding an answer could echo it in, in either case.
    pub(super) fn of(credentials: &[String], served: Option<&Served>) -> Self {
        let mut values: Vec<String> = credentials.to_vec();
        if let Some(served) = served {
            values.push(served.container.clone());
            values.extend(served.container.get(..SHORT_ID).map(str::to_owned));
            values.push(served.hostname.clone());
            values.extend(served.addresses.iter().cloned());
            values.extend(served.machine.iter().cloned());
        }
        let spellings: Vec<String> = values
            .iter()
            .filter(|value| value.chars().count() >= SHORTEST)
            .flat_map(|value| encoded(value))
            .flat_map(|spelling| [spelling.to_lowercase(), spelling.to_uppercase(), spelling])
            .filter(|spelling| spelling.chars().count() >= SHORTEST)
            .collect();
        Self::ordered(spellings)
    }

    /// The same, with every spelling of the identity of the container `served` besides.
    pub(super) fn and_served(self, served: &Served) -> Self {
        let mut spellings = self.spellings;
        spellings.extend(Self::of(&[], Some(served)).spellings);
        Self::ordered(spellings)
    }

    /// The same, with every spelling of each of `more` besides.
    pub(super) fn and(self, more: &[String]) -> Self {
        let mut spellings = self.spellings;
        spellings.extend(Self::of(more, None).spellings);
        Self::ordered(spellings)
    }

    /// `spellings` once each, the longest first so that none is left half replaced.
    fn ordered(mut spellings: Vec<String>) -> Self {
        spellings.sort_by(|one, other| other.len().cmp(&one.len()).then(one.cmp(other)));
        spellings.dedup();
        Self { spellings }
    }

    /// `text` with every spelling written as [`REDACTED`].
    pub(super) fn text(&self, text: &str) -> String {
        self.spellings
            .iter()
            .fold(text.to_owned(), |text, spelling| {
                text.replace(spelling.as_str(), REDACTED)
            })
    }

    /// Whether `text` still holds any spelling.
    pub(super) fn holds(&self, text: &str) -> bool {
        self.spellings
            .iter()
            .any(|spelling| text.contains(spelling.as_str()))
    }

    /// `answer` with every header, every key and string of its body and the beginning of
    /// a body that is not JSON scrubbed.
    pub(super) fn answer(&self, answer: Answer) -> Answer {
        Answer {
            status: answer.status,
            headers: answer
                .headers
                .into_iter()
                .map(|(name, value)| (self.text(&name), self.text(&value)))
                .collect(),
            json: answer.json.map(|json| self.value(json)),
            body_starts_with: answer.body_starts_with.map(|body| self.text(&body)),
        }
    }

    /// `value` with every key and string in it scrubbed.
    fn value(&self, value: Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.text(&text)),
            Value::Array(items) => {
                Value::Array(items.into_iter().map(|item| self.value(item)).collect())
            }
            Value::Object(members) => Value::Object(
                members
                    .into_iter()
                    .map(|(key, member)| (self.text(&key), self.value(member)))
                    .collect(),
            ),
            other => other,
        }
    }
}

/// `value` as made, and as a URL, a form, base64 in both alphabets with and without
/// padding, and a JSON string carry it.
fn encoded(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    let json = serde_json::to_string(value).unwrap_or_default();
    vec![
        value.to_owned(),
        crate::endpoint::query_encoded(value),
        form_urlencoded::byte_serialize(bytes).collect(),
        base64::engine::general_purpose::STANDARD.encode(bytes),
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes),
        base64::engine::general_purpose::URL_SAFE.encode(bytes),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
        json.get(1..json.len().saturating_sub(1))
            .unwrap_or_default()
            .to_owned(),
    ]
}

#[cfg(test)]
mod tests;
