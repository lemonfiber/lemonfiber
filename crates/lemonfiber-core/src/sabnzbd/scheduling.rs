//! Writing the household's hours into `SABnzbd`'s own scheduler.
//!
//! This client keeps a schedule the way a crontab does — a list of dated
//! instruction lines, each one a time, a set of days, an action and its argument —
//! rather than the window with two sets of limits qBittorrent keeps. Everything
//! awkward here follows from that, and every piece of it was learned by driving the
//! pinned image rather than read out of a document.
//!
//! **A line is added and removed, never written over.** The whole list is one
//! setting, and the door that takes the whole list at once splits what it is given
//! on spaces unless it finds a comma — so a single line handed to it becomes six
//! nonsense lines, accepted, saved, and silently ignored by the scheduler
//! afterwards. The client's own configuration pages take one line at a time
//! instead, which is both the safe door and the only one that reloads the running
//! scheduler.
//!
//! **What is not a rate line is left exactly as the operator wrote it.** Their
//! nightly pause, their weekday resume, their server switches and folder scans are
//! none of this errand's business and are never read for more than their action.
//!
//! **What *is* a rate line is replaced.** A household's hours and an operator's own
//! speed schedule are one setting, and two of them in one list is a window
//! overridden at an hour nobody chose — which is worse than either alone. So the
//! rate lines become lemonfiber's, and that is the one thing here that takes
//! something away.

use serde::Deserialize;

use crate::ports::service::{Failure, Hours};

use super::Sabnzbd;

/// The days a household's window runs on, as this client numbers them.
const EVERY_DAY: &str = "1234567";

/// The one action lemonfiber owns a line for.
const RATE: &str = "speedlimit";

/// The first field of a line the client will act on.
const ENABLED: &str = "1";

/// The client's schedule, as `get_config` answers with it.
#[derive(Deserialize)]
struct Held {
    config: Sections,
}

/// The section the schedule lives in.
#[derive(Deserialize)]
struct Sections {
    misc: Schedule,
}

/// The instruction lines themselves.
#[derive(Deserialize)]
struct Schedule {
    #[serde(default)]
    schedlines: Vec<String>,
}

/// One rate-limit line lemonfiber writes: when it fires, and what it sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Turn {
    /// The hour it fires, on the client's own clock.
    pub(super) hour: u8,
    /// The minute of it.
    pub(super) minute: u8,
    /// The limit it sets, in the client's own units.
    pub(super) figure: String,
}

impl Turn {
    /// The line as the client stores it.
    ///
    /// Exactly as the client's own page writes one, down to the field order — the
    /// minute before the hour — because this string is compared against what comes
    /// back, and a line that does not match its stored form is one added again on
    /// every run.
    fn stored(&self) -> String {
        format!(
            "{ENABLED} {} {} {EVERY_DAY} {RATE} {}",
            self.minute, self.hour, self.figure
        )
    }
}

impl Sabnzbd {
    /// The instruction lines the client is holding.
    pub(super) async fn schedule_lines(&self) -> Result<Vec<String>, Failure> {
        let held: Held = self
            .read(
                "get_config&section=misc&keyword=schedlines",
                "the schedule could not be read",
            )
            .await?;
        Ok(held.config.misc.schedlines)
    }

    /// Hold this client to `turns` and to nothing else of lemonfiber's making.
    ///
    /// Idempotent by comparison rather than by rewriting: a run that wants what the
    /// client already holds writes nothing, which matters because every line added
    /// or removed reloads the client's scheduler — and a reload is what makes the
    /// boundary land on the current side of the day rather than at the next one.
    ///
    /// Confirmed by reading the list back, because the pages that take a line
    /// answer with a redirect to themselves and say nothing about what they did.
    pub(super) async fn keeping(&self, turns: &[Turn]) -> Result<Vec<String>, Failure> {
        let wanted: Vec<String> = turns.iter().map(Turn::stored).collect();
        let held = self.schedule_lines().await?;

        for line in held.iter().filter(|line| is_rate(line)) {
            if !wanted.contains(line) {
                self.page("scheduling/delSchedule", &[("line", line.as_str())])
                    .await?;
            }
        }
        for turn in turns.iter().filter(|turn| !held.contains(&turn.stored())) {
            self.page(
                "scheduling/addSchedule",
                &[
                    ("minute", &turn.minute.to_string()),
                    ("hour", &turn.hour.to_string()),
                    ("daysofweek", EVERY_DAY),
                    ("action", RATE),
                    ("arguments", &turn.figure),
                ],
            )
            .await?;
        }

        let after = self.schedule_lines().await?;
        let rates = after.iter().filter(|line| is_rate(line)).count();
        if rates != wanted.len() || !wanted.iter().all(|line| after.contains(line)) {
            return Err(self.endpoint.refused(&format!(
                "the household's hours were written and the client is keeping {rates} rate \
                 instructions instead"
            )));
        }
        Ok(after)
    }
}

