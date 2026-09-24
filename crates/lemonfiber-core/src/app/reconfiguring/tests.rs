use super::{assessed, edited, record, recorded, refusal, Edited, Findings, SETTINGS};
use crate::config::{Settings, DATA_ROOT_KEY, PROVIDER_PASS_KEY};
use crate::test_support::{a_context, env_without_password};

/// A context over a given environment file, so the baseline this writes is read
/// back from where this run put it.
fn over(env: std::path::PathBuf) -> crate::app::Ctx {
    a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
}

/// A context whose configuration directory is a scratch one of its own.
fn ctx(name: &str) -> crate::app::Ctx {
    over(env_without_password(name))
}

#[test]
fn what_was_written_is_read_back_as_what_lemonfiber_last_wrote() {
    let ctx = ctx("recorded");
    record(&ctx, DATA_ROOT_KEY, "/srv/old");
    let held = recorded(&ctx).and_then(|base| {
        base.entry(SETTINGS, DATA_ROOT_KEY)
            .map(|record| record.value.clone())
    });
    assert_eq!(held.as_deref(), Some("/srv/old"));
}

#[test]
fn a_credential_is_never_written_into_a_second_file() {
    // The record exists to tell an edit from lemonfiber's own value. Buying that
    // for a password would mean keeping the password twice.
    //
    // A setting is recorded first so the record itself is there to look in: an
    // assertion made against a record that was never formed would pass whether or
    // not the credential had been kept out of it.
    let ctx = ctx("credential");
    record(&ctx, DATA_ROOT_KEY, "/srv/old");
    // A second setting, so the record this looks in is one that was read back and
    // added to rather than formed afresh — the state every change after the first
    // is made against.
    record(&ctx, crate::config::PROJECT_KEY, "lemonfiber");
    record(&ctx, PROVIDER_PASS_KEY, "the-password");
    let held = recorded(&ctx);
    assert!(held.is_some(), "the record was formed");
    let credential = held.and_then(|base| base.entry(SETTINGS, PROVIDER_PASS_KEY).cloned());
    assert_eq!(credential, None);
}

/// What lemonfiber last wrote, recorded so an edit can be judged against it.
fn wrote(ctx: &crate::app::Ctx, key: &str, value: &str) {
    record(ctx, key, value);
}

#[test]
fn a_value_the_file_holds_that_lemonfiber_wrote_is_no_edit_at_all() {
    // The three-way comparison's ordinary answer: the file still holds what
    // lemonfiber put there, so the change is lemonfiber's own to make.
    let ctx = ctx("ours");
    wrote(&ctx, DATA_ROOT_KEY, "/srv/old");
    assert_eq!(
        edited(&ctx, DATA_ROOT_KEY, Some("/srv/old"), "/srv/new"),
        None
    );
}

#[test]
fn a_value_the_file_no_longer_holds_is_reported_from_both_sides() {
    let ctx = ctx("theirs");
    wrote(&ctx, DATA_ROOT_KEY, "/srv/old");
    assert_eq!(
        edited(&ctx, DATA_ROOT_KEY, Some("/mnt/theirs"), "/srv/new"),
        Some(Edited {
            wrote: "/srv/old".to_owned(),
            found: "/mnt/theirs".to_owned(),
            secret: false,
        })
    );
}

#[test]
fn an_edited_credential_is_reported_from_neither_side() {
    // Both halves withheld, and the report still says an edit is there: what the
    // operator needs is that somebody changed it, not what they changed it to.
    let ctx = ctx("edited-credential");
    let mut baseline = crate::baseline::Baseline::new();
    baseline.record(SETTINGS, PROVIDER_PASS_KEY, "the-one-we-wrote", "1");
    super::super::seed::save_baseline(&ctx, &baseline);
    assert_eq!(
        edited(&ctx, PROVIDER_PASS_KEY, Some("theirs"), "another"),
        Some(Edited {
            wrote: crate::config::store::REDACTED.to_owned(),
            found: crate::config::store::REDACTED.to_owned(),
            secret: true,
        })
    );
}

#[test]
fn a_line_taken_out_of_the_file_says_the_line_is_gone() {
    let ctx = ctx("removed");
    wrote(&ctx, DATA_ROOT_KEY, "/srv/old");
    let gone = edited(&ctx, DATA_ROOT_KEY, None, "/srv/new").map(|edit| edit.found);
    assert_eq!(gone.as_deref(), Some("nothing — the line is gone"));
}

