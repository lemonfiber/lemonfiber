//! What an adapter is asked: every contract it speaks in the order its cases are asked,
//! and what names each case and where its recording sits.

use lemonfiber_contract::conformance::{cases, filed, Placeholder};
use serde_json::Value;

use crate::plugin::conforming::RECORDINGS;

/// What names the case `case` of the contract `contract`.
pub(super) fn named(contract: &str, case: &str) -> String {
    format!("conformance {contract}/{case}")
}

/// Where the recording of the case `case` of the contract `contract` sits.
pub(super) fn fixture(contract: &str, case: &str) -> String {
    format!("{RECORDINGS}/{contract}/{case}.json")
}

/// What names each case of every contract `adapter` speaks that an upstream filing `files`
/// is asked, and where its recording sits.
pub(in crate::plugin::recording) fn fixtures(
    adapter: &lemonfiber_plugin::Service,
    files: &[String],
) -> Vec<(String, String)> {
    contracts(adapter)
        .iter()
        .flat_map(|(spoken, capability)| {
            filed(capability, files)
                .into_iter()
                .map(|case| (named(spoken, &case.case), fixture(spoken, &case.case)))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Every contract `adapter` speaks, in the order it lists them, except that each comes
/// after every contract whose answers its cases' placeholders read.
pub(super) fn contracts(
    adapter: &lemonfiber_plugin::Service,
) -> Vec<(String, lemonfiber_contract::Capability)> {
    let published = lemonfiber_contract::capabilities::all();
    let mut left: Vec<(String, lemonfiber_contract::Capability)> = adapter
        .speaks
        .iter()
        .filter_map(|spoken| {
            published
                .iter()
                .find(|one| lemonfiber_contract::spoken(one.name, one.major) == *spoken)
                .map(|one| (spoken.clone(), one.clone()))
        })
        .collect();
    let mut ordered = Vec::with_capacity(left.len());
    while !left.is_empty() {
        let next = left
            .iter()
            .position(|(spoken, capability)| {
                reads(capability)
                    .iter()
                    .all(|read| read == spoken || !left.iter().any(|(waiting, _)| waiting == read))
            })
            .unwrap_or_default();
        ordered.push(left.remove(next));
    }
    ordered
}

/// Every contract, as `<capability>@<major>`, whose answers a case of `capability` reads.
fn reads(capability: &lemonfiber_contract::Capability) -> Vec<String> {
    let mut read = Vec::new();
    for case in cases(capability) {
        answered_in(&case.asks, &mut read);
    }
    read
}

/// The contract of each earlier answer a placeholder in `asks` reads, into `read`.
fn answered_in(asks: &Value, read: &mut Vec<String>) {
    match asks {
        Value::String(text) => {
            if let Some(Placeholder::Answered { case, .. }) = Placeholder::read(text) {
                if let Some((contract, _)) = case.split_once('/') {
                    read.push(contract.to_owned());
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| answered_in(item, read)),
        Value::Object(members) => members
            .values()
            .for_each(|member| answered_in(member, read)),
        _ => {}
    }
}
