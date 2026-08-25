//! The terminal, the loop and the keyboard — the wire to a screen and a person.
//!
//! A module of its own, and deliberately the only untested one in this feature,
//! because it is the part no test can stand in for: a real terminal in raw mode,
//! a real keyboard, and a loop that runs until somebody stops it. Everything
//! about *what* a screen says lives elsewhere — [`crate::dashboard`] and
//! [`crate::logs`] — where it is drawn into a buffer and read back, and so does
//! everything about what a key *means*: which action a letter reaches, what an
//! action may be given, and the question asked before it are [`crate::acting`]'s,
//! where every one of them can be put to a test. What is here is the wire.
//!
//! Both full-screen views are here rather than one each, because what they share is
//! all of the hard part: taking the terminal, giving it back, and reading a keyboard
//! without letting the reading hold anything else up. That share is [`screen`], and
//! a loop each is [`dashboard`] and [`tailing`].
//!
//! Four properties are the whole reason this is shaped the way it is.
//!
//! **A refresh must never hold up a keypress.** Gathering talks to Docker and to
//! half a dozen services, and any of them may take seconds. So the gather runs as
//! a task and the loop waits on whichever arrives first — the next frame's facts
//! or the operator's key — rather than doing one and then the other. A quit
//! typed during a slow refresh is acted on immediately.
//!
//! **The terminal is put back whatever happens.** Raw mode and the alternate
//! screen are global state on a device this process does not own; leaving them on
//! hands the operator a shell that no longer echoes. The restore is a `Drop`, so
//! it runs on the ordinary way out, on an error, and on a panic alike.
//!
//! **A flood must never lock the operator out.** A service in a restart loop can
//! write faster than any screen can draw, and a viewer that worked through all of it
//! would stop answering the one key that would narrow the filter. So a pass takes a
//! bounded number of the lines waiting and lets the oldest of the rest go, counted
//! and said on the screen.
//!
//! **Leaving the screen is not leaving the work.** There is no daemon here and no
//! second process: this one claimed the stack and issued the command, and returning
//! out of the loop would drop the runtime with the task carrying it still at an
//! await — the claim left on disk naming a process that no longer exists, and every
//! later `up`, `down` or browser action refused in its name. So the terminal is
//! given back first and the action is waited on after, on an ordinary screen.

mod dashboard;
mod screen;
mod tailing;

use std::process::ExitCode;

use lemonfiber_core::app::Ctx;
use ratatui::layout::Rect;

pub(crate) use tailing::watching;

use crate::say::say;
use crate::setup::Bare;

/// What a bare run on a machine that is already set up does about it.
///
/// The decision and the words are both [`crate::setup`]'s, and tested there. What
/// is here is only the doing of it: taking a terminal, or printing to one.
pub(crate) async fn configured(ctx: Ctx, bare: Bare) -> ExitCode {
    match bare {
        Bare::Dashboard => dashboard::shown(ctx).await,
        Bare::Guidance => {
            for line in crate::setup::already_set_up() {
                say!("{line}");
            }
            // The whole of it, rather than a line saying where to find it. A run
            // with nowhere to draw is a pipe, a cron line or a CI step, and none of
            // them can go and ask a second time.
            say!("\n{}", lemonfiber::cli::help());
            ExitCode::SUCCESS
        }
    }
}

/// Record what the pane explained, since opening it is the asking.
///
/// Which words those are is [`crate::pane`]'s, where a test can read them back.
fn learned(showing: &str, screen: Rect, rehearsing: bool) {
    crate::context::remember(&crate::pane::taught_on(showing, screen), rehearsing);
}
