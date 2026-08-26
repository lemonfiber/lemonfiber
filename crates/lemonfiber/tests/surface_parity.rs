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
//! **The web offers nothing unaccounted for.** Every action the web translates and
//! every route it serves appears in some row, so an action added there without a
//! command behind it fails here as well as being refused by the contract — and a
//! route added there without a decision about what request it answers fails too.
//! A route that answers no command-line request is declared by name below, which
//! is what turns "it does not belong in the table" into something somebody wrote
//! down rather than something nobody noticed.
//!
//! **The terminal offers nothing unaccounted for either.** The screen's own lists
//! live in `acting/`, which is `mod acting;` in `main.rs` and private to the binary,
//! so this could never read them; what it reads instead is
//! [`lemonfiber::reaching`], the projection of them the library publishes, which the
//! screen's own tests hold to those lists in both directions. So a row claiming the
//! dashboard reaches a request it does not fails here, and an action or a question
//! the screen offers that no row accounts for fails here — the pair the web column
//! has had since this table was written, and the reason twenty-six rows no longer
//! have to be re-read by eye every slice.
//!
//! Three rows name a screen that is not the dashboard, and three requests the
//! dashboard answers are its panels rather than its lists. A panel is a rendering
//! and not a named request, so there is no list of them to hold a row against;
//! those are declared in the projection and remain a reader's job.

use std::collections::BTreeSet;
use std::fs;

use clap::CommandFactory as _;
use lemonfiber::cli::Cli;
use lemonfiber::reaching;
use lemonfiber_api::actions::OFFERED;

/// The table, relative to this crate.
const TABLE: &str = "../../.docs/architecture/surface-parity.md";

/// Where the web surface declares the routes it serves.
const API: &str = "../lemonfiber-api/src";

/// The heading the parity table sits under.
const HEADING: &str = "## The table";

/// Where the page states what its rows add up to.
const SUMMARY: &str = "## What the table adds up to";

/// What either column may say that is not something it names.
///
/// Four verdicts, shared, because the two columns are counted the same way: a row
/// that names a thing and adds none of these reaches the request in full, one
/// carrying `partial` reaches it in part, and one carrying `excepted` reaches all of
/// it but a thing that will never be offered there. The web column may also name an
/// action or a route and the terminal column a screen, which is the only difference
/// between them and is why each still has its own list of the words it may name.
const VERDICTS: [&str; 4] = ["none", "intrinsic", "partial", "excepted"];

/// What the terminal column may say.
///
/// Four screens and four verdicts. A screen is named rather than described so a
/// reader checking by eye is checking one word against one file.
const TERMINAL_WORDS: [&str; 8] = [
    "dashboard",
    "viewer",
    "wizard",
    "glossary",
    "none",
    "intrinsic",
    "partial",
    "excepted",
];

/// The words that qualify another and cannot stand on their own.
///
/// Two, and the difference between them is the difference the whole page turns on: a
/// request reached in part is short of something somebody is going to build, and one
/// reached but for an exception is short of something nobody is. `intrinsic` cannot
/// do this job — it says nothing is reachable and nothing ever will be, which of a
/// request reached in all but one argument is simply untrue.
const QUALIFIERS: [&str; 2] = ["partial", "excepted"];

/// A verdict that admits no company: nothing is reachable, or nothing ever will be.
const ALONE: [&str; 2] = ["none", "intrinsic"];

/// The word for a request reached in part, which is a gap somebody is going to close.
const PARTIAL: &str = "partial";

/// The word for a request reached but for something that will never be offered there.
const EXCEPTED: &str = "excepted";

/// The screen a row names where the dashboard is what reaches a request.
const DASHBOARD: &str = "dashboard";