/// Whether one stored line is a rate-limit instruction.
///
/// By its action alone. A line's time, days and figure are the operator's business
/// wherever the action is not this one, and reading further would be this errand
/// forming an opinion about a schedule it does not own.
fn is_rate(line: &str) -> bool {
    field(line, 4) == Some(RATE)
}

/// One whitespace-separated field of a stored line, where it has one.
fn field(line: &str, at: usize) -> Option<&str> {
    line.split_whitespace().nth(at)
}

/// Which side of the household's day this client's own schedule has it on.
///
/// Read from what the schedule does and from what is in force this moment, rather
/// than worked out: nothing in this product knows the client's local time of day,
/// and the client is the thing that does. A schedule that never changes the rate
/// puts the client on no side of any day, which is what a client with a single
/// standing limit is honestly on.
///
/// It follows that in the half-minute between a schedule being written and the
/// client's scheduler reloading, this reports the side the client is *still* on.
/// That is the truth about the client, and reporting the side it is about to be on
/// would be reporting a limit that is not yet in force.
pub(super) fn side(lines: &[String], limited: bool) -> Option<Hours> {
    let mut figures: Vec<&str> = lines
        .iter()
        .filter(|line| is_rate(line) && field(line, 0) == Some(ENABLED))
        .filter_map(|line| field(line, 5))
        .collect();
    figures.sort_unstable();
    figures.dedup();
    (figures.len() > 1).then_some(if limited { Hours::Active } else { Hours::Quiet })
}

#[cfg(test)]
mod tests {
    use lemonfiber_fixtures::http::{Answer, Fake};

    use super::{is_rate, side, Sabnzbd, Turn};
    use crate::ports::service::Hours;

    /// The operator's own lines: a nightly pause and a weekday resume.
    const THEIRS: [&str; 2] = ["1 0 3 1234567 pause ", "1 15 4 12345 resume "];

