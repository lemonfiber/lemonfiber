//! Reading what the line is doing, and telling the clients what they may take.
//!
//! One command with two halves, the same way the disk accounting has two: asked
//! nothing it reads and reports, and asked for a limit it declares it, hands it to
//! every download client, and reads back what each one says. There is no third
//! shape — the read-back an applying run reports is the same call the reading run
//! makes, so a rehearsal cannot describe something the real thing would not do.
//!
//! Three things are refused rather than half-done, and all three are refused
//! *before* anything is written, because a limit applied to one client and refused
//! at the next is a household with half a setting:
//!
//! - a share of a line nothing has measured, which holds nothing back;
//! - a schedule on a stack that does not say which zone its clients read a clock
//!   in, which would land the household's evening in the wrong hour;
//! - a limit with no download client to give it to.
//!
//! What the line carries is raised as it goes. A client that was moving with
//! nothing holding it back has just measured the line, at no cost and disturbing
//! nobody, and that reading is better than any figure this could ask for.
//!
//! One thing besides a request makes this write, and it is a decision the operator
//! already took: a month spent against a declared cap. What to do at a cap is
//! chosen when the cap is, precisely so that nobody has to choose at two in the
//! morning — and a run that read the figure, found the month over and handed the
//! clients the declared limits anyway would be that decision taken and never
//! carried out.

use crate::bandwidth::{
    at_the_cap, in_force, weigh, Declared, Metered, Reached, Reading, Sharing, WhenExceeded,
    NOTHING_MEASURED, NOTHING_TO_LIMIT, NO_ZONE,
};
use crate::config::store;
use crate::error::{Amiss, Diagnose, Problem, Remedy, Severity};
use crate::ports::service::{Rates, Wanted, Window};

use crate::app::command::BandwidthAsked as Asked;
use crate::app::targets::{download_targets, project_directory};
use crate::app::Ctx;
use reaching::Fetch;

mod reaching;
mod revising;

/// Where the declaration is kept, beside the environment file a backup captures.
///
/// Equal to [`crate::config::paths::Paths::bandwidth`].
const RECORD: &str = "bandwidth.json";

/// The stack setting that says which zone its containers read a clock in.
const ZONE: &str = "TZ";

/// What the line is doing, and — where anything was asked for — what the clients
/// say after being told.
///
/// # Errors
///
/// Returns a [`Problem`] where what was asked for could not be read, where a share
/// is asked of a line nothing has measured, where a schedule is asked for on a
/// stack with no zone, or where there is no download client to limit.
pub(crate) async fn bandwidth(ctx: &Ctx, asked: &Asked) -> Result<Sharing, Box<Problem>> {
    let now = now(ctx);
    let declared = revising::revised(now, recorded(ctx), asked)?;

    let stack = ctx
        .stack
        .manifest()
        .map_err(|err| Box::new(err.problem()))?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let targets = download_targets(&stack.services, project.as_deref());
    let clients = reaching::opened(ctx, &targets).await;
    let zone = zone(ctx);

    let metered = counting(ctx, &clients, &declared).await;
    let reached = declared
        .cap
        .zip(metered.as_ref())
        .map(|(cap, month)| cap.reached(month.moved()));
    let spent = at_the_cap(&declared, reached);

    // The three refusals are about the request, so only a request answers for
    // them. A run that asked for nothing and found a month over has nothing to
    // turn away — and refusing to report a spent cap because the clients could not
    // be opened would withhold exactly the reading somebody needed.
    if asked.anything() {
        possible(&declared, &clients, zone.as_deref())?;
    }

    // A spent cap turns a run that asked for nothing into one that writes, and it
    // is the only thing that does. The whole of what declaring a cap buys is that
    // the answer was settled in advance and is carried out when the month runs out
    // rather than argued about then — so a run that read the figure, found the
    // month over and handed the clients the declared limits anyway would be a
    // decision taken and never applied.
    let changing = asked.anything() || spent.is_some();

    // A respite lifts the limits rather than changing them, so what the clients
    // are told is nothing at all until it runs out.
    let lifted = declared
        .respite
        .is_some_and(|respite| respite.standing(now).lifting());
    let wanted = wanted(&declared, lifted, reached);

    let writing = changing && !ctx.dry_run;
    let fetch = declared
        .cap
        .map(|_| fetch(spent, declared.stopped, writing));
    let mut holding = Vec::new();
    for client in &clients {
        holding.push(reaching::holding(client, &wanted, fetch, writing).await);
    }

    let declared = settled(declared, &holding, now, tunnelled(&stack), spent, writing);
    // A rehearsal reports what it would declare and records nothing, which is the
    // promise every other write in this product makes. Every other run keeps the
    // record, including one that only read: what the line was seen to carry is
    // learned by watching rather than by being told, and a reading that threw the
    // learning away would be a stack that never came to know its own connection.
    if !ctx.dry_run {
        keep(ctx, &declared);
    }

    Ok(weigh(&crate::bandwidth::Measured {
        declared,
        now,
        zone,
        clients: holding,
        metered,
        applied: writing,
    }))
}

