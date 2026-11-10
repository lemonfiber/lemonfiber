//! Adding, changing, testing and removing the back-ends the stack delivers to, and listing
//! them.

use url::Url;

use super::backends::{self, Backend, Held, Kept, ATTEMPTS, DELAYS};
use super::reading;
use crate::app::{Ctx, Outcome};
use crate::config::store::Failure;
use crate::error::codes::notify::{
    BACKEND_INCOMPLETE, BACKEND_NAME_TAKEN, HOSTED_UNACKNOWLEDGED, NOT_A_BACKEND_ADDRESS,
    NOT_A_BACKEND_NAME, NOT_A_RETRY_SCHEDULE, NO_SUCH_BACKEND,
};
use crate::error::{Code, Diagnose, Problem, Remedy, State};
use crate::keys::names_a_key;
use crate::model::{BackendKind, BackendProof, ChosenBy, NotifyReport};

/// The most attempts a delivery is given.
const MOST_ATTEMPTS: u32 = 10;

/// The hosts known to be hosted services rather than one the household runs.
const HOSTED: [&str; 1] = ["ntfy.sh"];

/// Why a back-end whose secret changed delivers nothing until it is proven again.
const CHANGED: &str = "what it is reached with changed, so it is proven again before it delivers";

/// What was asked of the back-ends.
#[derive(Clone, PartialEq, Eq)]
pub enum Asked {
    /// List them.
    List,
    /// Add one.
    Add(Box<Adding>),
    /// Change one.
    Set(Box<Setting>),
    /// Send one a test and see whether it arrives.
    Test {
        /// The back-end, by its id.
        id: String,
    },
    /// Remove one.
    Remove {
        /// The back-end, by its id.
        id: String,
    },
}

/// A back-end to add.
#[derive(Clone, PartialEq, Eq)]
pub struct Adding {
    /// What to call it.
    pub id: String,
    /// What it speaks.
    pub kind: BackendKind,
    /// The server's address.
    pub url: String,
    /// The ntfy topic it posts to.
    pub topic: Option<String>,
    /// What it is posted with.
    pub token: Option<String>,
    /// What its history is read with, where that is another.
    pub reader: Option<String>,
    /// Whether the operator accepts that it is a hosted service.
    pub hosted: bool,
    /// How many attempts a delivery is given.
    pub attempts: Option<u32>,
    /// The minutes waited before each attempt after the first.
    pub delays: Option<Vec<u64>>,
}

/// A change to a back-end.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Setting {
    /// The back-end, by its id.
    pub id: String,
    /// What it is posted with from now on.
    pub token: Option<String>,
    /// What its history is read with from now on.
    pub reader: Option<String>,
    /// How many attempts a delivery is given from now on.
    pub attempts: Option<u32>,
    /// The minutes waited before each attempt after the first, from now on.
    pub delays: Option<Vec<u64>>,
    /// Whether it is offered to the household, and under what name.
    pub members: Option<Offering>,
}

/// Whether a back-end is offered to the household.
#[derive(Clone, PartialEq, Eq)]
pub enum Offering {
    /// Offered, under the name the household reads.
    Offer(String),
    /// Withdrawn from every member who chose it.
    Withdraw,
}

/// What was asked, its secrets withheld.
impl std::fmt::Debug for Asked {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::List => out.write_str("List"),
            Self::Add(adding) => out
                .debug_struct("Add")
                .field("id", &adding.id)
                .field("kind", &adding.kind)
                .finish_non_exhaustive(),
            Self::Set(setting) => out
                .debug_struct("Set")
                .field("id", &setting.id)
                .finish_non_exhaustive(),
            Self::Test { id } => out.debug_struct("Test").field("id", id).finish(),
            Self::Remove { id } => out.debug_struct("Remove").field("id", id).finish(),
        }
    }
}

/// Carry out what was asked of the back-ends.
///
/// # Errors
///
/// The [`Problem`] whichever was asked refuses with.
pub async fn asked(ctx: &Ctx, asked: Asked) -> Result<Outcome, Box<Problem>> {
    match asked {
        Asked::List => reading::report(ctx),
        Asked::Add(adding) => add(ctx, &adding),
        Asked::Set(setting) => set(ctx, &setting),
        Asked::Test { id } => super::proving::test(ctx, &id).await,
        Asked::Remove { id } => remove(ctx, &id),
    }
    .map(Outcome::Notify)
}

