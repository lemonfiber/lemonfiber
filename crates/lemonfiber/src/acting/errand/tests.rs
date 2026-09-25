use super::{all, every, Accepts, Errand, Given, Going, Needs, Outcome, Stage, KEY, LINES};
use crate::acting::offer::OFFERED as KEYED;
use lemonfiber::reaching::{ACTS, ALSO};
use lemonfiber_api::actions::{OFFERED as WEB, TAKES_AGREEMENT, TAKES_CONSENT};
use lemonfiber_core::app::restore::{Consent, Kept};
use lemonfiber_core::app::{Command, Waiting};
use lemonfiber_core::bundle::Filenames;

/// One errand naming an action no surface offers, for the paths that report a
/// translation that came to nothing.
pub(crate) static UNTRANSLATABLE: Errand = Errand {
    name: "an errand nothing answers",
    about: "for the refusal a translation that reaches no command produces",
    action: "not an action any surface offers",
    asks: "Do the impossible",
    needs: Needs::Nothing,
    accepts: None,
    going: Going::Once,
};

/// An errand given a name typed on a line, which is how the two that take one
/// were given theirs before either of them existed.
fn typed(name: &str) -> Given {
    Given::typed(name.to_owned())
}

/// The errand one action is on, for a test that wants a particular one.
pub(crate) fn sending(action: &str) -> Option<&'static Errand> {
    every().find(|errand| errand.action == action)
}

/// What one action is called on the list, for the screen's own tests, which
/// move to an errand by the name an operator would read rather than by number.
pub(crate) fn listed(action: &str) -> String {
    every()
        .filter(|errand| errand.action == action)
        .map(|errand| errand.name)
        .collect()
}

/// Inviting somebody opens a line to type their name on, and the word goes on to
/// what they may watch rather than straight to the question.
///
/// Driven through the stage machinery rather than by building a `Given` by hand,
/// because the two halves are decided in different places — that there is a line
/// at all, and what the word opens next — and a test that skipped the first would
/// pass with no line ever opening. What becomes of the two answers after it is
/// [`super::super::inviting`]'s, and is held there.
#[test]
fn inviting_somebody_opens_a_line_and_the_word_carries_their_name_on() {
    // Every errand, filtered to the one under test: a `let ... else` here would
    // leave a branch nothing reaches, which the gate counts as untested code.
    let errand = every()
        .find(|errand| errand.action == "invite")
        .unwrap_or(all().0);
    let mut stage = Stage::Idle;

    let _ = super::taken(&mut stage, errand, &[]);

    assert!(
        matches!(&stage, Stage::Naming { asks, .. } if asks.contains("Who it is for")),
        "no line was opened to type a name on"
    );

    let _ = super::given(&mut stage, errand, "ana".to_owned());

    assert!(
        matches!(&stage, Stage::Allowing { name, typed, .. }
            if name == "ana" && typed.is_empty()),
        "the name did not carry on to what they may watch"
    );
}

/// An errand that wants only a name opens the same line and sends on the word.
///
/// The invitation above asks for three things and moves to the second, so it no
/// longer reaches the case where the word typed is the whole of what was wanted.
/// A reissue does, and so does a removal — one line, one word, and the errand
/// sent as soon as it is answered.
#[test]
fn an_errand_that_wants_only_a_name_sends_it_as_soon_as_it_is_typed() {
    let errand = every()
        .find(|errand| errand.action == "reissue")
        .unwrap_or(all().0);
    let mut stage = Stage::Idle;

    let _ = super::taken(&mut stage, errand, &[]);

    assert!(
        matches!(&stage, Stage::Naming { asks, .. } if asks.contains("Whose account")),
        "no line was opened to type a name on"
    );

    let _ = super::given(&mut stage, errand, "ana".to_owned());

    assert!(
        matches!(&stage, Stage::Agreeing { given, .. }
            if given.asked().name.as_deref() == Some("ana")),
        "the word typed did not reach the question that sends it"
    );
}

