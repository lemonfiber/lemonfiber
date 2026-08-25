//! What each surface offers, checked against what the parity table claims.
//!
//! The table lives in `.docs/architecture/surface-parity.md` and says, for every
//! request the command line accepts, what the web and the terminal offer instead —
//! an action, a route, a screen, nothing at all, or a reason it never will. A table
//! written once and never read again is a table that quietly stops being true, so
//! this reads it.
//!
//! Three properties are worth the reading.
//!
//! **A new request forces a decision.** Adding a subcommand and not deciding what
//! the other two surfaces do with it is the failure the table exists to prevent,
//! and it is invisible in review — a diff that adds a subcommand looks complete.
//! So a request with no row fails here.
//!
//! **A claim of coverage is checked.** A row naming an action names one the web
//! actually translates, and a row naming a route names one something actually
//! serves. Otherwise the honest cell and the aspirational one read alike.
//!
//! **The web offers nothing unaccounted for.** Every action the web translates
//! appears in some row, so an action added there without a command behind it fails
//! here as well as being refused by the contract.
//!
//! The terminal column is not checked. Its keys live in the one file this workspace
//! deliberately leaves untested — a real terminal in raw mode — so the words are
//! constrained to a vocabulary and the truth of them is a reader's job.

use std::collections::BTreeSet;
use std::fs;

use clap::CommandFactory as _;
use lemonfiber::cli::Cli;
use lemonfiber_api::actions::OFFERED;

/// The table, relative to this crate.
const TABLE: &str = "../../.docs/architecture/surface-parity.md";

/// Where the web surface declares the routes it serves.
const API: &str = "../lemonfiber-api/src";

/// The heading the parity table sits under.
const HEADING: &str = "## The table";

/// What the web column may say that is not an action or a route.
const WEB_WORDS: [&str; 3] = ["none", "intrinsic", "partial"];

/// What the terminal column may say.
///
/// Four screens and three verdicts. A screen is named rather than described so a
/// reader checking by eye is checking one word against one file.
const TERMINAL_WORDS: [&str; 7] = [
    "dashboard",
    "viewer",
    "wizard",
    "glossary",
    "none",
    "intrinsic",
    "partial",
];

/// A word that qualifies another and cannot stand on its own.
const QUALIFIER: &str = "partial";

/// A verdict that admits no company: nothing is reachable, or nothing ever will be.
const ALONE: [&str; 2] = ["none", "intrinsic"];

/// One row of the parity table.
struct Row {
    /// The command-line request it is about.
    request: String,
    /// What the web offers for it.
    web: String,
    /// What the terminal offers for it.
    terminal: String,
    /// How it stands, in prose.
    standing: String,
}

/// Every row of the parity table, in the order it is written.
///
/// Bounded by the heading rather than by the shape of a row, because the page holds
/// another table above this one and two readings of "a line starting with a pipe"
/// would take both.
fn rows() -> Vec<Row> {
    let page = fs::read_to_string(TABLE).unwrap_or_default();
    let after = page.split_once(HEADING).map(|(_, rest)| rest);
    let Some(after) = after else {
        return Vec::new();
    };
    let table = after.split_once("\n## ").map_or(after, |(head, _)| head);
    table
        .lines()
        .filter(|line| line.starts_with('|'))
        .filter_map(row)
        .collect()
}

/// One row, or nothing for the header and the rule beneath it.
fn row(line: &str) -> Option<Row> {
    let cells: Vec<&str> = line.split('|').map(str::trim).collect();
    let (request, web, terminal, standing) =
        (cells.get(1)?, cells.get(2)?, cells.get(3)?, cells.get(4)?);
    if !request.starts_with('`') {
        return None;
    }
    Some(Row {
        request: request.trim_matches('`').to_owned(),
        web: (*web).to_owned(),
        terminal: (*terminal).to_owned(),
        standing: (*standing).to_owned(),
    })
}

/// One cell, as the comma-separated things it names.
fn tokens(cell: &str) -> Vec<&str> {
    cell.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect()
}

/// Every request the command line accepts, as clap renders them.
///
/// `help` is clap's own addition to a command that has subcommands, not something
/// this command line declares, so it is not one of them.
fn requests() -> BTreeSet<String> {
    Cli::command()
        .get_subcommands()
        .map(|command| command.get_name().to_owned())
        .filter(|name| name != "help")
        .collect()
}

