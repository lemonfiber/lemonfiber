use super::{declared, Declared, LONGEST, MOST_COVERED, MOST_DECLARED};
use crate::config::env::EnvFile;
use crate::unmanaged::CoveredKind;

/// The areas a file holding `line` as its declarations lists, among `things`.
fn listed(line: &str, things: &[(CoveredKind, String)]) -> Declared {
    declared(
        &EnvFile::parse(&format!("LEMONFIBER_UNMANAGED={line}\n")),
        things,
    )
}

/// `count` files beneath `config`, each a path.
fn files_beneath_config(count: usize) -> Vec<(CoveredKind, String)> {
    (0..count)
        .map(|at| (CoveredKind::Path, format!("config/file-{at}.yml")))
        .collect()
}

#[test]
fn nothing_declared_lists_nothing_and_leaves_nothing_out() {
    let read = declared(&EnvFile::parse("DATA_ROOT=/srv\n"), &[]);
    assert!(read.listed.is_empty());
    assert_eq!(read.more, 0);
}

#[test]
fn the_areas_past_the_most_listed_are_counted_rather_than_listed() {
    let line: Vec<String> = (0..=MOST_DECLARED)
        .map(|at| format!("area-{at}=kept by hand"))
        .collect();
    let read = listed(&line.join(","), &[]);
    assert_eq!(read.listed.len(), MOST_DECLARED);
    assert_eq!(read.more, 1);
    assert_eq!(
        read.listed.first().map(|one| one.area.as_str()),
        Some("area-0")
    );
}

#[test]
fn a_reason_past_the_longest_is_cut_and_says_so_and_one_at_it_is_whole() {
    let long = "w".repeat(LONGEST + 1);
    let whole = "w".repeat(LONGEST);
    let read = listed(&format!("sonarr={long},radarr={whole}"), &[]);
    let cut: Vec<(usize, bool)> = read
        .listed
        .iter()
        .map(|one| (one.why.chars().count(), one.why_cut))
        .collect();
    assert_eq!(cut, [(LONGEST, true), (LONGEST, false)]);
    assert!(read.listed.iter().all(|one| !one.area_cut));
}

#[test]
fn an_area_past_the_longest_is_cut_and_says_so() {
    let read = listed(&format!("{}=kept by hand", "a".repeat(LONGEST + 1)), &[]);
    let area = read.listed.first();
    assert_eq!(area.map(|one| one.area.chars().count()), Some(LONGEST));
    assert_eq!(area.map(|one| one.area_cut), Some(true));
}

#[test]
fn a_character_that_moves_a_cursor_or_turns_text_around_is_written_as_its_bytes() {
    let read = listed("sonarr=mine\u{1b}[2J \u{202e}gpj.exe", &[]);
    assert_eq!(
        read.listed.first().map(|one| one.why.as_str()),
        Some("mine\\1b[2J \\e2\\80\\aegpj.exe")
    );
}

#[test]
fn an_area_lists_what_it_covers_as_what_each_is_and_nothing_it_does_not() {
    let things = vec![
        (CoveredKind::Service, "config".to_owned()),
        (
            CoveredKind::Path,
            "config/recyclarr/recyclarr.yml".to_owned(),
        ),
        (CoveredKind::Path, "configuration.yml".to_owned()),
        (CoveredKind::Setting, "DATA_ROOT".to_owned()),
    ];
    let read = listed("config=mine,radar=a typo", &things);
    let covers: Vec<Vec<(CoveredKind, &str)>> = read
        .listed
        .iter()
        .map(|one| {
            one.covers
                .iter()
                .map(|covered| (covered.kind, covered.name.as_str()))
                .collect()
        })
        .collect();
    assert_eq!(
        covers,
        [
            vec![
                (CoveredKind::Service, "config"),
                (CoveredKind::Path, "config/recyclarr/recyclarr.yml")
            ],
            Vec::new()
        ]
    );
}

#[test]
fn the_things_past_the_most_covered_are_counted_rather_than_listed() {
    let read = listed("config=mine", &files_beneath_config(MOST_COVERED + 3));
    let area = read.listed.first();
    assert_eq!(area.map(|one| one.covers.len()), Some(MOST_COVERED));
    assert_eq!(area.map(|one| one.covers_more), Some(3));
}