/// Add a back-end, once its name, its address and what its kind needs are all there.
fn add(ctx: &Ctx, adding: &Adding) -> Result<NotifyReport, Box<Problem>> {
    let url = checked(adding)?;
    let (attempts, delays) = scheduled(adding.attempts, adding.delays.clone())?;
    let record = Backend {
        id: adding.id.clone(),
        kind: adding.kind,
        url,
        topic: adding.topic.clone(),
        chosen_by: ChosenBy::Operator,
        hosted: adding.hosted,
        members: None,
        receivers: Vec::new(),
        attempts,
        delays,
        proof: BackendProof::Unproven,
        testing: None,
        last: None,
        failures: Vec::new(),
        retry: None,
        owed: Vec::new(),
        added: super::now(ctx),
    };
    let path = backends::at(ctx).ok_or_else(nowhere)?;
    let mut refused = None;
    if ctx.dry_run {
        let kept = backends::kept(&path).map_err(|why| read_failure(&why))?;
        refused = taken(&kept, &record.id);
    } else {
        backends::changed(&path, &mut |kept: &mut Kept| {
            refused = taken(kept, &record.id);
            if refused.is_none() {
                kept.backends.push(record.clone());
            }
            Ok(())
        })
        .map_err(|why| read_failure(&why))?;
        if refused.is_none() {
            let secrets = backends::secrets_at(ctx).ok_or_else(nowhere)?;
            let held = Held {
                token: adding.token.clone(),
                reader: adding.reader.clone(),
            };
            backends::hold(&secrets, &record.id, Some(&held)).map_err(|why| read_failure(&why))?;
        }
    }
    refused.map_or_else(|| reading::report(ctx), Err)
}

/// Change a back-end.
fn set(ctx: &Ctx, setting: &Setting) -> Result<NotifyReport, Box<Problem>> {
    given(
        &setting.id,
        setting.token.as_deref(),
        setting.reader.as_deref(),
    )?;
    let path = backends::at(ctx).ok_or_else(nowhere)?;
    let kept = backends::kept(&path).map_err(|why| read_failure(&why))?;
    let current = kept
        .backends
        .iter()
        .find(|one| one.id == setting.id)
        .ok_or_else(|| Box::new(no_such(&setting.id)))?;
    let schedule = if setting.attempts.is_some() || setting.delays.is_some() {
        Some(scheduled(
            setting.attempts.or(Some(current.attempts)),
            setting
                .delays
                .clone()
                .or_else(|| Some(current.delays.clone())),
        )?)
    } else {
        None
    };
    if ctx.dry_run {
        return reading::report(ctx);
    }
    let rekeyed = setting.token.is_some() || setting.reader.is_some();
    backends::changed(&path, &mut |kept: &mut Kept| {
        if let Some(one) = kept.backends.iter_mut().find(|one| one.id == setting.id) {
            if let Some((attempts, delays)) = schedule.clone() {
                one.attempts = attempts;
                one.delays = delays;
            }
            match &setting.members {
                Some(Offering::Offer(name)) => one.members = Some(name.clone()),
                Some(Offering::Withdraw) => {
                    one.members = None;
                    one.receivers.clear();
                }
                None => {}
            }
            if rekeyed {
                one.proof = BackendProof::Held(CHANGED.to_owned());
            }
        }
        Ok(())
    })
    .map_err(|why| read_failure(&why))?;
    if rekeyed {
        let secrets = backends::secrets_at(ctx).ok_or_else(nowhere)?;
        let mut held = backends::secrets(&secrets)
            .map_err(|why| read_failure(&why))?
            .held
            .remove(&setting.id)
            .unwrap_or_default();
        if setting.token.is_some() {
            held.token.clone_from(&setting.token);
        }
        if setting.reader.is_some() {
            held.reader.clone_from(&setting.reader);
        }
        backends::hold(&secrets, &setting.id, Some(&held)).map_err(|why| read_failure(&why))?;
    }
    reading::report(ctx)
}

/// Remove a back-end and forget its secret.
fn remove(ctx: &Ctx, id: &str) -> Result<NotifyReport, Box<Problem>> {
    let path = backends::at(ctx).ok_or_else(nowhere)?;
    let kept = backends::kept(&path).map_err(|why| read_failure(&why))?;
    if !kept.backends.iter().any(|one| one.id == id) {
        return Err(Box::new(no_such(id)));
    }
    if !ctx.dry_run {
        backends::changed(&path, &mut |kept: &mut Kept| {
            kept.backends.retain(|one| one.id != id);
            if kept.backends.is_empty() {
                kept.told = None;
            }
            Ok(())
        })
        .map_err(|why| read_failure(&why))?;
        let secrets = backends::secrets_at(ctx).ok_or_else(nowhere)?;
        backends::hold(&secrets, id, None).map_err(|why| read_failure(&why))?;
    }
    reading::report(ctx)
}

/// Refused where a secret was given that holds nothing, as a variable named and left
/// unset gives it.
fn given(id: &str, token: Option<&str>, reader: Option<&str>) -> Result<(), Box<Problem>> {
    if [token, reader]
        .into_iter()
        .flatten()
        .any(|one| one.trim().is_empty())
    {
        return Err(Box::new(refused(
            BACKEND_INCOMPLETE,
            &format!("The back-end {id} was given a secret that holds nothing"),
            "Nothing was changed.",
            "Set the variable named with --token-env or --reader-env, and ask again",
        )));
    }
    Ok(())
}

