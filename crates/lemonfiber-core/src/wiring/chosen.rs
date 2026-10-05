//! Which service the operator chose to fill each capability, and why.
//!
//! Two settings, because `LEMONFIBER_FILLS` is a published setting of
//! `capability=service` pairs and keeps its shape. The reasons sit beside it in a setting
//! of their own, written in the same journalled change, so putting a choice back puts its
//! reason back with it.
//!
//! A reason is the operator's own words, so it is carried percent-encoded: nothing an
//! operator types can end a pair early or start a second one.

use std::collections::BTreeMap;

/// The most characters a reason may hold.
///
/// A reason is read beside the choice it explains, on a line of a listing and on a
/// phone. Longer than that it has stopped being a reason and become a note, and a
/// note is not something this setting is the place for.
pub const REASON_MOST: usize = 280;

/// Which service the operator chose to fill each capability.
///
/// Read from one setting and written back to it. A capability with no entry is
/// settled by the stack, which is where a default belongs: the operator's record
/// holds what they decided, not a copy of what they left alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chosen {
    /// The service chosen for each capability.
    fillers: BTreeMap<String, String>,
    /// What the operator said about a choice, where they said anything.
    reasons: BTreeMap<String, String>,
}

impl Chosen {
    /// The choices a recorded setting holds, with no reasons.
    ///
    /// Anything unreadable in it is dropped rather than failing the read. This value
    /// reaches a listing and a seed as much as it reaches the command that writes it,
    /// and a single malformed pair that stopped a stack from being described would
    /// cost more than the pair is worth.
    #[must_use]
    pub fn read(setting: Option<&str>) -> Self {
        Self {
            fillers: setting
                .unwrap_or_default()
                .split(',')
                .filter_map(|pair| pair.split_once('='))
                .map(|(capability, service)| {
                    (capability.trim().to_owned(), service.trim().to_owned())
                })
                .filter(|(capability, service)| !capability.is_empty() && !service.is_empty())
                .collect(),
            reasons: BTreeMap::new(),
        }
    }

    /// The same choices, with the reasons a recorded setting holds for them.
    ///
    /// A reason for a capability nobody chose a filler for is dropped: it explains a
    /// choice that is not there, and reading it back would credit the stack's own
    /// settlement with the operator's words.
    #[must_use]
    pub fn because(mut self, setting: Option<&str>) -> Self {
        self.reasons = form_urlencoded::parse(setting.unwrap_or_default().as_bytes())
            .map(|(capability, reason)| (capability.into_owned(), reason.into_owned()))
            .filter(|(capability, reason)| {
                !reason.is_empty() && self.fillers.contains_key(capability)
            })
            .collect();
        self
    }

    /// Every choice recorded, as the capability and the service chosen for it.
    pub fn choices(&self) -> impl Iterator<Item = (&str, &str)> {
        self.fillers
            .iter()
            .map(|(capability, service)| (capability.as_str(), service.as_str()))
    }

    /// Who the operator chose to fill this capability, where they chose.
    #[must_use]
    pub fn filler(&self, capability: &str) -> Option<&str> {
        self.fillers.get(capability).map(String::as_str)
    }

    /// What the operator said about their choice for this capability, where they said
    /// anything.
    #[must_use]
    pub fn why(&self, capability: &str) -> Option<&str> {
        self.reasons.get(capability).map(String::as_str)
    }

    /// The setting this becomes with one more choice recorded in it.
    #[must_use]
    pub fn with(&self, capability: &str, service: &str) -> String {
        let mut held = self.fillers.clone();
        held.insert(capability.to_owned(), service.to_owned());
        held.iter()
            .map(|(capability, service)| format!("{capability}={service}"))
            .collect::<Vec<String>>()
            .join(",")
    }

    /// The reasons setting this becomes with one choice made, or nothing where no
    /// reason is left.
    ///
    /// A choice made with no reason takes away whatever reason the last choice for
    /// that capability carried: the reason was for that choice, and reading it back
    /// beside this one would put words in the operator's mouth.
    #[must_use]
    pub fn reasons_with(&self, capability: &str, reason: Option<&str>) -> Option<String> {
        let mut held = self.reasons.clone();
        match reason.filter(|said| !said.is_empty()) {
            Some(said) => held.insert(capability.to_owned(), said.to_owned()),
            None => held.remove(capability),
        };
        encoded(&held)
    }

    /// What the setting says as it stands, or nothing where it says nothing.
    #[must_use]
    pub fn setting(&self) -> Option<String> {
        (!self.fillers.is_empty()).then(|| {
            self.fillers
                .iter()
                .map(|(capability, service)| format!("{capability}={service}"))
                .collect::<Vec<String>>()
                .join(",")
        })
    }

    /// What the reasons setting says as it stands, or nothing where it says nothing.
    #[must_use]
    pub fn reasons(&self) -> Option<String> {
        encoded(&self.reasons)
    }
}

/// Reasons as the setting holds them, or nothing where there are none.
fn encoded(reasons: &BTreeMap<String, String>) -> Option<String> {
    (!reasons.is_empty()).then(|| {
        form_urlencoded::Serializer::new(String::new())
            .extend_pairs(reasons)
            .finish()
    })
}

#[cfg(test)]
mod tests;
