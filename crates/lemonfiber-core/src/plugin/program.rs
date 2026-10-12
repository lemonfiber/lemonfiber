//! A proof that runs the image's own program: the container it runs in, settled from the
//! record of the service it proves, and what the run came to.

mod throwaway;

use std::path::Path;
use std::time::Duration;

use lemonfiber_manifest::container::{Posture, UserMode, Value};
use lemonfiber_plugin::{Expect, RUN_TIMEOUT};

use super::container::user_of;
use super::installed::Placed;
use super::judging::ran;
use super::recorded::RunResult;
use super::Verdict;
use crate::app::Ctx;
use crate::config::{DEFAULT_ZONE, PGID_KEY, PUID_KEY, ZONE_KEY};
use crate::ports::program::{Configuration, Ended, Program, Run, Unran};
use crate::project::entry::{
    configuration, constant_text, DROPPED, KEPT, OPERATOR_FALLBACK, PROCESSES, SECURITY,
};

pub(crate) use throwaway::{digest as starting_digest, keep, Starting};

/// The most of a run's standard output that is read.
pub(crate) const READ_AT_MOST: usize = 64 * 1024;

/// The most of a run's standard output a recording keeps.
pub(crate) const RECORDED_AT_MOST: usize = 1024;

/// The bytes in one mebibyte, which a memory ceiling is declared in.
const MEBIBYTE: i64 = 1024 * 1024;

/// What lemonfiber writes for a service as its time zone and the operator's ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Operator {
    /// The time zone.
    pub(crate) zone: String,
    /// The operator's uid and gid.
    pub(crate) ids: (String, String),
}

impl Operator {
    /// What this machine's settings write, each falling back as the entry's does.
    pub(crate) fn of(ctx: &Ctx) -> Self {
        let zone = ctx
            .settings
            .env_file
            .as_deref()
            .and_then(|file| crate::config::store::read(file).ok())
            .and_then(|file| file.get(ZONE_KEY).map(str::trim).map(str::to_owned))
            .filter(|zone| !zone.is_empty())
            .unwrap_or_else(|| DEFAULT_ZONE.to_owned());
        let ids = ctx.settings.service_user.map_or_else(
            || (OPERATOR_FALLBACK.to_owned(), OPERATOR_FALLBACK.to_owned()),
            |(uid, gid)| (uid.to_string(), gid.to_string()),
        );
        Self { zone, ids }
    }

    /// What the entry writes where nothing is set, which every recording is made with so
    /// that one machine's settings never reach a recording another reproduces.
    pub(crate) fn recording() -> Self {
        Self {
            zone: DEFAULT_ZONE.to_owned(),
            ids: (OPERATOR_FALLBACK.to_owned(), OPERATOR_FALLBACK.to_owned()),
        }
    }
}

/// What a run came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Came {
    /// It exited, having written this.
    Ended(RunResult),
    /// It was still running at its limit, in seconds, and was stopped.
    Overran(u32),
    /// It could not be run, for this reason.
    Unproven(String),
}

/// The seconds a run is given: what it declares, held to the bounds, or the default.
pub(crate) fn limit(run: &lemonfiber_plugin::Run) -> u32 {
    run.timeout_s
        .unwrap_or(RUN_TIMEOUT.default)
        .clamp(RUN_TIMEOUT.min, RUN_TIMEOUT.max)
}

/// The container a run of `run` is made in, for the service `placed` records.
pub(crate) fn settled(
    placed: &Placed,
    run: &lemonfiber_plugin::Run,
    operator: &Operator,
    configured: Option<&Path>,
) -> Run {
    let fields = &placed.fields;
    let user = user_of(placed);
    let mut env: Vec<String> = fields
        .env
        .iter()
        .filter_map(|(name, value)| match value {
            Value::Constant(constant) => Some(format!("{name}={}", constant_text(constant))),
            Value::Setting(_) | Value::Url(_) | Value::Secret(_) => None,
        })
        .collect();
    env.push(format!("{ZONE_KEY}={}", operator.zone));
    if user == UserMode::PuidEnv {
        env.push(format!("{PUID_KEY}={}", operator.ids.0));
        env.push(format!("{PGID_KEY}={}", operator.ids.1));
    }
    let confined = fields.posture == Posture::Confined;
    Run {
        image: format!("{}@{}", placed.image, placed.digest),
        args: run.args.clone(),
        env,
        user: (user == UserMode::Uid).then(|| format!("{}:{}", operator.ids.0, operator.ids.1)),
        security_opt: SECURITY.map(str::to_owned).to_vec(),
        cap_drop: DROPPED.map(str::to_owned).to_vec(),
        cap_add: if confined {
            Vec::new()
        } else {
            KEPT.map(str::to_owned).to_vec()
        },
        pids_limit: i64::from(PROCESSES),
        read_only: confined,
        memory: fields
            .mem_limit_mib
            .map(|mib| i64::from(mib).saturating_mul(MEBIBYTE)),
        configuration: configured.map(|source| Configuration {
            source: source.to_path_buf(),
            target: configuration(&placed.config_path).to_owned(),
        }),
        within: Duration::from_secs(u64::from(limit(run))),
        read_at_most: READ_AT_MOST,
    }
}

/// Run `run` for the service `placed` records, with a throwaway copy of its starting
/// files, and say what it came to.
pub(crate) async fn performed(
    program: &dyn Program,
    placed: &Placed,
    run: &lemonfiber_plugin::Run,
    operator: &Operator,
    starting: Option<Starting<'_>>,
) -> Came {
    let copy = match throwaway::Throwaway::of(starting) {
        Ok(copy) => copy,
        Err(why) => return Came::Unproven(why),
    };
    let asked = settled(placed, run, operator, Some(copy.path()));
    match program.run(&asked).await {
        Ok(ran) => match ran.ended {
            Ended::Exited(exit) => Came::Ended(RunResult {
                exit,
                stdout_starts_with: String::from_utf8_lossy(&ran.stdout).into_owned(),
            }),
            Ended::Overran => Came::Overran(limit(run)),
        },
        Err(unran) => Came::Unproven(unproven(&unran)),
    }
}

/// Why a run that could not be made leaves its proof unproven.
fn unproven(unran: &Unran) -> String {
    crate::text::plain(&unran.to_string())
}

/// What a run came to, held to what its proof declares.
pub(crate) fn verdict(came: &Came, expect: &Expect) -> Verdict {
    match came {
        Came::Ended(result) => {
            let faults: Vec<String> = ran(expect, result)
                .into_iter()
                .map(|fault| fault.said)
                .collect();
            if faults.is_empty() {
                Verdict::Passed
            } else {
                Verdict::Failed { faults }
            }
        }
        Came::Overran(seconds) => Verdict::Failed {
            faults: vec![format!(
                "it was still running at its limit of {seconds} seconds, and was stopped"
            )],
        },
        Came::Unproven(why) => Verdict::Unproven { why: why.clone() },
    }
}

/// A run's result as a recording keeps it: its output cut at a character boundary within
/// the most a recording holds.
pub(crate) fn recorded(result: &RunResult) -> RunResult {
    let text = &result.stdout_starts_with;
    let mut end = RECORDED_AT_MOST.min(text.len());
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    RunResult {
        exit: result.exit,
        stdout_starts_with: text.get(..end).unwrap_or_default().to_owned(),
    }
}

#[cfg(test)]
pub(crate) mod tests;
