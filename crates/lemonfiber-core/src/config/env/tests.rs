use super::{is_one_line, EnvFile};

/// The file this stack ships as its documented starting point.
const EXAMPLE: &str = include_str!("../../../../../assets/media-stack/.env.example");

#[test]
fn the_stack_s_own_example_round_trips_byte_for_byte() {
    assert_eq!(EnvFile::parse(EXAMPLE).render(), EXAMPLE);
}

#[test]
fn reads_every_setting_the_example_declares() {
    let file = EnvFile::parse(EXAMPLE);
    assert_eq!(file.keys().len(), 28);
    assert_eq!(file.get("DATA_ROOT"), Some("./data"));
    assert_eq!(file.get("TZ"), Some("Europe/Amsterdam"));
    // Six of the twenty-eight carry a digit. A key pattern of letters and
    // underscores alone would read them as prose and drop them silently.
    assert_eq!(file.get("UN_SONARR_0_URL"), Some(""));
}

#[test]
fn an_empty_value_is_a_value_rather_than_an_absence() {
    let file = EnvFile::parse(EXAMPLE);
    assert_eq!(
        file.get("WIREGUARD_PRIVATE_KEY"),
        Some(""),
        "the key is declared and waiting to be filled in"
    );
    assert_eq!(file.get("NOT_IN_THE_FILE"), None);
}

#[test]
fn removing_a_key_drops_its_line_and_keeps_the_rest() {
    let mut file = EnvFile::parse("# what B is for\nA=1\nB=2\n");
    file.remove("A");

    assert_eq!(file.render(), "# what B is for\nB=2\n");
    assert_eq!(file.get("A"), None);
}

#[test]
fn a_value_spanning_lines_is_more_settings_rather_than_one_value() {
    // Why the writer refuses rather than escaping: a break in the value is a break
    // in the file, quoted or not, so what it makes is a second setting, and this
    // shows it happening.
    let mut file = EnvFile::parse("TZ=UTC\n");
    file.set("INDEXER_APIKEY", "abc\nPUID=0");

    assert!(EnvFile::parse(&file.render()).get("PUID").is_some());
    assert!(!is_one_line("abc\nPUID=0"));
    assert!(!is_one_line("abc\rPUID=0"));
    assert!(is_one_line("abc"));
    assert!(is_one_line(""));
}

#[test]
fn removing_a_key_that_is_not_there_changes_nothing() {
    let mut file = EnvFile::parse("A=1\n");
    file.remove("MISSING");

    assert_eq!(file.render(), "A=1\n");
}

#[test]
fn removing_a_duplicated_key_drops_the_one_that_takes_effect() {
    // A hand-edited file with the key twice: `get` and `set` act on the last,
    // so `remove` must too, leaving the earlier line where it is.
    let mut file = EnvFile::parse("A=first\nA=second\n");
    file.remove("A");

    assert_eq!(file.render(), "A=first\n");
    assert_eq!(file.get("A"), Some("first"));
}

#[test]
fn changing_a_value_leaves_everything_around_it_alone() {
    let mut file = EnvFile::parse(EXAMPLE);
    file.set("TZ", "Pacific/Auckland");

    let rendered = file.render();
    assert_eq!(
        EnvFile::parse(&rendered).get("TZ"),
        Some("Pacific/Auckland")
    );
    assert_eq!(
        rendered.lines().count(),
        EXAMPLE.lines().count(),
        "no line was added or removed"
    );
    assert!(
        rendered.contains("# The user that owns everything under DATA_ROOT"),
        "the comments explaining the settings survive"
    );
}

#[test]
fn a_setting_stays_under_the_comment_that_explains_it() {
    let text = "# why this matters\nKEY=old\n# something else\nOTHER=x\n";
    let mut file = EnvFile::parse(text);
    file.set("KEY", "new");
    assert_eq!(
        file.render(),
        "# why this matters\nKEY=new\n# something else\nOTHER=x\n"
    );
}

#[test]
fn a_new_setting_is_appended() {
    let mut file = EnvFile::parse("A=1\n");
    file.set("B", "2");
    assert_eq!(file.render(), "A=1\nB=2\n");
}

#[test]
fn a_file_with_no_trailing_newline_keeps_not_having_one() {
    assert_eq!(EnvFile::parse("A=1").render(), "A=1");
    assert_eq!(EnvFile::parse("A=1\n").render(), "A=1\n");
    assert_eq!(EnvFile::parse("").render(), "");
}

#[test]
fn anything_that_is_not_a_setting_is_kept_as_it_was() {
    // A value containing `=`, an indented key, a bare word, a line that is
    // only a comment: none of these should be normalised into something
    // tidier, because tidying is how a hand-edited file loses its meaning.
    let text = "  SPACED=1\nVALUE=a=b\nnot a setting\n#\n\n";
    assert_eq!(EnvFile::parse(text).render(), text);

    let file = EnvFile::parse(text);
    assert_eq!(file.get("SPACED"), Some("1"));
    assert_eq!(file.get("VALUE"), Some("a=b"));
    assert_eq!(file.keys(), vec!["SPACED", "VALUE"]);
}

#[test]
fn a_repeated_key_reads_as_the_last_one_set() {
    // Which is what a shell would do with the same file.
    let file = EnvFile::parse("A=first\nA=second\n");
    assert_eq!(file.get("A"), Some("second"));
}

#[test]
fn a_repeated_key_is_changed_where_it_takes_effect() {
    // The last occurrence is the one the shell and `get` read, so that is the
    // one `set` rewrites — otherwise the change is a silent no-op.
    let mut file = EnvFile::parse("A=first\nA=second\n");
    file.set("A", "only");
    assert_eq!(file.render(), "A=first\nA=only\n");
    assert_eq!(file.get("A"), Some("only"));
}

#[test]
fn a_key_that_is_not_a_key_is_left_alone() {
    let text = "not-a-key=value\n";
    let file = EnvFile::parse(text);
    assert_eq!(file.keys(), Vec::<&str>::new());
    assert_eq!(file.render(), text);
}

/// A value a container wrote about itself cannot have Compose fill it with another
/// setting: what is written reads back, to lemonfiber and to Compose alike, as exactly
/// the text it was — the reference, the dollar, the quote — and nothing else on the
/// line or around it moves.
#[test]
fn a_value_holding_a_reference_is_written_so_nothing_expands_it() {
    let mut file = EnvFile::parse("# keys\nWIREGUARD_PRIVATE_KEY=secret\n");
    let planted = "${WIREGUARD_PRIVATE_KEY}$it's\"";

    file.set("SONARR_API_KEY", planted);
    let text = file.render();

    assert_eq!(
        text,
        "# keys\nWIREGUARD_PRIVATE_KEY=secret\nSONARR_API_KEY=\"\\${WIREGUARD_PRIVATE_KEY}\\$it's\\\"\"\n"
    );
    assert_eq!(EnvFile::parse(&text).get("SONARR_API_KEY"), Some(planted));
}

/// A setting an operator quoted by hand is read as the value they meant, not with its
/// quotes, and is written back exactly as they spelled it while nothing changes it.
#[test]
fn a_hand_quoted_setting_is_read_as_meant_and_kept_as_spelled() {
    let text = "TZ=\"Europe/Amsterdam\"\nNOTE='$HOME' # literal\nPASS=a$$b\n";
    let file = EnvFile::parse(text);

    assert_eq!(file.get("TZ"), Some("Europe/Amsterdam"));
    assert_eq!(file.get("NOTE"), Some("$HOME"));
    assert_eq!(file.get("PASS"), Some("a$b"));
    assert_eq!(file.render(), text);
}
