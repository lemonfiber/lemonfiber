//! Every refusal asking the keys for something can meet, each in its own words.

use crate::error::codes::key::{
    BAD_NAME, MEMBERS_MAY_NOT_MINT, NAME_TAKEN, NOT_A_PURPOSE, NOT_A_SCOPE, NOT_FOR_YOURSELF,
    NO_SECRET, NO_SUCH_KEY, NO_SUCH_MEMBER, UNASKED, UNREADABLE,
};
use crate::error::{Amiss, Diagnose, Problem, Remedy, Severity};
use crate::PRODUCT;

use super::super::scope::SCOPES;
use super::super::{Purpose, Record, Unkept, LONGEST_NAME};

/// There is nowhere on this machine keys are kept.
pub(super) fn nowhere() -> Problem {
    crate::config::store::Failure::Nowhere.problem()
}

/// The keys or the record of them could not be read or written.
pub(super) fn unkept(why: Unkept) -> Problem {
    match why {
        Unkept::NotWritten(failure) => failure.problem(),
        Unkept::Unreadable(path) => Problem::new(
            UNREADABLE,
            Severity::Error,
            "The record of this machine's keys could not be read",
            format!(
                "It is at {}, and it does not read as keys. Nothing is written over it, \
                 because it may still hold keys somebody depends on.",
                path.display()
            ),
            Remedy::new("Look at the file, and move it aside if it is damaged")
                .with_detail("every key it held is refused until it reads again"),
        ),
    }
}

/// The word cannot name a key.
pub(super) fn bad_name(name: &str) -> Problem {
    Problem::new(
        BAD_NAME,
        Severity::Error,
        format!("\"{name}\" cannot name a key"),
        format!(
            "A key's name travels in an address, an alert and the journal, so it is lower-case \
             letters, digits, dots, dashes and underscores, beginning with a letter or a digit, \
             and at most {LONGEST_NAME} characters."
        ),
        Remedy::new("Name it as the program that holds it, as in home-assistant"),
    )
    .lies_in(Amiss::Asking)
}

/// The scope names none of the three.
pub(super) fn not_a_scope(scope: &str) -> Problem {
    Problem::new(
        NOT_A_SCOPE,
        Severity::Error,
        format!("\"{scope}\" is not a scope a key can have"),
        "A key has exactly one scope, and it decides everything the key admits.",
        Remedy::new(format!("Give it one of {SCOPES}")),
    )
    .lies_in(Amiss::Asking)
}

/// The purpose names none of the three.
pub(super) fn not_a_purpose(purpose: &str) -> Problem {
    let purposes: Vec<&str> = Purpose::EVERY.into_iter().map(Purpose::written).collect();
    Problem::new(
        NOT_A_PURPOSE,
        Severity::Error,
        format!("\"{purpose}\" is not a purpose a key can carry"),
        "A key says what it is for so the listing can tell keys apart.",
        Remedy::new(format!("Give it one of {}", purposes.join(", "))),
    )
    .lies_in(Amiss::Asking)
}

/// Another key holds the name.
pub(super) fn name_taken(held: &Record) -> Problem {
    let standing = held.revoked.as_deref().map_or_else(
        || "and it is active".to_owned(),
        |when| format!("revoked {when}"),
    );
    Problem::new(
        NAME_TAKEN,
        Severity::Error,
        format!("A key named {} already exists", held.name),
        format!(
            "It was minted {} with the scope {}, {standing}. A name identifies one key, so an \
             alert or a journal entry naming it means the same key for good.",
            held.minted,
            held.scope.written()
        ),
        Remedy::new("Mint it under another name"),
    )
    .lies_in(Amiss::Asking)
}

/// Another member's key holds the name, which is not this member's to be told about.
pub(super) fn name_held(name: &str) -> Problem {
    Problem::new(
        NAME_TAKEN,
        Severity::Error,
        format!("A key named {name} already exists"),
        "A name identifies one key on this machine, so an alert or a journal entry naming it \
         means the same key for good.",
        Remedy::new("Mint it under another name"),
    )
    .lies_in(Amiss::Asking)
}

/// The operator has not allowed household members to mint keys.
pub(super) fn members_may_not_mint() -> Problem {
    Problem::new(
        MEMBERS_MAY_NOT_MINT,
        Severity::Warning,
        "Whoever looks after this machine has not allowed household members to mint keys",
        "A member's key carries that member's requests and viewing to whatever program holds \
         it, so minting one is something the operator allows first. Nothing was minted.",
        Remedy::new("Ask whoever looks after this machine to allow it").with_detail(format!(
            "{PRODUCT} config set {} on",
            crate::config::MEMBER_KEYS_KEY
        )),
    )
    .lies_in(Amiss::Held)
}

/// No account in the household goes by the name a member's scope gave.
pub(super) fn no_such_member(account: &str) -> Problem {
    Problem::new(
        NO_SUCH_MEMBER,
        Severity::Error,
        format!("Nobody in the household goes by \"{account}\""),
        "A member's key admits exactly what that member's own account does, so it is minted \
         for an account the household holds.",
        Remedy::new("Name them as they appear in the household")
            .with_detail(format!("{PRODUCT} household")),
    )
    .lies_in(Amiss::Naming)
}

/// The household could not be asked about the account.
pub(super) fn unasked() -> Problem {
    Problem::new(
        UNASKED,
        Severity::Error,
        "The household could not be asked about that account",
        "A member's key is minted for an account the media server holds, and the media \
         server did not answer, so nothing was minted.",
        Remedy::new("Try again once the media server is running"),
    )
}

/// A member asked for a key, or a revoke, that is not theirs alone.
pub(super) fn not_for_yourself() -> Problem {
    Problem::new(
        NOT_FOR_YOURSELF,
        Severity::Warning,
        "A household member mints and revokes only keys of their own",
        "A member's key admits what their own account does and nothing more, so the only key \
         a member may mint or revoke is one scoped to themselves.",
        Remedy::new("Ask for member:<your own account>, or ask whoever looks after this machine"),
    )
    .lies_in(Amiss::Asking)
}

/// No active key holds the name.
pub(super) fn no_such_key(name: &str, held: Option<&Record>) -> Problem {
    let meaning = match held.and_then(|record| record.revoked.as_deref()) {
        Some(when) => format!("The key named {name} was already revoked {when}."),
        None => format!("No key is named {name}."),
    };
    Problem::new(
        NO_SUCH_KEY,
        Severity::Error,
        format!("There is no active key named {name}"),
        meaning,
        Remedy::new("List the keys and revoke one by the name it shows")
            .with_detail(format!("{PRODUCT} key list")),
    )
    .lies_in(Amiss::Naming)
}

/// This machine would not supply the bytes a secret is made of.
pub(super) fn no_secret() -> Problem {
    Problem::new(
        NO_SECRET,
        Severity::Error,
        format!("{PRODUCT} could not mint a secret for that key"),
        "A key's secret is unpredictable bytes this machine supplies, and it would not supply \
         them, so nothing was minted.",
        Remedy::new(
            "Try again, and if it happens twice the operating system's own random source is at \
             fault",
        ),
    )
}