/// The whole point of naming the action rather than assembling a command here:
/// what this screen sends has to be something another surface already offers,
/// or the requirement it is being built for is defeated by the thing built for
/// it.
#[test]
fn every_errand_this_screen_sends_is_one_the_other_surfaces_offer() {
    let missing: Vec<&str> = every()
        .map(|errand| errand.action)
        .filter(|action| !WEB.contains(action))
        .collect();

    assert!(missing.is_empty(), "{missing:?}");
}

/// An errand is named once, or the second is unreachable on a list that shows
/// both and nobody would know which they took.
#[test]
fn no_two_errands_go_by_the_same_name() {
    for errand in every() {
        let same = every().filter(|other| other.name == errand.name).count();
        assert_eq!(same, 1, "more than one errand is called {}", errand.name);
    }
    assert!(every().all(|errand| !errand.about.is_empty()));
}

/// The key this list opens on is not one the screen already answers, or the
/// thing it already did stops happening and nothing says so.
#[test]
fn the_key_that_opens_them_is_not_one_the_screen_already_answers() {
    for taken in ['q', 'r', '?', crate::acting::question::KEY] {
        assert_ne!(KEY, taken, "{taken:?} was already spoken for");
    }
    assert!(KEYED.iter().all(|offer| offer.key != KEY));
}

/// The one thing the screen's actions are checked against from outside the
/// binary, and the whole of what it offers rather than this file's share of it —
/// the projection is one list, so only one place can hold it to be exactly what
/// the screen offers. An action offered anywhere with no entry there leaves the
/// parity table's terminal column claiming less than the screen does, and an
/// entry there naming an action nothing offers leaves it claiming more.
///
/// Six places offer one: the keys, this list, the two that keep going, the three
/// quality changes, the two that answer a diagnosis, and the two widened reads,
/// which are on no key at all because each is offered under the answer that named
/// the gap.
///
/// The published names come off two lists because the requests do. Every other
/// action is the only way its request is reached; the three quality writes and
/// the two widened reads act on requests the screen already reaches as reads, so
/// they are published beside the rest rather than among them — and both lists are
/// read here, or a write could be added to the screen and excused by the list it
/// was not on.
#[test]
fn every_action_this_screen_offers_is_published_for_the_parity_table() {
    let mut offered: Vec<&str> = KEYED
        .iter()
        .map(|offer| offer.action)
        .chain(every().map(|errand| errand.action))
        .chain(crate::acting::lasting::every().map(|lasting| lasting.action))
        .chain(crate::acting::quality::every().map(|change| change.action))
        .chain(crate::acting::mending::every().map(|mending| mending.action))
        .chain(crate::acting::disturbing::every().map(|widened| widened.action))
        .collect();
    let mut published: Vec<&str> = ACTS.iter().chain(ALSO).map(|reach| reach.through).collect();
    offered.sort_unstable();
    published.sort_unstable();

    assert_eq!(offered, published);
}

/// Which errands say what they would do first is asked of the tables that say
/// which actions carry an answer, rather than being decided again here — a second
/// list would come to disagree with the first, and the place it would disagree is
/// in front of somebody about to throw work away.
///
/// Two tables rather than one, because there are two shapes of answer and both of
/// them are answers to something read. Most carry a flag, and the one that lets a
/// download go carries the name the offer gave itself — which is the stronger of
/// the two and, on that action, the only one there is.
#[test]
fn an_errand_whose_action_takes_an_answer_says_what_it_would_do_first() {
    for errand in every() {
        let takes =
            TAKES_AGREEMENT.contains(&errand.action) || TAKES_CONSENT.contains(&errand.action);
        let says = errand.would(&typed("a-backup.tar.gz")).is_some();
        assert_eq!(says, takes, "{}", errand.name);
    }
}