/// Whether what was asked for can be carried out at all, checked before anything
/// is written.
///
/// All three refusals are about the request rather than the machine, and all three
/// are made in one place, before the first client is told anything: a limit applied
/// to one client and refused at the next leaves a household with half a setting and
/// nothing saying which half.
fn possible(
    declared: &Declared,
    clients: &[reaching::Client],
    zone: Option<&str>,
) -> Result<(), Box<Problem>> {
    if clients.is_empty() {
        return Err(Box::new(nothing_to_limit()));
    }
    if declared.rhythm.is_some() && zone.is_none() {
        return Err(Box::new(no_zone()));
    }
    let capacity = declared.capacity;
    let unmeasured = [
        (declared.down, capacity.map(|line| line.down)),
        (declared.up, capacity.map(|line| line.up)),
    ]
    .into_iter()
    .any(|(limit, carried)| {
        limit.is_some_and(|limit| limit.is_share() && Reading::of(limit, carried).bytes().is_none())
    });
    if unmeasured {
        return Err(Box::new(nothing_measured()));
    }
    Ok(())
}

/// What every client is to be held to.
///
/// The quiet hours are unlimited by construction rather than by setting. That is
/// what the household's day is *for*: the whole point of declaring one is to have
/// the line back when nobody is using it, and a stack that stayed throttled
/// overnight would be a stack somebody switches the limits off on.
fn wanted(declared: &Declared, lifted: bool, reached: Option<Reached>) -> Wanted {
    let spent = at_the_cap(declared, reached);
    // A spent cap outranks an override, the same way it outranks everything else
    // true of a metered line: it is the one with a bill behind it, and lifting the
    // limits for an hour is not a thing to do to a month that is already over.
    if lifted && spent.is_none() {
        return Wanted {
            active: Rates::default(),
            quiet: Rates::default(),
            window: None,
        };
    }
    let capacity = declared.capacity;
    let (down, up) = in_force(declared, reached);
    let rates = Rates {
        down: Reading::of(down, capacity.map(|line| line.down)).bytes(),
        up: Reading::of(up, capacity.map(|line| line.up)).bytes(),
    };
    // A crawl runs around the clock. The month is over whatever hour it is, and a
    // household's quiet hours are not extra allowance.
    if spent == Some(WhenExceeded::Throttle) {
        return Wanted {
            active: rates,
            quiet: rates,
            window: None,
        };
    }
    Wanted {
        active: rates,
        quiet: Rates::default(),
        window: declared.rhythm.map(|rhythm| Window {
            from_hour: rhythm.from.hour(),
            from_minute: rhythm.from.minute(),
            to_hour: rhythm.to.hour(),
            to_minute: rhythm.to.minute(),
        }),
    }
}

/// What this run is to do about the clients' fetching.
///
/// Stopping never consults the record: while the cap says stop, every run says so
/// again, so a client somebody started by hand in the middle of a spent month is
/// stopped rather than left as the one exception nobody remembers making.
///
/// Starting always consults it. A client lemonfiber never stopped is one an
/// operator stopped for reasons of their own, and a run that started everything it
/// found stopped would undo a deliberate act on the strength of a month turning
/// over.
fn fetch(spent: Option<WhenExceeded>, stopped: bool, writing: bool) -> Fetch {
    if !writing {
        return Fetch::Ask;
    }
    if spent.is_some_and(WhenExceeded::stops) {
        return Fetch::Stop;
    }
    if stopped {
        return Fetch::Resume;
    }
    Fetch::Ask
}

/// The declaration with whatever this run learned about the line folded into it.
fn settled(
    declared: Declared,
    holding: &[crate::bandwidth::Holding],
    now: u64,
    tunnelled: bool,
    spent: Option<WhenExceeded>,
    writing: bool,
) -> Declared {
    let mut settled = declared;
    // Only a run that told the clients something records what it told them. A
    // rehearsal that wrote this down would leave the next run starting clients
    // nothing had ever stopped.
    if writing {
        settled.stopped = spent.is_some_and(WhenExceeded::stops);
    }
    if let Some(seen) = crate::bandwidth::observed(holding, now, tunnelled) {
        settled.capacity = Some(settled.capacity.map_or(seen, |held| held.raised_by(seen)));
    }
    // A respite that has run out is reported by the run that finds it and cleared
    // by the same run, so the next one does not report it again — and so nothing
    // has to be cleared by hand.
    if settled
        .respite
        .is_some_and(|respite| respite.standing(now).spent())
    {
        settled.respite = None;
    }
    settled
}

