use lemonfiber_core::app::answered_under;

use super::{every, Read};
use crate::read::table::{self, Wanted, BUNDLE, LOGS, OFFERED, THE_STACK, THIS_BINARY};

/// The read served at `path`, as it is published.
fn published(path: &str) -> Option<Read> {
    every().into_iter().find(|read| read.path == path)
}

/// Every read this surface serves is published, and nothing it does not serve.
#[test]
fn every_read_served_is_published_and_nothing_else() {
    let mut served: Vec<&str> = OFFERED.iter().copied().chain([LOGS, BUNDLE]).collect();
    let mut listed: Vec<&str> = every().iter().map(|read| read.path).collect();
    served.sort_unstable();
    listed.sort_unstable();
    assert_eq!(listed, served);
}

/// A read answers either under a kind or with a file, and never with neither.
#[test]
fn every_read_answers_under_a_kind_or_with_a_file() {
    for read in every() {
        assert_ne!(
            read.file,
            !read.kinds.is_empty(),
            "{} says it answers with {:?} and a file: {}",
            read.path,
            read.kinds,
            read.file
        );
    }
    assert!(published(BUNDLE).is_some_and(|bundle| bundle.file));
}

/// A read that forks on a parameter answers under the kind of each command it reaches.
#[test]
fn a_read_that_forks_answers_under_both_kinds() {
    let kinds = |path: &str| published(path).map(|read| read.kinds).unwrap_or_default();
    assert_eq!(kinds(table::FORMS), ["forms", "preview"]);
    assert_eq!(kinds(table::CONFIG), ["config"]);
    assert_eq!(kinds(table::EXPLAIN), ["glossary", "word"]);
    assert_eq!(kinds(table::UPDATE), ["self-update", "update"]);
    assert_eq!(kinds(LOGS), ["job", "log"]);
}

/// What a read may be given is what the table lets it take, and whether each may be
/// given twice is what the table says of it.
#[test]
fn a_reads_parameters_are_what_the_table_lets_it_take() {
    let logs = published(LOGS)
        .map(|read| read.parameters)
        .unwrap_or_default();
    let named: Vec<(&str, bool)> = logs
        .iter()
        .map(|parameter| (parameter.name, parameter.repeatable))
        .collect();
    assert_eq!(
        named,
        [
            ("form", true),
            ("service", true),
            ("tail", false),
            ("follow", false)
        ]
    );
    assert!(published(table::VERSION).is_some_and(|read| read.parameters.is_empty()));
}

/// However a read is asked, it answers under a kind it is published under.
///
/// Every parameter a read could be given, each set alone on the plainest request, and
/// every command that reaches: a fork the published list missed would be a kind a
/// client was never told to expect.
#[test]
fn however_a_read_is_asked_it_answers_under_a_published_kind() {
    let plainest = Wanted::naming_everything;
    let mut asked: Vec<Wanted> = vec![
        Wanted {
            forms: vec!["library".to_owned()],
            ..plainest()
        },
        Wanted {
            defaults: Some("true".to_owned()),
            ..plainest()
        },
        Wanted {
            season: Some("1".to_owned()),
            ..plainest()
        },
        Wanted {
            key: Some("DATA_ROOT".to_owned()),
            ..plainest()
        },
        Wanted {
            only: Some("storage".to_owned()),
            ..plainest()
        },
        Wanted {
            word: Some("seeding".to_owned()),
            ..plainest()
        },
        Wanted {
            to: Some("0.1.0".to_owned()),
            ..plainest()
        },
        Wanted {
            most: Some("5".to_owned()),
            ..plainest()
        },
    ];
    for what in [THIS_BINARY, THE_STACK] {
        asked.push(Wanted {
            what: Some(what.to_owned()),
            ..plainest()
        });
    }
    for tier in lemonfiber_core::uninstall::TIERS {
        asked.push(Wanted {
            tier: Some(tier.name().to_owned()),
            ..plainest()
        });
    }
    for read in every() {
        for given in &asked {
            let Ok(command) = table::named(read.path, given.clone()) else {
                continue;
            };
            for kind in answered_under(&command) {
                assert!(
                    read.kinds.contains(&kind.as_str()),
                    "{} answers under {kind} and is not published under it",
                    read.path
                );
            }
        }
    }
}