/// No bundle this screen can send names a setting to show as it is.
///
/// Over every answer the two lines in front of a bundle can be given rather than
/// over the one nobody typed at, because a guard that only read the default would
/// pass on a screen that had grown a third line. The agreement beside it is safe
/// to carry precisely because this holds: an agreement to publish nothing is what
/// every one of these carries.
#[test]
fn no_bundle_this_screen_can_send_names_a_setting_to_reveal() {
    let bundling = [
        (Given::nothing(), LINES, Filenames::Replaced),
        (
            Given::bundled(LINES, Filenames::Replaced),
            LINES,
            Filenames::Replaced,
        ),
        (Given::bundled(1, Filenames::Shown), 1, Filenames::Shown),
        (
            Given::bundled(999_999_999, Filenames::Shown),
            999_999_999,
            Filenames::Shown,
        ),
    ];

    for (given, lines, filenames) in &bundling {
        assert_eq!(
            sending("support").map(|errand| errand.sent(given)),
            Some(Ok(Command::Support {
                write: true,
                wanted: lemonfiber_core::bundle::run::Wanted {
                    lines: *lines,
                    filenames: *filenames,
                    reveal: Vec::new(),
                    confirmed: true,
                },
                dest: lemonfiber_core::app::support::Destination::Kept,
            }))
        );
    }
}

/// A restore that came back as something other than a restoration calls for no
/// re-point.
///
/// A screen that read one out of the wrong shape would be putting an operator's
/// agreement to a move on the strength of an answer about something else — and
/// the shape it would have read it out of is whatever the core answered.
#[test]
fn an_answer_that_is_not_a_restoration_calls_for_no_re_point() {
    let restoring = sending("restore");

    let accepting = restoring.and_then(|errand| errand.accepting(&Outcome::Version(a_version())));

    assert_eq!(accepting, None);
}

/// An update's own account of itself, with `coming` still on the way down.
fn an_update(coming: &[&str]) -> Outcome {
    Outcome::Update(lemonfiber_core::update::run::Report {
        state: lemonfiber_core::update::State::UpdatesAvailable,
        changes: Vec::new(),
        in_flight: coming.iter().map(|one| (*one).to_owned()).collect(),
        confirmed: false,
        backup: None,
        stack_edits: Vec::new(),
        applied: Vec::new(),
        halted: None,
        changelog: lemonfiber_core::changelog::Notes::unread(),
    })
}

/// Where an update's question stands once the run in front of it has been read.
fn after(outcome: &Outcome) -> Option<Stage> {
    let errand = sending("update")?;
    Some(super::weighed(
        errand,
        Given::nothing(),
        outcome,
        Vec::new(),
    ))
}

/// An update with nothing coming down calls for nothing further, so the question
/// under its account is the plain one.
#[test]
fn an_update_with_nothing_coming_down_asks_nobody_to_wait_for_it() {
    let errand = sending("update");

    let accepting = errand.and_then(|errand| errand.accepting(&an_update(&[])));

    assert_eq!(accepting, None);
    let plain = after(&an_update(&[]));
    assert!(
        matches!(&plain, Some(Stage::Agreeing { given, .. })
            if given.asked().wait == Waiting::Never),
        "a run with nothing to wait for offered the wait anyway"
    );
}

/// And one that would interrupt a transfer offers the wait, in the sentence the
/// yes is given to — which is the only way this screen can reach it.
#[test]
fn an_update_that_would_interrupt_a_transfer_carries_the_wait_it_was_agreed_to_with() {
    let errand = sending("update");

    let accepting = errand.and_then(|errand| errand.accepting(&an_update(&["Ubuntu.iso"])));

    assert!(
        matches!(accepting, Some((Accepts::Wait, _))),
        "{accepting:?}"
    );
    let carried = after(&an_update(&["Ubuntu.iso"]));
    assert!(
        matches!(&carried, Some(Stage::Agreeing { given, .. })
            if given.asked().wait == Waiting::ForTheDownloads
                && given.said().contains("finish")),
        "the wait was not carried into what the yes sends"
    );
}

/// An offer over one seeding download, as the core would answer with one.
fn an_offer() -> lemonfiber_core::space::Letting {
    lemonfiber_core::space::letting::offering(lemonfiber_core::space::Candidate {
        name: "A.Show.S01E01".to_owned(),
        bytes: 8_000,
        standing: lemonfiber_core::space::Standing::Seeding { ratio: 175 },
        consequence: Some(lemonfiber_core::space::RATIO_CONSEQUENCE.to_owned()),
    })
}