/// What the stack moved this calendar month, where a cap was declared to weigh it
/// against.
///
/// Only where one was. Asking every client what it has moved on a stack with no
/// cap is traffic spent on a figure nothing would do anything with.
async fn counting(ctx: &Ctx, clients: &[reaching::Client], declared: &Declared) -> Option<Metered> {
    declared.cap?;
    let today = ctx.today();
    let month = format!("{:04}-{:02}", today.year, today.month);

    let mut down = 0_u64;
    let mut up = 0_u64;
    let mut incomplete = Vec::new();
    let mut counted = false;
    for client in clients {
        let Some(moved) = client.moved(&month).await else {
            incomplete.push(format!(
                "{} would not say what it has moved, so none of it is in this figure.",
                client.name()
            ));
            continue;
        };
        counted = true;
        down = down.saturating_add(moved.down);
        up = up.saturating_add(moved.up);
        if moved.since_start {
            incomplete.push(format!(
                "{} counts only what it has moved since it last started, so this is \
                 short by whatever it moved before that.",
                client.name()
            ));
        }
    }
    // Nothing counted is no figure rather than a zero. A month reported as
    // untouched on a stack whose clients would not answer is the one reading that
    // would let a cap be passed in silence.
    counted.then(|| Metered::of(month, down, up, incomplete))
}

/// Whether the torrent client's traffic goes through a tunnel on this stack.
///
/// Resolved by capability and dependency rather than by name, the same way the
/// tunnel check resolves it, so a stack whose gateway is called something else is
/// read the same way.
fn tunnelled(stack: &lemonfiber_manifest::Manifest) -> bool {
    crate::doctor::vpn::resolve_pair(stack).is_some()
}

/// Which zone the stack tells its containers to read a clock in.
fn zone(ctx: &Ctx) -> Option<String> {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|path| store::read(path).ok())
        .and_then(|file| file.get(ZONE).map(str::to_owned))
        .filter(|zone| !zone.trim().is_empty())
}

/// The moment this reading was taken, in seconds since the epoch.
fn now(ctx: &Ctx) -> u64 {
    ctx.clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// What has been declared about the line, or nothing declared at all.
fn recorded(ctx: &Ctx) -> Declared {
    crate::app::record::beside(ctx, RECORD)
}

/// Keep the declaration where the next run — and a backup — will find it.
///
/// Best effort. A declaration that could not be written is a worse picture on the
/// next run rather than a wrong claim on this one, and the limits themselves are
/// already in the clients by the time this is reached.
fn keep(ctx: &Ctx, declared: &Declared) {
    crate::app::record::keep_beside(ctx, RECORD, declared);
}

/// There is no download client to limit.
fn nothing_to_limit() -> Problem {
    Problem::new(
        NOTHING_TO_LIMIT,
        Severity::Error,
        "There is no download client on this stack to hold to a limit",
        "Limits here are set inside the download clients themselves. With none \
         running there is nothing to set them on, and nothing taking the line \
         either.",
        Remedy::new("Start a form that has a download client in it")
            .with_detail("lemonfiber up tv"),
    )
    .lies_in(Amiss::Asking)
}

/// A share was asked for and nothing has measured the line.
fn nothing_measured() -> Problem {
    Problem::new(
        NOTHING_MEASURED,
        Severity::Error,
        "Nothing has measured this line, so a share of it is not a limit",
        "Half of an unknown number holds nothing back. Rather than write a setting \
         that would do nothing while looking like it was working, this is refused \
         until there is a figure to take a share of.",
        Remedy::new("Say what the line carries, or give a figure instead of a share")
            .with_detail("lemonfiber bandwidth --line 60MiB/6MiB, or --down 2MiB"),
    )
    .lies_in(Amiss::Asking)
}

/// A schedule was asked for on a stack that names no zone.
fn no_zone() -> Problem {
    Problem::new(
        NO_ZONE,
        Severity::Error,
        "Nothing says which zone the download clients read a clock in",
        "The household's hours are kept by the clients themselves, on their own \
         clocks, which is what makes them follow your wall clock through the \
         daylight-saving changes. On a stack that names no zone those clocks are \
         UTC, and quiet hours would start at the wrong time of night — so the \
         schedule is refused rather than applied to the wrong hours.",
        Remedy::new("Set the zone, then ask again")
            .with_detail("lemonfiber config set TZ Europe/Amsterdam"),
    )
    .lies_in(Amiss::Asking)
}

#[cfg(test)]
mod tests;
