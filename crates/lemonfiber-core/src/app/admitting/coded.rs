//! A member signed in by a code they approve on a device already signed in.

use std::net::IpAddr;

use serde::Serialize;

use super::household::{named, opened, vouched};
use super::refused::{no_session, not_waiting, signs_in_by_password, too_many_codes};
use super::sessions::Opened;
use super::signing_in::device;
use super::Answer;
use super::Caller;
use crate::app::{Ctx, Outcome};
use crate::error::Problem;
use crate::instant::Instant;
use crate::ports::service::Failure;
use crate::sign_in_codes::{self, Asker, NotKept};

/// A code to sign in by, or the sign-in by a code still waiting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SignInCode {
    /// The code the member enters where they approve it, where one was just asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Where the member approves the code, where one was just asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approve_at: Option<String>,
    /// When the code lapses.
    pub lapses_at: Instant,
    /// The handle that finishes signing in by the code.
    pub waiting: String,
}

/// A code asked for, or a sign-in by one finished.
#[derive(Clone, PartialEq, Eq)]
pub struct Coded {
    /// The session the request carried, where it carried one.
    pub session: Option<String>,
    /// The address it came from, where it came over a socket.
    pub from: Option<IpAddr>,
    /// The handle of the code to finish with, or nothing to ask for one.
    pub waiting: Option<String>,
}

impl std::fmt::Debug for Coded {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Coded")
            .field("from", &self.from)
            .field("finishing", &self.waiting.is_some())
            .finish_non_exhaustive()
    }
}

/// What asking for a code, or finishing with one, comes to.
pub(super) async fn coded(ctx: &Ctx, coded: Coded) -> Result<Outcome, Box<Problem>> {
    let asker = Asker::new(coded.session.as_deref(), coded.from);
    match coded.waiting {
        None => asked(ctx, asker)
            .await
            .map(|code| Outcome::Admitting(Answer::SignInCode(code))),
        Some(handle) => finished(ctx, &asker, handle).await,
    }
}

/// A code drawn for `asker`.
async fn asked(ctx: &Ctx, asker: Asker) -> Result<SignInCode, Box<Problem>> {
    let limits = sign_in_codes::limits(ctx);
    let codes = &ctx.admitting.codes;
    codes
        .asking(&asker, &limits, ctx.seams.clock.now())
        .map_err(too_many_codes)?;
    let household = opened(ctx).await.ok_or_else(Caller::unconfirmed)?;
    let random = ctx.seams.random.as_ref();
    let device = device(random).ok_or_else(Caller::unconfirmed)?;
    let issued = match household.sign_in_by_code(&device).await {
        Ok(issued) => issued,
        Err(Failure::Unsupported { .. }) => return Err(signs_in_by_password()),
        Err(_) => return Err(Caller::unconfirmed()),
    };
    let waiting = codes
        .kept(
            random,
            (issued.id, asker, issued.lapses_at.time()),
            &limits,
            ctx.seams.clock.now(),
        )
        .map_err(unkept)?;
    Ok(SignInCode {
        code: Some(issued.code),
        approve_at: Some(issued.approve_at),
        lapses_at: issued.lapses_at,
        waiting,
    })
}

/// The refusal a code the table would not keep is answered with.
fn unkept(not_kept: NotKept) -> Box<Problem> {
    match not_kept {
        NotKept::Waiting(left) => too_many_codes(left),
        NotKept::Undrawn => Caller::unconfirmed(),
    }
}

/// The code `handle` names, still waiting, or the member's session once approved.
async fn finished(ctx: &Ctx, asker: &Asker, handle: String) -> Result<Outcome, Box<Problem>> {
    let limits = sign_in_codes::limits(ctx);
    let admitting = &ctx.admitting;
    let finishing = admitting
        .codes
        .claimed(&handle, asker, &limits, ctx.seams.clock.now())
        .ok_or_else(not_waiting)?;
    let household = opened(ctx).await.ok_or_else(Caller::unconfirmed)?;
    let signed = match household.signed_by_code(&finishing.id).await {
        Ok(Some(signed)) => signed,
        Ok(None) => {
            return Ok(Outcome::Admitting(Answer::SignInCode(SignInCode {
                code: None,
                approve_at: None,
                lapses_at: Instant::of(finishing.lapses),
                waiting: handle,
            })))
        }
        Err(_) => return Err(Caller::unconfirmed()),
    };
    finishing.spent();
    if !vouched(ctx, &signed.id).await {
        return Err(not_waiting());
    }
    let name = named(ctx, &signed.id)
        .await
        .ok_or_else(Caller::unconfirmed)?;
    admitting
        .sessions
        .opened(
            ctx.seams.random.as_ref(),
            ctx.seams.clock.now(),
            Opened::Member(signed),
        )
        .await
        .map(|opened| Outcome::Admitting(Answer::Admission(opened.called(Some(name)))))
        .ok_or_else(no_session)
}

#[cfg(test)]
mod tests;