    /// A schedule holding those two and one rate line of the operator's own.
    const BEFORE: &str = r#"{"config":{"misc":{"schedlines":[
        "1 0 3 1234567 pause ","1 15 4 12345 resume ","1 0 9 1234567 speedlimit 100k"]}}}"#;

    /// The same schedule once the household's hours are in it.
    const AFTER: &str = r#"{"config":{"misc":{"schedlines":[
        "1 0 3 1234567 pause ","1 15 4 12345 resume ",
        "1 0 7 1234567 speedlimit 5120k","1 30 23 1234567 speedlimit 0"]}}}"#;

    /// The two turns the household's day comes to.
    fn a_window() -> Vec<Turn> {
        vec![
            Turn {
                hour: 7,
                minute: 0,
                figure: "5120k".to_owned(),
            },
            Turn {
                hour: 23,
                minute: 30,
                figure: "0".to_owned(),
            },
        ]
    }

    /// A client whose transport answers each call from `replies` in order.
    fn client(replies: Vec<(u16, &'static str)>) -> Sabnzbd {
        Sabnzbd::new(Fake::scripted(replies), "http://127.0.0.1:8080", "the-key")
    }

    #[tokio::test]
    async fn a_line_the_operator_wrote_survives_the_household_s_hours_being_written() {
        // The whole reason this was deferred. Their pause and their weekday resume
        // are none of this errand's business; only the rate line is replaced, and
        // only because a household's hours and an operator's own speed schedule are
        // one setting that cannot be held twice.
        let transport = Fake::scripted(vec![
            (200, BEFORE),
            (200, "<html/>"),
            (200, "<html/>"),
            (200, "<html/>"),
            (200, AFTER),
        ]);
        let kept = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
            .keeping(&a_window())
            .await;
        assert!(
            kept.is_ok_and(|lines| THEIRS
                .iter()
                .all(|theirs| lines.iter().any(|held| held == theirs))),
            "the operator's own lines are still in the schedule afterwards"
        );

        let asked: Vec<String> = transport
            .requests()
            .iter()
            .map(|request| request.url.clone())
            .collect();
        let sent = asked.join(" ");
        assert!(
            sent.contains("delSchedule") && sent.contains("speedlimit+100k"),
            "their rate line is the only one taken out: {sent}"
        );
        assert!(
            !sent.contains("pause") && !sent.contains("resume"),
            "and neither of their other lines is so much as named: {sent}"
        );
    }

    #[tokio::test]
    async fn a_run_that_wants_what_the_client_already_holds_writes_nothing() {
        // Every line added or removed reloads the client's scheduler, and a reload
        // re-applies the side of the day. A run that changed nothing and reloaded
        // anyway would be a stack disturbing a limit it agreed with.
        let transport = Fake::scripted(vec![(200, AFTER), (200, AFTER)]);
        let kept = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
            .keeping(&a_window())
            .await;
        assert!(kept.is_ok());
        assert!(
            transport
                .requests()
                .iter()
                .all(|request| request.url.contains("get_config")),
            "only the two reads went out"
        );
    }

    #[tokio::test]
    async fn taking_the_window_away_takes_every_rate_line_with_it() {
        let transport = Fake::scripted(vec![
            (200, AFTER),
            (200, "<html/>"),
            (200, "<html/>"),
            (
                200,
                r#"{"config":{"misc":{"schedlines":["1 0 3 1234567 pause "]}}}"#,
            ),
        ]);
        let kept = Sabnzbd::new(transport.clone(), "http://127.0.0.1:8080", "the-key")
            .keeping(&[])
            .await;
        assert!(kept.is_ok_and(|lines| lines.len() == 1));
        assert_eq!(
            transport
                .requests()
                .iter()
                .filter(|request| request.url.contains("delSchedule"))
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn a_schedule_that_does_not_come_back_as_it_was_written_is_a_failure() {
        // The pages that take a line answer with a redirect to themselves and say
        // nothing about what they did, so the list is what settles it.
        let refused = client(vec![
            (200, BEFORE),
            (200, "<html/>"),
            (200, "<html/>"),
            (200, "<html/>"),
            (200, BEFORE),
        ])
        .keeping(&a_window())
        .await;
        assert!(refused.is_err());
    }

    #[tokio::test]
    async fn a_client_that_will_not_say_what_it_is_keeping_is_a_failure() {
        assert!(client(vec![(200, "not json")]).keeping(&[]).await.is_err());
        assert!(client(vec![(403, "no")]).keeping(&[]).await.is_err());
    }

    #[tokio::test]
    async fn a_page_that_refuses_the_line_is_a_failure_rather_than_a_read_back() {
        let refused = client(vec![(200, BEFORE), (403, "no")])
            .keeping(&a_window())
            .await;
        assert!(refused.is_err());
    }

    #[test]
    fn only_the_action_says_whether_a_line_is_this_errand_s() {
        assert!(is_rate("1 0 7 1234567 speedlimit 5120k"));
        assert!(is_rate("0 0 7 12345 speedlimit 5120k"), "disabled or not");
        assert!(!is_rate("1 0 3 1234567 pause "));
        assert!(!is_rate("1 15 4 12345 resume "));
        assert!(!is_rate(""));
    }

    #[test]
    fn a_schedule_that_never_changes_the_rate_puts_the_client_on_no_side_of_the_day() {
        let one = ["1 0 7 1234567 speedlimit 5120k".to_owned()];
        assert_eq!(side(&one, true), None);
        assert_eq!(side(&[], false), None);
        let same = [
            "1 0 7 1234567 speedlimit 5120k".to_owned(),
            "1 30 23 1234567 speedlimit 5120k".to_owned(),
        ];
        assert_eq!(
            side(&same, true),
            None,
            "two lines at one figure switch nothing"
        );
    }

    #[test]
    fn a_schedule_that_switches_the_rate_puts_it_on_the_side_the_limit_says() {
        let switching = [
            "1 0 7 1234567 speedlimit 5120k".to_owned(),
            "1 30 23 1234567 speedlimit 0".to_owned(),
        ];
        assert_eq!(side(&switching, true), Some(Hours::Active));
        assert_eq!(side(&switching, false), Some(Hours::Quiet));
    }

    #[test]
    fn a_line_the_client_will_not_act_on_is_not_a_switch() {
        // A disabled line is one the operator turned off in their own interface,
        // and counting it would report a window nothing is keeping.
        let off = [
            "1 0 7 1234567 speedlimit 5120k".to_owned(),
            "0 30 23 1234567 speedlimit 0".to_owned(),
        ];
        assert_eq!(side(&off, true), None);
    }

    #[tokio::test]
    async fn a_line_is_built_in_the_field_order_the_client_stores_it_in() {
        // The minute before the hour. A line built any other way never matches what
        // comes back, so every run would add it again.
        let transport = Fake::always(Answer::reply(200, AFTER));
        let kept = Sabnzbd::new(transport, "http://127.0.0.1:8080", "the-key")
            .keeping(&a_window())
            .await;
        assert!(kept.is_ok(), "nothing needed adding, so the shape matched");
    }
}
