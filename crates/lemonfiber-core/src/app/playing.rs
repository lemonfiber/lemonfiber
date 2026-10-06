//! What the media server is playing now, read for everybody or for one member.
//!
//! **One member's is theirs alone.** A member asking reaches this command with their
//! own id in it, put there by whoever admitted them, so there is no path on which
//! another member's session is read and then left out. An operator may name a member
//! to see what that person is watching, or name nobody to see the whole house.

use super::targets::jellyfin_reader;
use super::{Ctx, Outcome};

use crate::error::{Diagnose, Problem};
use crate::model::PlayingReport;
use crate::ports::service::{Failure, Household as _, Member, Playback};

/// Read what is playing now, for one member where `member` names them, or for
/// everybody.
///
/// A member is matched against the id the server files them under first and against
/// their name second, as the shelf matches them: a member's own session carries the id
/// and an operator types a name, and one resolution serves both.
pub(crate) async fn playing(
    ctx: &Ctx,
    member: Option<&str>,
) -> Result<PlayingReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let Some(server) = jellyfin_reader(ctx, &manifest) else {
        return Ok(unread(
            member,
            "there is no media server to ask what is playing, or no recorded password to \
             sign in with",
        ));
    };

    let Some(member) = member else {
        return Ok(answered(server.playing(None).await, String::new()));
    };

    let Ok(accounts) = server.household().await else {
        return Ok(unread(
            Some(member),
            "the media server would not say who holds an account, so whose sessions these \
             are could not be established",
        ));
    };

    let Some(account) = account(&accounts, member) else {
        return Ok(unread(
            Some(member),
            "nobody in this household is known by that, so there is nobody to say is \
             watching — which is not the same as somebody watching nothing",
        ));
    };

    Ok(answered(
        server.playing(Some(&account.id)).await,
        account.name.clone(),
    ))
}

/// The reading, as the outcome the command answers with.
pub(crate) async fn asked(ctx: &Ctx, member: Option<&str>) -> Result<Outcome, Box<Problem>> {
    playing(ctx, member).await.map(Outcome::Playing)
}

/// What the server answered, said as a report about whoever it was asked for.
fn answered(answered: Result<Vec<Playback>, Failure>, member: String) -> PlayingReport {
    let Ok(sessions) = answered else {
        return PlayingReport {
            member,
            findings: vec![
                "the media server would not say what it is playing, so this is reported as \
                 unread rather than as nobody watching"
                    .to_owned(),
            ],
            ..PlayingReport::default()
        };
    };
    PlayingReport {
        member,
        sessions,
        available: true,
        findings: Vec::new(),
    }
}

/// The account a name or an id means, or nobody.
fn account<'a>(accounts: &'a [Member], named: &str) -> Option<&'a Member> {
    accounts.iter().find(|held| held.id == named).or_else(|| {
        accounts
            .iter()
            .find(|held| held.name.eq_ignore_ascii_case(named))
    })
}

/// A reading that could not be made, said as that rather than as nobody watching.
fn unread(member: Option<&str>, why: &str) -> PlayingReport {
    PlayingReport {
        member: member.unwrap_or_default().to_owned(),
        findings: vec![why.to_owned()],
        ..PlayingReport::default()
    }
}

#[cfg(test)]
mod tests;