#[tokio::test]
async fn a_proposal_over_an_edited_setting_comes_back_turned_away() {
    // The gathering and the deciding together, driven the way the write path
    // drives them: what was found is carried on the proposal, and the proposal
    // itself is the thing that has been turned away.
    let env = env_without_password("assessed");
    let ctx = over(env.clone());
    wrote(&ctx, DATA_ROOT_KEY, "/srv/old");
    let file = crate::config::env::EnvFile::parse("DATA_ROOT=/mnt/theirs\n");
    let proposed = crate::reconfigure::Review::proposed(
        DATA_ROOT_KEY,
        Some("/mnt/theirs"),
        "/srv/new",
        &crate::reconfigure::Consent {
            settled: true,
            rehearsing: false,
        },
    );

    let weighed = assessed(&ctx, proposed, &file, (DATA_ROOT_KEY, "/srv/new"), false).await;

    assert_eq!(weighed.stance, crate::reconfigure::Stance::Blocked);
    assert!(weighed.findings.edited.is_some());
    assert!(weighed
        .refusal
        .is_some_and(|said| said.contains("--confirm")));
}

#[tokio::test]
async fn a_proposal_nothing_stands_in_front_of_comes_back_as_it_went_in() {
    let env = env_without_password("assessed-clear");
    let ctx = over(env);
    let file = crate::config::env::EnvFile::parse("DATA_ROOT=/srv/old\n");
    let proposed = crate::reconfigure::Review::proposed(
        DATA_ROOT_KEY,
        Some("/srv/old"),
        "/srv/new",
        &crate::reconfigure::Consent {
            settled: true,
            rehearsing: false,
        },
    );

    let weighed = assessed(&ctx, proposed, &file, (DATA_ROOT_KEY, "/srv/new"), true).await;

    assert_eq!(weighed.stance, crate::reconfigure::Stance::Applied);
    assert!(!weighed.findings.any());
}

#[test]
fn a_library_that_would_be_lost_is_refused_before_anything_else_and_takes_no_yes() {
    // Ordered first because it is nobody's to override: what is still coming down
    // and a hand-edit underneath are the operator's own calls, and this is not.
    let found = Findings {
        library: vec![crate::reconfigure::LibraryPath {
            service: "Radarr".to_owned(),
            path: "/data/media/movies".to_owned(),
            host: Some("/srv/new/media/movies".to_owned()),
            carried: false,
            because: "not there".to_owned(),
        }],
        active: vec![crate::reconfigure::Active {
            protocol: "torrent".to_owned(),
            name: "Ubuntu.iso".to_owned(),
            progress: 94,
        }],
        ..Findings::default()
    };
    let said = refusal(&found, None, true).unwrap_or_default();
    assert!(said.contains("/data/media/movies"), "{said}");
    assert!(!said.contains("--confirm"), "{said}");
}

#[test]
fn what_could_not_be_read_is_said_ahead_of_what_was() {
    // A library nothing would speak for is the sharper answer: the paths that were
    // read say what happens to them, and this says nobody could say.
    let found = Findings::default();
    assert_eq!(
        refusal(&found, Some("nothing would say".to_owned()), true).as_deref(),
        Some("nothing would say")
    );
}

#[test]
fn work_still_coming_down_is_counted_and_says_both_ways_out() {
    let one = |name: &str| crate::reconfigure::Active {
        protocol: "torrent".to_owned(),
        name: name.to_owned(),
        progress: 10,
    };
    let found = Findings {
        active: vec![one("a"), one("b")],
        ..Findings::default()
    };
    let said = refusal(&found, None, false).unwrap_or_default();
    assert!(said.contains("2 downloads still coming down"), "{said}");
    assert!(said.contains("--wait"), "{said}");
    assert!(said.contains("--confirm"), "{said}");
    assert_eq!(refusal(&found, None, true), None);
}

#[test]
fn a_change_that_found_nothing_is_refused_for_nothing() {
    assert_eq!(refusal(&Findings::default(), None, false), None);
}

#[test]
fn an_edit_underneath_says_how_to_write_over_it_and_agreeing_clears_it() {
    // The refusal is the operator's to override, so it names the word that does
    // it — a refusal with no way past it is what people go and edit the file to
    // get around.
    let found = Findings {
        edited: Some(Edited {
            wrote: "/srv/old".to_owned(),
            found: "/mnt/theirs".to_owned(),
            secret: false,
        }),
        ..Findings::default()
    };
    let said = refusal(&found, None, false).unwrap_or_default();
    assert!(said.contains("--confirm"), "{said}");
    assert_eq!(refusal(&found, None, true), None);
}

#[test]
fn a_machine_with_nowhere_to_keep_a_record_keeps_none() {
    let ctx = a_context().build();
    record(&ctx, DATA_ROOT_KEY, "/srv/old");
    assert!(recorded(&ctx).is_none());
}

#[test]
fn a_record_that_will_not_parse_is_read_as_none_and_left_where_it_is() {
    // The same line seeding holds. A record that may be there but unreadable
    // cannot tell an edit from lemonfiber's own value, and re-forming it around
    // whatever the file happens to hold would silently throw away whatever it
    // was about to say.
    let env = env_without_password("lost");
    let path = env.with_file_name("baseline.json");
    let _ = std::fs::write(&path, "not json at all");
    let ctx = over(env);

    assert!(recorded(&ctx).is_none());
    record(&ctx, DATA_ROOT_KEY, "/srv/old");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap_or_default(),
        "not json at all"
    );
}