/// The address `adding` is reached at, once its name, its address, its host and what its
/// kind needs are all as they must be.
fn checked(adding: &Adding) -> Result<String, Box<Problem>> {
    given(
        &adding.id,
        adding.token.as_deref(),
        adding.reader.as_deref(),
    )?;
    if !names_a_key(&adding.id) {
        return Err(Box::new(refused(
            NOT_A_BACKEND_NAME,
            &format!("{} cannot name a back-end", adding.id),
            "Nothing was added.",
            "Name it after where it delivers, as in home-ntfy",
        )));
    }
    let url = Url::parse(&adding.url)
        .ok()
        .filter(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.has_host()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        })
        .ok_or_else(|| {
            Box::new(refused(
                NOT_A_BACKEND_ADDRESS,
                &format!(
                    "{} is not an address the back-end {} can be reached at",
                    adding.url, adding.id
                ),
                "A back-end is reached at an http or https address with no login, query or \
                 fragment in it, so nothing was added.",
                "Give the server's address, and its token with --token-env",
            ))
        })?;
    let missing = match adding.kind {
        BackendKind::Ntfy if adding.topic.as_deref().is_none_or(str::is_empty) => Some("a topic"),
        BackendKind::Gotify if adding.token.as_deref().is_none_or(str::is_empty) => {
            Some("its application token")
        }
        BackendKind::Unifiedpush => Some("a phone, which registers its own push endpoint"),
        _ => None,
    };
    if let Some(missing) = missing {
        return Err(Box::new(refused(
            BACKEND_INCOMPLETE,
            &format!("The back-end {} needs {missing}", adding.id),
            "Nothing was added.",
            "Add it again with what is missing",
        )));
    }
    let host = url.host_str().unwrap_or_default();
    if !adding.hosted
        && HOSTED
            .iter()
            .any(|hosted| host == *hosted || host.ends_with(&format!(".{hosted}")))
    {
        return Err(Box::new(refused(
            HOSTED_UNACKNOWLEDGED,
            &format!("{host} is a hosted service rather than one the household runs"),
            "What is delivered to it leaves the self-hosted posture, so nothing was added.",
            "Add it again with --hosted to accept that, or point it at a server the household runs",
        )));
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// The attempts and delays asked for, or the defaults where none were.
fn scheduled(
    attempts: Option<u32>,
    delays: Option<Vec<u64>>,
) -> Result<(u32, Vec<u64>), Box<Problem>> {
    let attempts = attempts.unwrap_or(ATTEMPTS);
    let delays = delays.unwrap_or_else(|| {
        DELAYS
            .iter()
            .copied()
            .take(usize::try_from(attempts.saturating_sub(1)).unwrap_or(0))
            .collect()
    });
    let fits = (1..=MOST_ATTEMPTS).contains(&attempts)
        && u32::try_from(delays.len()).is_ok_and(|given| given == attempts - 1)
        && delays.iter().all(|minutes| *minutes > 0);
    if fits {
        Ok((attempts, delays))
    } else {
        Err(Box::new(refused(
            NOT_A_RETRY_SCHEDULE,
            "Those attempts and delays do not agree",
            "A delivery is given between one and ten attempts, and every attempt after the first \
             waits the minutes given for it. Nothing was changed.",
            "Give as many delays as there are attempts after the first, as in --attempts 3 \
             --delays 1,10",
        )))
    }
}

/// The refusal of a back-end under a name `kept` holds, where it holds it.
fn taken(kept: &Kept, id: &str) -> Option<Box<Problem>> {
    kept.backends.iter().any(|one| one.id == id).then(|| {
        Box::new(refused(
            BACKEND_NAME_TAKEN,
            &format!("A back-end named {id} is already added"),
            "Nothing was added.",
            "Remove that one, or add this one under another name",
        ))
    })
}

/// No back-end holds the name.
pub(super) fn no_such(id: &str) -> Problem {
    refused(
        NO_SUCH_BACKEND,
        &format!("No back-end is named {id}"),
        "Nothing was changed.",
        "List the back-ends, and name one of them",
    )
}

/// A refusal of what was asked, guided.
fn refused(code: Code, summary: &str, meaning: &str, remedy: &str) -> Problem {
    Problem::new(code, summary.to_owned(), meaning, Remedy::new(remedy)).in_state(State::Guided)
}

/// There is nowhere to keep a back-end.
pub(super) fn nowhere() -> Box<Problem> {
    Box::new(Failure::Nowhere.problem())
}

/// A record that could not be read or written, as its problem.
pub(super) fn read_failure(failure: &Failure) -> Box<Problem> {
    Box::new(failure.problem())
}

#[cfg(test)]
mod tests;
