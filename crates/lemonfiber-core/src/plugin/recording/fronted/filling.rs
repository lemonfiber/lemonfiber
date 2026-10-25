//! A case's ask with every placeholder in it filled: from a credential minted for it, a
//! field of the adapter's `upstream.json`, or what an earlier case answered.

use std::collections::BTreeMap;
use std::path::Path;

use lemonfiber_contract::adapter::UPSTREAM_FILE;
use lemonfiber_contract::conformance::Placeholder;
use serde_json::Value;

use crate::plugin::program::Throwaway;
use crate::ports::http::{Response, PROBLEM};
use crate::ports::random::Random;

/// A case's ask with every placeholder in it filled.
pub(super) struct Filled {
    /// What is posted.
    pub(super) value: Value,
    /// Each field of `upstream.json` a credential minted for the case is written to once
    /// the adapter answers it with a success, and that credential.
    pub(super) minted: Vec<(String, String)>,
}

/// What a placeholder is filled from.
pub(super) struct Filling<'a> {
    /// Where a credential minted for a case comes from.
    pub(super) random: &'a dyn Random,
    /// The adapter's starting files, which hold its `upstream.json`.
    pub(super) held: &'a Throwaway,
    /// What each case asked so far answered, by `<contract>/<case>`.
    pub(super) answers: BTreeMap<String, Value>,
    /// Every credential the run has made so far.
    pub(super) made: Vec<String>,
}

impl Filling<'_> {
    /// `asks` with every placeholder in it filled, or why one could not be.
    pub(super) fn filled(&mut self, asks: &Value) -> Result<Filled, String> {
        let mut minted = Vec::new();
        let value = self.value(asks, &mut minted)?;
        Ok(Filled { value, minted })
    }

    fn value(&mut self, asks: &Value, minted: &mut Vec<(String, String)>) -> Result<Value, String> {
        Ok(match asks {
            Value::String(text) => match Placeholder::read(text) {
                Some(placeholder) => Value::String(
                    self.placeholder(&placeholder, minted)
                        .map_err(|why| format!("{text} cannot be filled: {why}"))?,
                ),
                None => asks.clone(),
            },
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.value(item, minted))
                    .collect::<Result<_, _>>()?,
            ),
            Value::Object(members) => Value::Object(
                members
                    .iter()
                    .map(|(name, member)| Ok((name.clone(), self.value(member, minted)?)))
                    .collect::<Result<_, String>>()?,
            ),
            other => other.clone(),
        })
    }

    /// What `placeholder` is filled with, kept among what the run made where it is a
    /// credential minted for the case or read from an answer.
    fn placeholder(
        &mut self,
        placeholder: &Placeholder,
        minted: &mut Vec<(String, String)>,
    ) -> Result<String, String> {
        match placeholder {
            Placeholder::Minted => self.minted(),
            Placeholder::MintedUpstream(field) => {
                let fresh = self.minted()?;
                minted.push((field.clone(), fresh.clone()));
                Ok(fresh)
            }
            Placeholder::Upstream(field) => self
                .upstream()?
                .get(field)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("{UPSTREAM_FILE} holds no {field}")),
            Placeholder::Answered { case, pointer } => {
                let answered = self
                    .answers
                    .get(case)
                    .ok_or_else(|| format!("{case} was not asked before it"))?
                    .pointer(pointer)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{case} answered no string at {pointer:?}"))?;
                self.made.push(answered.clone());
                Ok(answered)
            }
        }
    }

    /// Why a case posting `asks` is not offered by an adapter speaking `speaks`: a
    /// placeholder in it reads a case of a contract the adapter does not speak, or one it
    /// answered `unsupported`. Nothing where every placeholder reads an answer it may.
    pub(super) fn unoffered(&self, asks: &Value, speaks: &[String]) -> Option<String> {
        match asks {
            Value::String(text) => match Placeholder::read(text) {
                Some(Placeholder::Answered { case, .. }) => {
                    let contract = case.split_once('/').map_or(case.as_str(), |(of, _)| of);
                    if !speaks.iter().any(|spoken| spoken == contract) {
                        Some(format!(
                            "it speaks no {contract}, which {case} is a case of"
                        ))
                    } else if self.answers.get(&case).is_some_and(refused_unsupported) {
                        Some(format!("{case} was answered unsupported"))
                    } else {
                        None
                    }
                }
                _ => None,
            },
            Value::Array(items) => items.iter().find_map(|item| self.unoffered(item, speaks)),
            Value::Object(members) => members
                .values()
                .find_map(|member| self.unoffered(member, speaks)),
            _ => None,
        }
    }

    /// A credential minted for one case, kept among what the run made.
    fn minted(&mut self) -> Result<String, String> {
        let fresh = crate::secret::generate(self.random)
            .ok_or_else(|| crate::secret::NO_RANDOMNESS_FOR_PASSWORD.to_owned())?;
        self.made.push(fresh.clone());
        Ok(fresh)
    }

    /// What the adapter's `upstream.json` holds now.
    fn upstream(&self) -> Result<serde_json::Map<String, Value>, String> {
        held_upstream(self.held)?.ok_or_else(|| format!("there is no {UPSTREAM_FILE} yet"))
    }

    /// Each minted credential written to `upstream.json` at its field.
    pub(super) fn keep_minted(&self, minted: &[(String, String)]) -> Result<(), String> {
        if minted.is_empty() {
            return Ok(());
        }
        handed_over(self.held, minted)
    }
}

/// Whether `answer` is the refusal an adapter answers an operation it cannot perform with.
fn refused_unsupported(answer: &Value) -> bool {
    serde_json::from_value::<lemonfiber_contract::wire::Refusal>(answer.clone())
        .is_ok_and(|refusal| refusal.kind == lemonfiber_contract::wire::Kind::Unsupported)
}

/// What a case that is not offered is recorded as answering: `unsupported`, saying why.
pub(super) fn unsupported(why: String) -> Response {
    let kind = lemonfiber_contract::wire::Kind::Unsupported;
    Response {
        status: kind.status(),
        headers: vec![("Content-Type".to_owned(), PROBLEM.to_owned())],
        body: serde_json::to_string(&lemonfiber_contract::wire::Refusal { kind, detail: why })
            .unwrap_or_default(),
    }
}

/// What the adapter's `upstream.json` in `held` holds, where it holds one.
fn held_upstream(held: &Throwaway) -> Result<Option<serde_json::Map<String, Value>>, String> {
    match held.wrote(Path::new(UPSTREAM_FILE))? {
        None => Ok(None),
        Some(bytes) => match serde_json::from_slice(&bytes) {
            Ok(Value::Object(fields)) => Ok(Some(fields)),
            _ => Err(format!("{UPSTREAM_FILE} is not an object")),
        },
    }
}

/// `fields` written into the adapter's `upstream.json` in `held`, beside whatever else it
/// already holds.
///
/// # Errors
///
/// Where what it holds is not an object, or the file could not be written.
pub(super) fn handed_over(held: &Throwaway, fields: &[(String, String)]) -> Result<(), String> {
    let mut kept = held_upstream(held)?.unwrap_or_default();
    for (field, value) in fields {
        kept.insert(field.clone(), Value::String(value.clone()));
    }
    held.hold(UPSTREAM_FILE, &Value::Object(kept).to_string())
}

#[cfg(test)]
mod tests;