/// Letting a download go opens a line, and the word fills the download rather
/// than any of the other things a line on this screen can be typed for.
///
/// Driven through the stage machinery rather than by building a `Given` by hand,
/// because that there is a line at all and what the word fills are decided in
/// different places, and a test that skipped the first would pass with no line
/// ever opening.
#[test]
fn letting_a_download_go_opens_a_line_and_the_word_names_the_download() {
    let errand = every()
        .find(|errand| errand.action == "stop-seeding")
        .unwrap_or(all().0);
    let mut stage = Stage::Idle;

    let _ = super::taken(&mut stage, errand, &[]);

    assert!(
        matches!(&stage, Stage::Naming { asks, .. } if asks.contains("Which completed download")),
        "no line was opened to name a download on"
    );

    let _ = super::given(&mut stage, errand, "A.Show.S01E01".to_owned());

    assert!(
        matches!(&stage, Stage::Weighing { .. }),
        "the word did not carry on to the run that says what it would cost"
    );
}

/// The yes is the name the offer gave itself, carried off the answer rather than
/// typed — and it reaches the command as the agreement.
///
/// The whole of what makes this errand safe. A screen that sent it without the
/// name would be asking the core to remove a torrent on a yes nobody could have
/// read a consequence for, and the core refuses that — so the failure would be a
/// dead end rather than a wrong removal, and this is what keeps it from being one.
#[test]
fn the_yes_to_letting_a_download_go_is_the_name_the_offer_gave_itself() {
    let errand = every()
        .find(|errand| errand.action == "stop-seeding")
        .unwrap_or(all().0);
    let offer = an_offer();

    let stage = super::weighed(
        errand,
        Given::downloaded("A.Show.S01E01".to_owned()),
        &Outcome::Letting(offer.clone()),
        vec!["what it costs".to_owned()],
    );
    // Read out of the stage rather than matched with an arm for the case that
    // cannot happen: an arm no run reaches is a line the coverage gate counts
    // against a file every one of whose cases passed.
    let mut answered = None;
    if let Stage::Agreeing { errand, given, .. } = &stage {
        answered = Some(errand.sent(given));
    }

    assert_eq!(
        answered,
        Some(Ok(Command::StopSeeding {
            download: "A.Show.S01E01".to_owned(),
            agreement: Some(offer.agreement),
        })),
        "the offer that was read is what the yes names"
    );
}

/// An answer of another shape carries no offer to answer.
///
/// The same rule the re-point above it follows, for the same reason: a name read
/// out of the wrong answer would be an agreement to a reading nobody made.
#[test]
fn an_answer_that_is_not_an_offer_carries_no_name_to_answer_it_with() {
    let stopping = every().find(|errand| errand.action == "stop-seeding");

    assert_eq!(
        stopping.and_then(|errand| errand.answering(&Outcome::Version(a_version()))),
        None
    );
    // And an errand whose yes is a flag takes no name off one either, however
    // right the shape of the answer looks.
    assert_eq!(
        sending("space").and_then(|errand| errand.answering(&Outcome::Letting(an_offer()))),
        None
    );
}

/// A version report, which is an answer of a shape no errand ever has.
fn a_version() -> lemonfiber_core::model::VersionReport {
    lemonfiber_core::model::VersionReport {
        binary: "0.9.0".to_owned(),
        supported_schema: vec![1],
        stack: "1.2.3".to_owned(),
        compose: None,
        changelog: lemonfiber_core::changelog::Notes::unread(),
    }
}

/// Asked for with nothing chosen it is asked for with every careful default, and
/// the yes carries the agreement the command names.
///
/// The agreement is the half that was missing rather than the half that was
/// wrong: nothing about the file differs, because the reveal is empty either way
/// — which is exactly why nothing caught it.
#[test]
fn the_bundle_this_screen_asks_for_replaces_filenames_and_carries_the_agreement() {
    let bundle = sending("support").map(|errand| errand.sent(&Given::nothing()));

    assert_eq!(
        bundle,
        Some(Ok(Command::Support {
            write: true,
            wanted: lemonfiber_core::bundle::run::Wanted {
                confirmed: true,
                ..lemonfiber_core::bundle::run::Wanted::default()
            },
            dest: lemonfiber_core::app::support::Destination::Kept,
        }))
    );
}

