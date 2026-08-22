//! The lines the viewer writes in its own voice.
//!
//! A viewer mostly shows what services said. Now and then it has something of its
//! own to say — a service restarted, an export landed — and those go into the same
//! stream rather than into a banner, because a banner cannot say *when* and when is
//! most of what makes them useful beside the lines around them.
//!
//! Apart from the screen so that both kinds of line are built one way. What a state
//! is called and what the viewer calls itself are vocabulary, and vocabulary that
//! lives next to a state machine gets edited like a state machine.

use lemonfiber_core::ports::docker::{Lifecycle, LogLine, Stream};

/// What the viewer calls itself when a line is its own rather than a service's.
pub(super) const SELF: &str = "lemonfiber";

/// A line the viewer wrote itself.
///
/// Tagged with a service where it is about one, so it sits under the same name as
/// that service's own output and a filter narrowed to it keeps the notice.
pub(super) fn remark(service: &str, said: &str) -> LogLine {
    LogLine {
        service: service.to_owned(),
        stream: Stream::Stdout,
        at: None,
        line: format!("--- {said} ---"),
    }
}

/// A line saying what the engine reported about a service.
pub(super) fn noticed(service: &str, lifecycle: Lifecycle) -> LogLine {
    remark(service, &format!("{service} {}", becoming(lifecycle)))
}

/// What to say about a service that has just reached this state.
///
/// Said as what happened rather than as the engine's word for it: `Exited` is a
/// state, "has stopped" is news.
const fn becoming(lifecycle: Lifecycle) -> &'static str {
    match lifecycle {
        Lifecycle::Created => "was created",
        Lifecycle::Running => "is running again",
        Lifecycle::Paused => "was paused",
        Lifecycle::Restarting => "is restarting",
        Lifecycle::Exited => "has stopped",
        Lifecycle::Removing => "is being removed",
        Lifecycle::Dead => "died",
    }
}