/// Every `.rs` file the web surface is declared in, read as one text.
fn api() -> String {
    let mut read = String::new();
    let mut looking = vec![std::path::PathBuf::from(API)];
    while let Some(here) = looking.pop() {
        let Ok(entries) = fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                looking.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                read.push_str(&fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    read
}

/// Every request has exactly one row, and every row names a request.
///
/// Both directions, because they fail differently: a request with no row is a
/// surface decision nobody made, and a row with no request is a claim about
/// something that no longer exists.
/// The numbers the page states about itself, held to the rows it states them about.
///
/// The summary said ten and five where the rows said eleven and four. Nothing read
/// it, so a page whose whole purpose is to be counted had stopped being countable.
#[test]
fn the_count_the_page_states_is_the_count_of_its_rows() {
    let counted = rows();
    assert!(!counted.is_empty(), "the table was read");

    let full = counted
        .iter()
        .filter(|row| !tokens(&row.web).iter().any(|word| WEB_WORDS.contains(word)))
        .count();
    let partial = counted
        .iter()
        .filter(|row| tokens(&row.web).contains(&QUALIFIER))
        .count();
    let none = counted.iter().filter(|row| row.web == "none").count();
    let intrinsic = counted
        .iter()
        .filter(|row| tokens(&row.web).contains(&"intrinsic"))
        .count();

    // Line breaks fall between a number and the noun it counts, so the page is read
    // as one run of words rather than as lines.
    let page = fs::read_to_string(TABLE).unwrap_or_default().to_lowercase();
    let page: String = page.split_whitespace().collect::<Vec<_>>().join(" ");
    for (number, what) in [
        (counted.len(), "requests"),
        (full, "reach the web in full"),
        (partial, "reach it in part"),
        (none, "do not reach it at all"),
        (partial + none, "gaps"),
    ] {
        let said = spelled(number);
        assert!(
            page.contains(&format!("{said} ")),
            "the page says `{said}` for the {what} its rows carry ({number})"
        );
    }
    assert_eq!(intrinsic, 1, "one exception, which the page names as `ui`");
}

/// A number as the page writes it, since it writes them as words.
fn spelled(number: usize) -> String {
    const WORDS: [&str; 27] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
        "twenty-one",
        "twenty-two",
        "twenty-three",
        "twenty-four",
        "twenty-five",
        "twenty-six",
    ];
    WORDS
        .get(number)
        .map_or_else(|| number.to_string(), |&word| word.to_owned())
}

#[test]
fn every_request_the_command_line_accepts_has_a_row() {
    let rows = rows();
    let tabled: BTreeSet<String> = rows.iter().map(|row| row.request.clone()).collect();
    let declared = requests();

    assert_eq!(
        rows.len(),
        tabled.len(),
        "a request is written twice, so which row a reader believes is a coin toss"
    );
    let unrowed: Vec<&String> = declared.difference(&tabled).collect();
    assert!(
        unrowed.is_empty(),
        "these requests have no row — decide what the web and the terminal do with \
         each, in .docs/architecture/surface-parity.md: {unrowed:?}"
    );
    let stale: Vec<&String> = tabled.difference(&declared).collect();
    assert!(
        stale.is_empty(),
        "these rows name a request the command line no longer accepts: {stale:?}"
    );
}

/// A row claiming the web reaches a request names something that exists.
#[test]
fn a_row_claiming_the_web_names_an_action_or_a_route_that_exists() {
    let served = api();
    let mut invented: Vec<String> = Vec::new();
    for row in rows() {
        for token in tokens(&row.web) {
            let Some(named) = token.strip_prefix('`').and_then(|t| t.strip_suffix('`')) else {
                continue;
            };
            let exists = if named.starts_with('/') {
                served.contains(&format!(".route(\"{named}\""))
            } else {
                OFFERED.contains(&named)
            };
            if !exists {
                invented.push(format!("{}: `{named}`", row.request));
            }
        }
    }
    assert!(
        invented.is_empty(),
        "the table claims the web offers these and nothing serves them: {invented:?}"
    );
}

/// Every action the web translates is accounted for by a row.
///
/// The converse of the check above, and the one that catches an action added to the
/// web surface without a command behind it — which the contract forbids and which
/// nothing else here would notice.
#[test]
fn every_action_the_web_offers_is_claimed_by_a_row() {
    let claimed: BTreeSet<String> = rows()
        .iter()
        .flat_map(|row| tokens(&row.web))
        .map(|token| token.trim_matches('`').to_owned())
        .collect();
    let unclaimed: Vec<&&str> = OFFERED
        .iter()
        .filter(|action| !claimed.contains(**action))
        .collect();
    assert!(
        unclaimed.is_empty(),
        "the web offers these and no row says which request they answer: {unclaimed:?}"
    );
}

/// Every cell says one of the things a cell may say, and says why where it must.
///
/// A vocabulary rather than free prose, because the two verdicts that matter are one
/// word apart: `none` is work not done and `intrinsic` is work that will never be
/// done, and a cell free to say "n/a" says neither.
#[test]
fn every_cell_speaks_the_vocabulary_and_carries_its_reason() {
    let mut wrong: Vec<String> = Vec::new();
    for row in rows() {
        check(&row.request, "web", &row.web, &WEB_WORDS, true, &mut wrong);
        check(
            &row.request,
            "terminal",
            &row.terminal,
            &TERMINAL_WORDS,
            false,
            &mut wrong,
        );
        if row.standing.is_empty() {
            wrong.push(format!("{}: says nothing about how it stands", row.request));
        }
    }
    assert!(wrong.is_empty(), "{wrong:?}");
}

/// One cell against the words it is allowed, gathering what is wrong with it.
fn check(
    request: &str,
    column: &str,
    cell: &str,
    allowed: &[&str],
    quoting: bool,
    wrong: &mut Vec<String>,
) {
    let said = tokens(cell);
    if said.is_empty() {
        wrong.push(format!("{request}: the {column} column says nothing"));
        return;
    }
    for token in &said {
        let quoted = token.starts_with('`') && token.ends_with('`');
        if quoted && quoting {
            continue;
        }
        if !allowed.contains(token) {
            wrong.push(format!("{request}: the {column} column says `{token}`"));
        }
    }
    if said.contains(&QUALIFIER) && said.len() == 1 {
        wrong.push(format!(
            "{request}: the {column} column is qualified and says nothing to qualify"
        ));
    }
    for verdict in ALONE {
        if said.contains(&verdict) && said.len() > 1 {
            wrong.push(format!(
                "{request}: the {column} column says `{verdict}` and something else"
            ));
        }
    }
}
