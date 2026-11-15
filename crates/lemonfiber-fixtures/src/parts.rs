//! The parts the bundle pins, by id.

/// The part ids that are also the core's own words, each with what the core means by it.
pub const OWNED: [(&str, &str); 3] = [
    (
        "door",
        "the front door a household is handed, which the core decides",
    ),
    ("decline", "invite.decline, a capability of the vocabulary"),
    (
        "request-gate",
        "request.gate, a capability of the vocabulary",
    ),
];

/// Whether `id` is a part id that is also one of the core's own words.
#[must_use]
pub fn owned(id: &str) -> bool {
    OWNED.iter().any(|(owned, _)| *owned == id)
}

/// Every plugin the bundle pins, offered or not, by id, apart from those in [`OWNED`].
pub fn pinned() -> impl Iterator<Item = &'static str> {
    lemonfiber_bundled::PINNED
        .iter()
        .map(|pinned| pinned.first_party.plugin)
        .filter(|id| !owned(id))
}
