/// A title with more room than anything here is testing the edge of.
const WIDE: usize = 200;

use super::{cleared, colours, sampled, wanted, Asked, Press, Shown, Viewer, BATCH};
use lemonfiber_core::bundle::Marks;
use lemonfiber_core::logs::Level;
use lemonfiber_core::ports::docker::{Lifecycle, LogLine, Stream};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// One keypress as this view reads it.
fn read(code: KeyCode, modifiers: KeyModifiers) -> Option<Press> {
    wanted(KeyEvent::new(code, modifiers))
}

/// Every key this view answers arrives as something it can act on, and one it
/// has no use for arrives as nothing. Ctrl-C backs out rather than typing a
/// character, because raw mode no longer turns it into a signal.
#[test]
fn the_keyboard_reaches_this_view_as_the_presses_it_answers() {
    for (code, press) in [
        (KeyCode::Char('f'), Press::Typed('f')),
        (KeyCode::Backspace, Press::Rubout),
        (KeyCode::Enter, Press::Accept),
        (KeyCode::Esc, Press::Abandon),
        (KeyCode::Up, Press::Back),
        (KeyCode::Down, Press::Forward),
        (KeyCode::End, Press::Tail),
    ] {
        assert_eq!(read(code, KeyModifiers::NONE), Some(press), "{code:?}");
    }
    assert_eq!(
        read(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(Press::Abandon)
    );
    assert_eq!(read(KeyCode::Home, KeyModifiers::NONE), None);
    // A character held with control is not that character: ctrl-e is not a
    // filter somebody began typing.
    assert_eq!(read(KeyCode::Char('e'), KeyModifiers::CONTROL), None);
}

/// One line as the engine hands it over.
fn line(service: &str, said: &str) -> LogLine {
    LogLine {
        service: service.to_owned(),
        stream: Stream::Stdout,
        at: None,
        line: said.to_owned(),
    }
}

/// A viewer holding everything, already fed these lines.
fn fed(said: &[(&str, &str)]) -> Viewer {
    let mut viewer = Viewer::opened();
    for (service, text) in said {
        viewer.take(line(service, text));
    }
    viewer
}

/// Two services' worth of lines, for the tests that need more than one source.
fn a_viewer() -> Viewer {
    fed(&[
        ("sonarr", "INFO Grabbed an episode"),
        ("radarr", "WARN Import timed out"),
        ("sonarr", "Torrent finished"),
    ])
}

/// What the screen is showing, as the words of each line.
fn shown(viewer: &Viewer, rows: usize) -> Vec<String> {
    viewer
        .showing(rows)
        .into_iter()
        .map(|shown| shown.said)
        .collect()
}

/// Opening the words is the asking. Closing them again is not, and neither is
/// a key pressed on a run that explains nothing.
#[test]
fn opening_the_words_asks_and_closing_them_does_not() {
    let mut viewer = a_viewer();

    assert_eq!(viewer.pressed(Press::Typed('?')), Asked::Learned);
    assert_eq!(viewer.pressed(Press::Typed('?')), Asked::Nothing);
}

#[test]
fn a_run_that_explains_nothing_asks_nothing_either() {
    let mut viewer = Viewer::opened().without_explanations();

    assert_eq!(viewer.pressed(Press::Typed('?')), Asked::Nothing);
}

/// What the loop records is what was behind the pane.
#[test]
fn the_words_on_screen_are_the_ones_showing() {
    let viewer = fed(&[("sonarr", "no indexer answered in time")]);

    let said = viewer.showing_words(10);

    assert!(said.contains("indexer"), "{said}");
    assert!(said.contains("sonarr"), "and which service said it: {said}");
}

/// Press each of these in turn.
fn press(viewer: &mut Viewer, presses: &[Press]) {
    for asked in presses {
        viewer.pressed(*asked);
    }
}

/// Randomness a test chose, so an export reads the same on every run.
///
/// Written here rather than borrowed from the fixtures crate, which this binary
/// does not depend on: the port has one method, and this calls it.
struct Chosen;

impl lemonfiber_core::ports::random::Random for Chosen {
    fn bytes(&self, count: usize) -> Option<Vec<u8>> {
        Some(vec![7; count])
    }
}

/// The view, redacted — empty where the chosen randomness somehow refused, which
/// every caller rules out by asserting on what it got back.
fn exported(viewer: &Viewer) -> String {
    Marks::new(&Chosen)
        .and_then(|marks| viewer.exported(&marks).ok())
        .unwrap_or_default()
}

/// Type a search and apply it.
fn search(viewer: &mut Viewer, text: &str) {
    viewer.pressed(Press::Typed('/'));
    for character in text.chars() {
        viewer.pressed(Press::Typed(character));
    }
    viewer.pressed(Press::Accept);
}

/// The trade the screen makes under a flood, and the one it does not make when
/// there is no flood to make it about.
#[test]
fn a_small_backlog_is_taken_whole_and_a_flood_only_in_part() {
    assert_eq!(sampled(0), (0, 0));
    assert_eq!(sampled(10), (10, 0));
    assert_eq!(sampled(BATCH), (BATCH, 0));
    assert_eq!(sampled(BATCH + 7), (BATCH, 7));
}

#[test]
fn a_new_viewer_is_open_at_the_tail_with_nothing_in_it() {
    let viewer = Viewer::opened();

    assert!(viewer.open());
    assert_eq!(viewer.typing(), None);
    assert_eq!(viewer.heading(WIDE), "waiting for lines");
    assert_eq!(viewer.footing(), "following");
}

#[test]
fn each_service_is_named_once_however_many_lines_it_writes() {
    assert_eq!(a_viewer().heading(WIDE), "sonarr, radarr");
}

/// A title is one row and cannot be given a second, so it names as many services
/// as fit whole and counts the rest. A name cut in half says which service no
/// better than an absent one, and it costs the same room to say so.
#[test]
fn a_title_narrower_than_the_names_counts_the_ones_it_leaves_out() {
    let mut viewer = Viewer::opened();
    for service in [
        "sonarr",
        "radarr",
        "calibre-web-automated",
        "audiobookshelf",
    ] {
        viewer.take(line(service, "up"));
    }

    assert_eq!(
        viewer.heading(WIDE),
        "sonarr, radarr, calibre-web-automated, audiobookshelf",
        "with room for all of them it names all of them"
    );
    assert_eq!(viewer.heading(38), "sonarr, radarr, +2 more");
    assert_eq!(
        viewer.heading(12),
        "4 services",
        "where not one name fits, the count stands on its own"
    );
}

/// The names in the title are a container's, and the title is drawn by the box's
/// border rather than by the rows inside it.
///
/// The rows are made plain where they are built, which left the title as the one run
/// of somebody else's text on this screen that was not — and a name is exactly where
/// an override goes, because it is the short run an operator reads rather than scans.
#[test]
fn a_container_cannot_name_itself_something_the_title_would_obey() {
    let viewer = fed(&[("son\u{1b}arr", "up"), ("rad\u{202e}arr", "up")]);
    let heading = viewer.heading(80);
    assert_eq!(heading, "sonarr, radarr", "{heading:?}");
}

/// How far behind the screen is survives however narrow the title gets: it is
/// the half that changes, it sits at the end where a cut takes it first, and a
/// tail that stopped saying it has stopped being one.
#[test]
fn a_narrow_title_gives_up_names_before_it_gives_up_the_count() {
    let mut viewer = Viewer::opened();
    for service in ["sonarr", "radarr", "calibre-web-automated"] {
        viewer.take(line(service, "up"));
    }
    viewer.pressed(Press::Back);
    viewer.take(line("sonarr", "and another"));

    let heading = viewer.heading(30);
    assert!(heading.contains("1 unseen"), "{heading}");
    assert!(heading.chars().count() <= 30, "{heading}");
}

/// What the screen shows about one line: who said it, how bad they said it was,
/// and the words themselves.
#[test]
fn a_shown_line_carries_its_source_its_severity_and_its_words() {
    let viewer = fed(&[("radarr", "WARN Import timed out")]);

    assert_eq!(
        viewer.showing(1),
        vec![Shown {
            service: "radarr".to_owned(),
            level: Some(Level::Warn),
            said: "WARN Import timed out".to_owned(),
        }]
    );
}

/// A log line is text from somebody else's container, and a terminal is not a
/// text box.
#[test]
fn a_line_that_would_drive_the_terminal_is_shown_without_the_instructions() {
    let viewer = fed(&[("sonarr", "INFO \u{1b}[2Jand your screen is gone")]);
    let said = shown(&viewer, 1).concat();

    assert!(!said.contains('\u{1b}'), "{said}");
    assert!(said.contains("and your screen is gone"), "{said}");
}

#[test]
fn quitting_and_escaping_both_leave() {
    let mut quit = a_viewer();
    quit.pressed(Press::Typed('q'));
    assert!(!quit.open());

    let mut escaped = a_viewer();
    escaped.pressed(Press::Abandon);
    assert!(!escaped.open());
}

#[test]
fn keys_the_screen_has_no_use_for_change_nothing() {
    let mut viewer = a_viewer();
    press(
        &mut viewer,
        &[Press::Typed('x'), Press::Accept, Press::Rubout],
    );

    assert!(viewer.open());
    assert_eq!(viewer.footing(), "following");
    assert_eq!(shown(&viewer, 10).len(), 3);
}

/// The whole reason a typing mode exists: an operator searching for `queue`
/// must not have the `q` close the screen out from under them.
#[test]
fn a_search_is_typed_rubbed_out_and_applied_without_the_keys_acting() {
    let mut viewer = a_viewer();
    press(
        &mut viewer,
        &[
            Press::Typed('/'),
            Press::Typed('q'),
            Press::Typed('t'),
            Press::Typed('i'),
            Press::Typed('m'),
        ],
    );
    assert!(viewer.open(), "the q was text, not a command");
    assert_eq!(viewer.typing(), Some("qtim"));

    viewer.pressed(Press::Rubout);
    viewer.pressed(Press::Rubout);
    viewer.pressed(Press::Rubout);
    viewer.pressed(Press::Rubout);
    assert_eq!(viewer.typing(), Some(""));

    press(&mut viewer, &[Press::Typed('t'), Press::Typed('i')]);
    viewer.pressed(Press::Accept);

    assert_eq!(viewer.typing(), None);
    assert_eq!(shown(&viewer, 10), ["WARN Import timed out"]);
    let footing = viewer.footing();
    assert!(footing.contains("/ti"), "{footing}");
}

#[test]
fn a_search_typed_and_left_empty_asks_for_everything() {
    let mut viewer = a_viewer();
    search(&mut viewer, "");

    assert_eq!(viewer.footing(), "following");
    assert_eq!(shown(&viewer, 10).len(), 3);
}

/// Giving up on a search puts the operator back where they were, rather than
/// clearing the filter they had before they started typing a new one.
#[test]
fn abandoning_a_search_keeps_the_one_that_was_in_force() {
    let mut viewer = a_viewer();
    search(&mut viewer, "timed");

    press(&mut viewer, &[Press::Typed('/'), Press::Typed('z')]);
    viewer.pressed(Press::Abandon);

    assert_eq!(viewer.typing(), None);
    assert!(viewer.open(), "the escape left the search, not the screen");
    assert_eq!(shown(&viewer, 10), ["WARN Import timed out"]);
}

#[test]
fn scrolling_while_a_search_is_typed_leaves_the_search_alone() {
    let mut viewer = a_viewer();
    press(&mut viewer, &[Press::Typed('/'), Press::Typed('t')]);

    press(&mut viewer, &[Press::Back, Press::Forward, Press::Tail]);

    assert_eq!(viewer.typing(), Some("t"));
    assert_eq!(viewer.footing(), "following", "and nothing scrolled");
}

#[test]
fn the_severity_asked_for_cycles_up_and_back_to_everything() {
    let mut viewer = a_viewer();

    for expected in ["info+", "warn+", "error+"] {
        viewer.pressed(Press::Typed('w'));
        assert_eq!(viewer.footing(), expected);
    }
    viewer.pressed(Press::Typed('w'));
    assert_eq!(viewer.footing(), "following");
}

#[test]
fn the_service_shown_cycles_through_them_and_back_to_all() {
    let mut viewer = a_viewer();

    viewer.pressed(Press::Typed('s'));
    assert_eq!(viewer.heading(WIDE), "sonarr");
    assert_eq!(shown(&viewer, 10).len(), 2);

    viewer.pressed(Press::Typed('s'));
    assert_eq!(viewer.heading(WIDE), "radarr");

    viewer.pressed(Press::Typed('s'));
    assert_eq!(viewer.heading(WIDE), "sonarr, radarr");
}

#[test]
fn cycling_services_before_any_line_arrives_stays_on_all_of_them() {
    let mut viewer = Viewer::opened();
    viewer.pressed(Press::Typed('s'));

    assert_eq!(viewer.heading(WIDE), "waiting for lines");
}

#[test]
fn clearing_puts_every_filter_back_at_once() {
    let mut viewer = a_viewer();
    search(&mut viewer, "timed");
    press(&mut viewer, &[Press::Typed('w'), Press::Typed('s')]);

    viewer.pressed(Press::Typed('c'));

    assert_eq!(viewer.footing(), "following");
    assert_eq!(viewer.heading(WIDE), "sonarr, radarr");
    assert_eq!(shown(&viewer, 10).len(), 3);
}

/// Scrolling back detaches, stops at the oldest line rather than running off
/// the end of it, and returning to the tail attaches again.
#[test]
fn scrolling_back_detaches_stops_at_the_oldest_and_comes_back() {
    let mut viewer = a_viewer();

    press(&mut viewer, &[Press::Back, Press::Back, Press::Back]);
    assert_eq!(viewer.footing(), "detached");
    assert_eq!(
        shown(&viewer, 1),
        ["INFO Grabbed an episode"],
        "three presses over three lines stop at the oldest"
    );

    viewer.pressed(Press::Forward);
    assert_eq!(viewer.footing(), "detached");

    viewer.pressed(Press::Forward);
    assert_eq!(viewer.footing(), "following");
}

#[test]
fn following_and_the_end_key_both_return_to_the_tail() {
    for key in [Press::Typed('f'), Press::Tail] {
        let mut viewer = a_viewer();
        viewer.pressed(Press::Back);
        assert_eq!(viewer.footing(), "detached");

        viewer.pressed(key);
        assert_eq!(viewer.footing(), "following");
    }
}

/// The point of counting the offset from the newest line: what the operator is
/// reading stays where they left it while the stack keeps writing.
#[test]
fn a_line_arriving_while_detached_keeps_the_operators_place() {
    let mut viewer = fed(&[
        ("sonarr", "alpha timed"),
        ("sonarr", "beta timed"),
        ("sonarr", "gamma timed"),
    ]);
    search(&mut viewer, "timed");
    viewer.pressed(Press::Back);
    assert_eq!(shown(&viewer, 1), ["beta timed"]);

    // One the filter hides moves nothing, because it changes nothing on screen.
    viewer.take(line("sonarr", "delta hidden"));
    assert_eq!(shown(&viewer, 1), ["beta timed"]);

    // One it admits would push the view up by a line, so the offset grows with it.
    viewer.take(line("sonarr", "epsilon timed"));
    assert_eq!(shown(&viewer, 1), ["beta timed"]);

    let heading = viewer.heading(WIDE);
    assert!(heading.contains("2 unseen"), "{heading}");
}

#[test]
fn the_screen_shows_what_fits_ending_at_the_newest_line() {
    let viewer = a_viewer();

    assert_eq!(
        shown(&viewer, 2),
        ["WARN Import timed out", "Torrent finished"]
    );
    assert_eq!(shown(&viewer, 10).len(), 3, "more room than lines is fine");
    assert!(shown(&viewer, 0).is_empty());
}

/// An empty screen that does not say how much was looked at reads the same
/// whether the filter is too narrow or nothing has arrived at all.
#[test]
fn a_filter_matching_nothing_is_stated_with_how_much_was_scanned() {
    let mut viewer = a_viewer();
    assert_eq!(viewer.nothing(), None);

    search(&mut viewer, "nothing says this");

    assert_eq!(
        viewer.nothing(),
        Some("nothing matches — 3 lines scanned".to_owned())
    );
}

/// The two kinds of loss are said apart because they call for different answers:
/// a deeper buffer against one, a narrower filter against the other.
#[test]
fn lines_dropped_for_age_and_lines_skipped_for_speed_are_said_apart() {
    let mut viewer = Viewer::holding(2);
    for said in ["one", "two", "three"] {
        viewer.take(line("sonarr", said));
    }
    assert_eq!(viewer.footing(), "1 older line dropped");

    viewer.take(line("sonarr", "four"));
    viewer.outpaced_by(1);
    viewer.outpaced_by(8);

    assert_eq!(
        viewer.footing(),
        "2 older lines dropped · 9 lines skipped to keep up"
    );
}

#[test]
fn everything_in_force_is_said_at_once() {
    // Two lines the filter admits, because scrolling back through one line has
    // nowhere to go and would leave the screen attached.
    let mut viewer = fed(&[
        ("sonarr", "WARN one timed out"),
        ("sonarr", "WARN two timed out"),
    ]);
    search(&mut viewer, "timed");
    press(&mut viewer, &[Press::Typed('w'), Press::Back]);

    assert_eq!(viewer.footing(), "/timed · info+ · detached");
}

#[test]
fn a_press_says_what_it_is() {
    assert!(format!("{:?}", Press::Rubout).contains("Rubout"));
}

/// What the engine reports for one service.
fn engine(service: &str, lifecycle: Lifecycle) -> Vec<(String, Lifecycle)> {
    vec![(service.to_owned(), lifecycle)]
}

/// Opening a viewer onto a running stack would otherwise print a notice for
/// every service in it, none of which is news.
#[test]
fn the_first_look_at_the_engine_is_not_news() {
    let mut viewer = a_viewer();

    viewer.doing(&engine("sonarr", Lifecycle::Running));

    assert_eq!(shown(&viewer, 10).len(), 3, "nothing was added");
}

/// The requirement in one test: the restart is in the stream, and the view
/// carries on around it.
#[test]
fn a_service_that_restarts_is_noted_without_ending_the_view() {
    let mut viewer = a_viewer();
    viewer.doing(&engine("sonarr", Lifecycle::Running));

    viewer.doing(&engine("sonarr", Lifecycle::Restarting));

    let said = shown(&viewer, 10);
    assert!(
        said.iter()
            .any(|line| line.contains("sonarr is restarting")),
        "{said:?}"
    );
    assert_eq!(
        said.len(),
        4,
        "it joined the lines rather than replacing them"
    );
    assert!(viewer.open(), "the view did not end");
    assert_eq!(viewer.footing(), "following", "and was not disturbed");
}

#[test]
fn a_service_that_has_not_changed_is_not_mentioned_again() {
    let mut viewer = a_viewer();
    for _ in 0..4 {
        viewer.doing(&engine("sonarr", Lifecycle::Running));
    }

    assert_eq!(shown(&viewer, 10).len(), 3);
}

/// A notice is tagged with the service it is about, so narrowing to that
/// service keeps the reason its output stopped.
#[test]
fn a_notice_belongs_to_the_service_it_is_about() {
    let mut viewer = a_viewer();
    viewer.doing(&engine("sonarr", Lifecycle::Running));
    viewer.doing(&engine("sonarr", Lifecycle::Restarting));

    viewer.pressed(Press::Typed('s'));

    assert_eq!(viewer.heading(WIDE), "sonarr");
    assert!(
        shown(&viewer, 10)
            .iter()
            .any(|line| line.contains("is restarting")),
        "a notice narrowed away with its own service"
    );
}

/// Every state the engine can report says what happened rather than naming
/// itself: `Exited` is a state, "has stopped" is news.
#[test]
fn every_state_the_engine_reports_reads_as_news() {
    for (lifecycle, expected) in [
        (Lifecycle::Created, "was created"),
        (Lifecycle::Running, "is running again"),
        (Lifecycle::Paused, "was paused"),
        (Lifecycle::Restarting, "is restarting"),
        (Lifecycle::Exited, "has stopped"),
        (Lifecycle::Removing, "is being removed"),
        (Lifecycle::Dead, "died"),
    ] {
        // Seeded with something this case is not, so every one is a change.
        let seed = match lifecycle {
            Lifecycle::Running => Lifecycle::Exited,
            _ => Lifecycle::Running,
        };
        let mut viewer = a_viewer();
        viewer.doing(&engine("sonarr", seed));
        viewer.doing(&engine("sonarr", lifecycle));

        let said = shown(&viewer, 10).concat();
        assert!(said.contains(expected), "{lifecycle:?}: {said}");
    }
}

/// Writing a file is the loop's to do, so the screen asks rather than does.
#[test]
fn asking_to_export_is_reported_rather_than_carried_out() {
    let mut viewer = a_viewer();

    assert_eq!(viewer.pressed(Press::Typed('e')), Asked::Export);
    assert_eq!(viewer.pressed(Press::Typed('f')), Asked::Nothing);
    assert_eq!(
        shown(&viewer, 10).len(),
        3,
        "asking changed nothing on the screen"
    );
}

/// The mode exists so that letters are letters; `e` is no more special than `q`.
#[test]
fn an_e_typed_into_a_search_is_a_letter_not_an_export() {
    let mut viewer = a_viewer();
    viewer.pressed(Press::Typed('/'));

    assert_eq!(viewer.pressed(Press::Typed('e')), Asked::Nothing);
    assert_eq!(viewer.typing(), Some("e"));
}

/// An export is a copy of what the operator is looking at. One that quietly
/// carried the lines they had narrowed away would be a different document.
#[test]
fn an_export_carries_what_the_filter_admits_and_nothing_else() {
    let mut viewer = a_viewer();
    search(&mut viewer, "timed");

    assert_eq!(exported(&viewer), "radarr | WARN Import timed out\n");
}

/// The shape the support bundle's own log extract takes, because the redaction
/// that runs over this was written against that shape.
#[test]
fn an_export_tags_every_line_with_the_service_that_wrote_it() {
    let text = exported(&a_viewer());

    assert_eq!(
        text,
        "sonarr | INFO Grabbed an episode\n\
         radarr | WARN Import timed out\n\
         sonarr | Torrent finished\n"
    );
}

/// A log line is somebody else's text, and a file it is written into is opened
/// by something eventually.
#[test]
fn an_export_carries_no_instruction_a_terminal_would_obey() {
    let viewer = fed(&[("sonarr", "INFO \u{1b}[2Jgone")]);

    let text = exported(&viewer);
    assert!(!text.contains('\u{1b}'), "{text:?}");
    assert!(text.contains("gone"), "{text:?}");
}

/// The requirement: an export is redacted by the support bundle's own rules, not
/// by a second set that could disagree with them about what a credential is.
///
/// Anchored on the rule that a query string goes wholesale — that is where the key
/// nobody spotted actually lives, riding inside something that looks like an
/// address — so this fails if the redaction is skipped or swapped for another.
#[test]
fn an_export_is_redacted_the_way_the_support_bundle_is() {
    let viewer = fed(&[("sonarr", "GET /api/v3/series?apikey=letmein done")]);

    let text = exported(&viewer);

    assert!(
        !text.contains("letmein"),
        "the key in the query survived into the export"
    );
    assert!(
        text.contains("/api/v3/series?"),
        "the address the key rode in on went with it"
    );
    assert!(
        text.contains("done"),
        "the rest of the line went with the key"
    );
}

/// What the viewer did belongs where the operator was reading, at the point it
/// happened — a row that the next thing overwrites cannot say when.
#[test]
fn a_remark_joins_the_stream_under_the_viewers_own_name() {
    let mut viewer = a_viewer();

    viewer.remarked("written to somewhere.txt");

    let said = shown(&viewer, 10);
    assert!(
        said.iter()
            .any(|line| line.contains("written to somewhere.txt")),
        "{said:?}"
    );
    assert_eq!(
        said.len(),
        4,
        "it joined the lines rather than replacing them"
    );
    assert_eq!(viewer.heading(WIDE), "sonarr, radarr, lemonfiber");
}

/// The convention is the variable's presence, not its value — so `NO_COLOR=0`
/// refuses colour like everything else does. Surprising exactly once, and what
/// every other tool that honours it does.
#[test]
fn any_value_at_all_refuses_colour() {
    assert!(colours(None), "unset means colour is fine");
    assert!(
        colours(Some("")),
        "set but empty is not set, by the convention"
    );

    for said in ["1", "0", "true", "false", "no", " "] {
        assert!(
            !colours(Some(said)),
            "NO_COLOR={said:?} should refuse colour"
        );
    }
}

#[test]
fn a_viewer_may_be_asked_to_add_no_colour() {
    assert!(Viewer::opened().colours(), "colour by default");
    assert!(!Viewer::opened().without_colour().colours());
}

/// An export whose text still reads as a credential after redaction is refused, naming
/// the line, and one that does not is handed over as it is.
#[test]
fn an_export_that_still_reads_as_a_credential_is_refused() {
    let terms = lemonfiber_core::bundle::Terms::default();
    let key: String = ('a'..='f').chain('0'..='9').cycle().take(32).collect();
    let still = format!("sonarr | fine\nsonarr | the key {key}\n");
    assert_eq!(
        cleared(still, &terms).err().map(|found| found.line),
        Some(2)
    );
    let clean = "sonarr | fine\n".to_owned();
    assert_eq!(cleared(clean.clone(), &terms).ok(), Some(clean));
}
