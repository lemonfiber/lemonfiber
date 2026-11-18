//! What proves who asked for a mint over the web API: the minter's password, or a
//! signature over a challenge by one of the operator's device keys.

use std::net::IpAddr;

use crate::app::admitting::{held, proves_the_member, proves_the_operator, Door, PASSWORD};
use crate::app::Ctx;
use crate::device_keys::proof::Mint;
use crate::device_keys::run::{challenge_not_standing, misproved, signature_wrong, standing, used};
use crate::error::Problem;
use crate::keys::Minter;

/// How a mint proves who asked.
#[derive(Clone, PartialEq, Eq)]
pub enum Proof {
    /// The minter's password, which the asker gives when asked.
    Password,
    /// A signature over a challenge by one of the operator's device keys.
    Device {
        /// The device key's name.
        name: String,
        /// The challenge it signed.
        challenge: String,
        /// The signature it made.
        signature: String,
        /// The session the challenge was issued to.
        session: String,
    },
}

impl std::fmt::Debug for Proof {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Password => out.write_str("Password"),
            Self::Device { name, .. } => out
                .debug_struct("Device")
                .field("name", name)
                .finish_non_exhaustive(),
        }
    }
}

/// A proof, and the address the request carrying it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proving {
    /// What proves who asked.
    pub proof: Proof,
    /// Where the request came from, where it came over a socket.
    pub from: Option<IpAddr>,
}

/// The device key that proved `mint` for `by`, nothing where the password did, or the
/// refusal where neither proves them.
pub(super) async fn proved(
    ctx: &Ctx,
    by: &Minter,
    mint: &Mint<'_>,
    proving: Proving,
) -> Result<Option<String>, Box<Problem>> {
    let now = ctx.seams.clock.now();
    match (proving.proof, by) {
        (Proof::Device { .. }, Minter::Member { .. }) => Err(not_yours()),
        (
            Proof::Device {
                name,
                challenge,
                signature,
                session,
            },
            Minter::Operator,
        ) => {
            let record = standing(ctx, &name).await?;
            let attempts = &ctx.admitting.attempts;
            let ticket = attempts
                .taken(proving.from, None, now)
                .await
                .map_err(held)?;
            let stood = ctx
                .admitting
                .challenges
                .spent(&challenge, &session, &name, now)
                .await;
            if !stood {
                attempts.forgiven(&ticket).await;
                return Err(Box::new(challenge_not_standing()));
            }
            let signed = record
                .key()
                .is_some_and(|key| mint.proved(&challenge, &signature, &key));
            if !signed {
                misproved(ctx, &record).await;
                return Err(Box::new(signature_wrong()));
            }
            attempts.right(&ticket, Door::Operator, now).await;
            used(ctx, &name).await?;
            Ok(Some(name))
        }
        (Proof::Password, Minter::Member { .. }) if !super::members_may_mint(ctx) => Ok(None),
        (Proof::Password, Minter::Operator) => {
            let password = ctx.asking.secret(PASSWORD);
            answered(proves_the_operator(ctx, &password, proving.from, now).await)
        }
        (Proof::Password, Minter::Member { id }) => {
            let password = ctx.asking.secret(PASSWORD);
            answered(proves_the_member(ctx, id, &password, proving.from, now).await)
        }
    }
}

fn answered(proved: Result<bool, std::time::Duration>) -> Result<Option<String>, Box<Problem>> {
    match proved {
        Ok(true) => Ok(None),
        Ok(false) => Err(crate::app::admitting::not_the_password()),
        Err(left) => Err(held(left)),
    }
}

fn not_yours() -> Box<Problem> {
    let refused = crate::app::admitting::not_yours();
    Box::new(Problem {
        summary: "Device keys are the operator's, and only theirs.".to_owned(),
        ..*refused
    })
}