/// The routes that answer no request the command line accepts.
///
/// Each is the web's own half of something that has no command-line form. The
/// stream is one of the three exceptions running the other way — a live gather
/// nothing prints. The action path is how every action is asked for, and the rows
/// name the actions rather than the path they arrive on. The job path is the other
/// end of the name a long-running action is answered with, and being answered with
/// a name is itself the web's own arrangement. The door a password is exchanged at
/// is the same kind of thing: a command line is already sitting at the machine that
/// printed this run's token, so there is nothing for it to log in to.
///
/// Anything else this surface routes belongs in a row.
const UNREQUESTED: [&str; 4] = [
    "/api/events",
    "/api/actions/{action}",
    "/api/jobs/{job}",
    "/api/session",
];

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
        .filter(|row| !tokens(&row.web).iter().any(|word| VERDICTS.contains(word)))
        .count();
    let partial = counted
        .iter()
        .filter(|row| tokens(&row.web).contains(&PARTIAL))
        .count();
    let none = counted.iter().filter(|row| row.web == "none").count();
    let silent = counted.iter().filter(|row| row.terminal == "none").count();
    // The same three counts down the other column. It had only the one that says how
    // many requests reach no screen at all, which reached zero — so a column with
    // seventeen rows still losing an argument each read as finished, and every slice
    // closing one of them moved a figure nothing was reading.
    let reached = counted
        .iter()
        .filter(|row| {
            !tokens(&row.terminal)
                .iter()
                .any(|word| VERDICTS.contains(word))
        })
        .count();
    let short = counted
        .iter()
        .filter(|row| tokens(&row.terminal).contains(&PARTIAL))
        .count();
    // And the terminal's own exceptions, which the web column counts by the word it
    // spells one with. A row reached but for something that will never be offered
    // there is neither full nor a gap, and counted as either it would make one of the
    // other two figures a lie — the gap count claiming work outstanding that is not,
    // or the full count claiming a reach the row itself denies.
    let excepted = counted
        .iter()
        .filter(|row| tokens(&row.terminal).contains(&EXCEPTED))
        .count();
    let intrinsic = counted
        .iter()
        .filter(|row| tokens(&row.web).contains(&"intrinsic"))
        .count();

    // Line breaks fall between a number and the noun it counts, so the section is
    // read as one run of words rather than as lines.
    //
    // The summary alone, and not the page. Written against the whole page, each
    // number was looked for on its own and found anywhere — `six` inside
    // `twenty-six`, `ten` inside `written`, `one` inside `none` — and the paragraph
    // below this one, which records the last time these numbers drifted, was itself
    // supplying `eleven` and `ten` to the search. The sentence that documented the
    // bug was what hid the next one.
    let page = fs::read_to_string(TABLE).unwrap_or_default().to_lowercase();
    let summary = page
        .split_once(&SUMMARY.to_lowercase())
        .map_or_else(String::new, |(_, rest)| {
            rest.split_once("\n## ")
                .map_or_else(|| rest.to_owned(), |(section, _)| section.to_owned())
        });
    let summary: String = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        summary.len() > 200,
        "read {} characters under `{SUMMARY}`, which is not a paragraph — the heading \
         has moved and this is no longer reading the summary",
        summary.len()
    );

    // Each figure with the words that go with it, in both numbers. A count reaching
    // one is not a rarity to be waited out: `none` was zero the day a browser could
    // reach everything, and the gaps have been coming down one at a time ever since.
    // Held to the plural alone, the page would have had to say `one reach it in
    // part` and `one gaps` to stay green — so the guard would have been enforcing
    // prose no one would write, on the page whose whole business is being read.
    for (number, many, one) in [
        (counted.len(), "requests", "request"),
        (full, "reach the web in full", "reaches the web in full"),
        (partial, "reach it in part", "reaches it in part"),
        (none, "do not reach it at all", "does not reach it at all"),
        (partial + none, "gaps", "gap"),
        (silent, "have no terminal form", "has no terminal form"),
        // Named for the column rather than said of it, so neither can satisfy the
        // web's own two: `reach the terminal in part` does not contain `reach it in
        // part`, which is the substring the row above but one is looked for by.
        (
            reached,
            "reach the terminal in full",
            "reaches the terminal in full",
        ),
        (
            short,
            "reach the terminal in part",
            "reaches the terminal in part",
        ),
        // Named the same way and for the same reason: neither `reach the terminal in
        // full` nor `reach it in part` is a substring of this, so the figure the page
        // states about its exceptions cannot be satisfied by a sentence about its
        // gaps.
        (
            excepted,
            "reach the terminal but for an exception",
            "reaches the terminal but for an exception",
        ),
    ] {
        let said = spelled(number);
        let what = if number == 1 { one } else { many };
        assert!(
            summary.contains(&format!("{said} {what}")),
            "the summary says `{said} {what}` for what its rows carry ({number})"
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
    let served = served();
    let mut invented: Vec<String> = Vec::new();
    for row in rows() {
        for token in tokens(&row.web) {
            let Some(named) = token.strip_prefix('`').and_then(|t| t.strip_suffix('`')) else {
                continue;
            };
            let exists = if named.starts_with('/') {
                served.contains(named)
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

/// Every route the web serves is claimed by a row, or declared as answering none.
///
/// The converse of the check above, and the read half of the pair: a route added to
/// the surface without deciding which request it answers fails here, which is the
/// failure that is invisible in review because a diff adding a route looks complete.
#[test]
fn every_route_the_web_serves_is_claimed_by_a_row_or_declared_as_answering_none() {
    let claimed: BTreeSet<String> = rows()
        .iter()
        .flat_map(|row| tokens(&row.web))
        .map(|token| token.trim_matches('`').to_owned())
        .collect();
    let unclaimed: Vec<String> = served()
        .into_iter()
        .filter(|route| !claimed.contains(route) && !UNREQUESTED.contains(&route.as_str()))
        .collect();
    assert!(
        unclaimed.is_empty(),
        "the web serves these and no row says which request they answer — add a row \
         in .docs/architecture/surface-parity.md, or declare it in UNREQUESTED: \
         {unclaimed:?}"
    );
}

/// Every request the dashboard reaches, as the screen itself publishes them.
fn reaches() -> BTreeSet<String> {
    reaching::reached().into_iter().map(str::to_owned).collect()
}

/// The requests whose row claims the dashboard reaches them.
fn claiming_the_dashboard() -> BTreeSet<String> {
    rows()
        .iter()
        .filter(|row| tokens(&row.terminal).contains(&DASHBOARD))
        .map(|row| row.request.clone())
        .collect()
}

/// A row claiming the dashboard names a request the dashboard actually reaches.
///
/// The half that catches a cell written ahead of the work — a `dashboard` typed into
/// a column while the key it names is still nobody's.
#[test]
fn a_row_claiming_the_dashboard_names_a_request_it_reaches() {
    let reaches = reaches();
    let invented: Vec<String> = claiming_the_dashboard()
        .into_iter()
        .filter(|request| !reaches.contains(request))
        .collect();

    assert!(
        invented.is_empty(),
        "the table claims the dashboard reaches these and it offers no form of them: \
         {invented:?}"
    );
}

/// Every request the dashboard reaches is accounted for by a row.
///
/// The converse, and the one that catches the failure this column kept having: an
/// action or a question added to the screen while the table went on saying `none`,
/// which is invisible in review because a diff adding a key looks complete.
#[test]
fn every_request_the_dashboard_reaches_is_claimed_by_a_row() {
    let claimed = claiming_the_dashboard();
    let unclaimed: Vec<String> = reaches().difference(&claimed).cloned().collect();

    assert!(
        unclaimed.is_empty(),
        "the dashboard reaches these and no row says so — put `dashboard` in the \
         terminal column in .docs/architecture/surface-parity.md: {unclaimed:?}"
    );
}

/// Every route declared as answering no request is one this surface still serves.
///
/// Without this the declaration outlives the route: a path renamed or removed leaves
/// a name here that excuses nothing, and the next route added under the old name is
/// excused by it.
#[test]
fn every_route_declared_as_answering_none_is_one_the_web_still_serves() {
    let served = served();
    let gone: Vec<&&str> = UNREQUESTED
        .iter()
        .filter(|route| !served.contains(**route))
        .collect();
    assert!(
        gone.is_empty(),
        "these are declared as answering no request and nothing serves them: {gone:?}"
    );
}

/// Every path the web surface routes, read from the calls that declare them.
///
/// A route is declared either with its path written out or with the constant that
/// holds it, so both are read. Only the literals matter in the end — a constant is
/// resolved against the same text, which is the only place a path can be written.
///
/// A route this cannot read is **reported**, never skipped. Dropped quietly it
/// would leave the tests below passing about a route they cannot see, which is the
/// one way this whole file could be green and wrong; the Python gate over the same
/// declarations refuses one for the same reason, and this agrees with it rather
/// than being the softer of the two.
fn served() -> BTreeSet<String> {
    let (paths, unreadable) = routing();
    assert!(
        unreadable.is_empty(),
        "these route calls declare a path this cannot read, so nothing below holds \
         them to anything — write the path out, or hold it in a `const` in the same \
         crate: {unreadable:?}"
    );
    paths
}

/// The paths, and the route calls whose path could not be read.
fn routing() -> (BTreeSet<String>, Vec<String>) {
    let source = api();
    let mut paths = BTreeSet::new();
    let mut unreadable = Vec::new();
    for rest in source.split(".route(").skip(1) {
        let Some((argument, _)) = rest.split_once(',') else {
            unreadable.push(named(rest));
            continue;
        };
        match path_of(argument.trim(), &source) {
            Some(path) => {
                paths.insert(path);
            }
            None => unreadable.push(named(argument)),
        }
    }
    (paths, unreadable)
}

/// As much of a route call as names it in a failure.
fn named(rest: &str) -> String {
    rest.split_whitespace()
        .take(3)
        .collect::<Vec<&str>>()
        .join(" ")
}

/// The path one route call names, written out or held in a constant.
fn path_of(argument: &str, source: &str) -> Option<String> {
    if let Some(quoted) = argument.strip_prefix('"') {
        return quoted.split_once('"').map(|(path, _)| path.to_owned());
    }
    // A name declared twice is a name this cannot resolve, and resolving it to
    // whichever declaration comes first is how a route hides behind another one's
    // path: the check then holds a path something else already serves, and the route
    // in front of it is never held to anything. Two modules calling their own path
    // `PATH` did exactly that.
    let declared = format!("const {argument}: &str = \"");
    let mut found = source.split(&declared).skip(1);
    let first = found.next()?;
    found.next().is_none().then_some(first)?;
    first.split_once('"').map(|(path, _)| path.to_owned())
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
        check(&row.request, "web", &row.web, &VERDICTS, true, &mut wrong);
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
    vocabulary(request, column, &said, allowed, quoting, wrong);
    qualified(request, column, &said, wrong);
    unaccompanied(request, column, &said, wrong);
}

/// Every word in the cell is one this column may say.
///
/// A quoted word is a name — an action, a route — and is the cell's own to choose
/// where the column allows one, so it is passed over rather than looked up.
fn vocabulary(
    request: &str,
    column: &str,
    said: &[&str],
    allowed: &[&str],
    quoting: bool,
    wrong: &mut Vec<String>,
) {
    for token in said {
        let quoted = token.starts_with('`') && token.ends_with('`');
        if quoted && quoting {
            continue;
        }
        if !allowed.contains(token) {
            wrong.push(format!("{request}: the {column} column says `{token}`"));
        }
    }
}

/// A word that qualifies something names the something it qualifies.
///
/// `partial` alone says a request is partly reachable and not by what, which is the
/// shape of a row that stopped being maintained rather than one that was written.
fn qualified(request: &str, column: &str, said: &[&str], wrong: &mut Vec<String>) {
    for qualifier in QUALIFIERS {
        if said.contains(&qualifier) && said.len() == 1 {
            wrong.push(format!(
                "{request}: the {column} column says `{qualifier}` and names nothing to \
                 qualify"
            ));
        }
    }
}

/// A verdict that stands for the whole cell stands in it alone.
///
/// `none` and `intrinsic` are answers about the request entire, so a cell pairing one
/// with a surface would be claiming both that nothing reaches it and that something
/// does.
fn unaccompanied(request: &str, column: &str, said: &[&str], wrong: &mut Vec<String>) {
    for verdict in ALONE {
        if said.contains(&verdict) && said.len() > 1 {
            wrong.push(format!(
                "{request}: the {column} column says `{verdict}` and something else"
            ));
        }
    }
}