/// What was chosen in front of the question is what the bundle is asked for.
///
/// The window and the filenames both, because either carried alone would be a
/// screen that asked two things and sent one.
#[test]
fn a_bundle_is_asked_for_with_the_window_and_the_filenames_that_were_chosen() {
    let bundle =
        sending("support").map(|errand| errand.sent(&Given::bundled(20, Filenames::Shown)));

    assert_eq!(
        bundle,
        Some(Ok(Command::Support {
            write: true,
            wanted: lemonfiber_core::bundle::run::Wanted {
                lines: 20,
                filenames: Filenames::Shown,
                reveal: Vec::new(),
                confirmed: true,
            },
            dest: lemonfiber_core::app::support::Destination::Kept,
        }))
    );
}

/// What a bundle would hold is asked for before it is written, and asking costs
/// nothing: the run that says is the run that produces no file.
#[test]
fn what_a_bundle_would_hold_is_asked_for_before_one_is_written() {
    let would = sending("support").and_then(|errand| errand.would(&Given::nothing()));

    assert!(matches!(
        would,
        Some(Ok(Command::Support { write: false, .. }))
    ));
}

/// A restore is asked for by the name a backup was written under, carried as a
/// name and never as a path — so a name holding one reaches the core's own
/// refusal rather than the file it points at.
#[test]
fn a_restore_carries_the_name_it_was_given_and_never_a_path() {
    let restoring = sending("restore");

    let climbing = restoring.map(|errand| errand.sent(&typed("../../etc/shadow")));
    assert_eq!(
        climbing,
        Some(Ok(Command::Restore {
            archive: Kept::Named("../../etc/shadow".to_owned()),
            repoint: false,
            consent: Consent::Standing,
        }))
    );
    let listing = restoring.and_then(|errand| errand.would(&typed("lemonfiber-full-1.tar.gz")));
    assert!(matches!(
        listing,
        Some(Ok(Command::Restore {
            consent: Consent::List,
            ..
        }))
    ));
}

/// A restore with nothing typed is refused in the sentence the web surface
/// gives for the same request, rather than in one this screen wrote.
#[test]
fn a_restore_with_no_name_is_refused_in_the_words_the_other_surface_gives() {
    let said = sending("restore")
        .and_then(|errand| errand.would(&typed("")).and_then(Result::err))
        .unwrap_or_default();

    assert!(said.contains("restore"), "{said}");
    assert!(said.contains("archive"), "{said}");
}

/// A reset is the agreement and nothing else: the run before it reverts nothing
/// and only names what would be lost.
#[test]
fn a_reset_reverts_nothing_until_it_is_agreed_to() {
    let errand = sending("reset");

    assert_eq!(
        errand.and_then(|errand| errand.would(&Given::nothing())),
        Some(Ok(Command::Reset { confirm: false }))
    );
    assert_eq!(
        errand.map(|errand| errand.sent(&Given::nothing())),
        Some(Ok(Command::Reset { confirm: true }))
    );
}

/// The four that carry no agreement are sent as they stand, and each reaches
/// the command every other surface produces for the same request.
///
/// Putting the last repair back is one of them, and it carries no subject either:
/// which repair was last and what reversing it takes are the core's, so there is
/// nothing here for this screen to name.
#[test]
fn an_errand_carrying_no_agreement_is_sent_as_it_stands() {
    let sent: Vec<Result<Command, String>> = ["seed", "adopt", "backup", "undo"]
        .into_iter()
        .filter_map(sending)
        .map(|errand| errand.sent(&Given::nothing()))
        .collect();

    assert_eq!(
        sent,
        vec![
            Ok(Command::Seed),
            Ok(Command::Adopt),
            Ok(Command::Backup { service: None }),
            Ok(Command::Undo { run: None }),
        ]
    );
}

/// The list opens on the first errand and holds every one of them.
#[test]
fn the_list_opens_on_the_first_errand_and_holds_them_all() {
    let (first, rest) = all();

    assert_eq!(first.action, "seed");
    assert_eq!(rest.len() + 1, every().count());
    assert!(matches!(first.needs, Needs::Nothing));
}
