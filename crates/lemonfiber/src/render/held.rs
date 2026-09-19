//! What one member can watch, printed.
//!
//! The household screen lists who is here and what each has asked for. This lists what
//! is already on the shelf for one of them — and it is one of them rather than all of
//! them because the media server answers it per account, applying that account's age
//! limit, blocked kinds and library access before it says anything.
//!
//! Which is what makes this worth printing at a terminal at all: it is the only way to
//! see what a restriction actually comes to. A limit set on the household screen is a
//! claim about what somebody may watch; this is the list.

use lemonfiber_core::model::{Held, HeldReport, Medium};

use super::Lines;

/// The shelf, a line per thing on it.
pub(super) fn held(report: &HeldReport) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} — what is on the shelf", report.member));

    for holding in &report.holdings {
        lines.put(format!("  {}", titled(holding)));
    }

    // An empty shelf and an unread one read identically on a screen unless one of them
    // says which it is, and the pair is the whole reason this report carries a mark for
    // whether it could be read at all.
    if report.holdings.is_empty() && report.available {
        lines.put("  Nothing this member can watch — which is the answer, not a gap.");
    } else if report.available {
        lines.spaced(counted(report));
    }

    for finding in &report.findings {
        lines.put(format!("  ! {finding}"));
    }
    lines
}

/// One thing, as somebody would say it.
fn titled(holding: &Held) -> String {
    let medium = match holding.medium {
        Medium::Film => "film",
        Medium::Series => "series",
        // Said rather than left blank. A line with no kind on it reads as an oversight
        // in this program, where what it is is the media server using a word this build
        // has not been taught.
        Medium::Other => "something else",
    };
    holding.year.map_or_else(
        || format!("{} ({medium})", holding.title),
        |year| format!("{} ({year}, {medium})", holding.title),
    )
}

/// How much of the shelf this is.
///
/// Said as a count rather than as "everything", because a read answers with as much as
/// it was asked for and a screen that claimed completeness would be claiming something
/// this cannot know: a shelf exactly as long as the number asked for is the one case
/// where there is no telling whether anything was left behind.
fn counted(report: &HeldReport) -> String {
    let films = report
        .holdings
        .iter()
        .filter(|held| held.medium == Medium::Film)
        .count();
    let series = report
        .holdings
        .iter()
        .filter(|held| held.medium == Medium::Series)
        .count();
    // A series is its own plural, which is why only one of the two is chosen between.
    format!(
        "  {} to watch — {films} {}, {series} series",
        report.holdings.len(),
        if films == 1 { "film" } else { "films" },
    )
}

#[cfg(test)]
mod tests {
    use lemonfiber_core::model::{Held, HeldReport, Medium};

    use super::held;

    fn one(title: &str, year: Option<u16>, medium: Medium) -> Held {
        Held {
            id: title.to_owned(),
            title: title.to_owned(),
            year,
            medium,
        }
    }

    fn shown(report: &HeldReport) -> String {
        held(report).text()
    }

    fn a_shelf() -> HeldReport {
        HeldReport {
            member: "Ada".to_owned(),
            id: "a7f3".to_owned(),
            holdings: vec![
                one("A Film", Some(1994), Medium::Film),
                one("A Series", None, Medium::Series),
                one("An Album", Some(1973), Medium::Other),
            ],
            available: true,
            findings: Vec::new(),
        }
    }

    #[test]
    fn every_kind_is_said_in_words_and_a_missing_year_is_left_out() {
        let said = shown(&a_shelf());
        assert!(said.contains("A Film (1994, film)"), "{said}");
        assert!(said.contains("A Series (series)"), "{said}");
        assert!(said.contains("An Album (1973, something else)"), "{said}");
        assert!(said.contains("3 to watch — 1 film, 1 series"), "{said}");
    }

    /// The pair this screen exists to keep apart. An empty shelf says it is empty; a
    /// shelf that could not be read says that instead, and neither is ever printed as
    /// the other.
    #[test]
    fn an_empty_shelf_and_an_unread_one_do_not_read_alike() {
        let empty = shown(&HeldReport {
            member: "Ada".to_owned(),
            available: true,
            ..HeldReport::default()
        });
        assert!(empty.contains("Nothing this member can watch"), "{empty}");

        let unread = shown(&HeldReport {
            member: "Ada".to_owned(),
            findings: vec!["the media server would not say".to_owned()],
            ..HeldReport::default()
        });
        assert!(
            !unread.contains("Nothing this member can watch"),
            "{unread}"
        );
        assert!(
            unread.contains("! the media server would not say"),
            "{unread}"
        );
    }
}
