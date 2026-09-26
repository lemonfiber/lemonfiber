use super::{
    prose, residual, settings, shown, Contents, Filenames, Marks, Piece, Residual, Taken, Terms,
    SALT_BYTES,
};
use crate::ports::random::Random;

/// Salt built rather than written, for the reason every credential-shaped fixture in
/// this repository is built: a literal that reaches a derivation is a hard-coded
/// cryptographic value, and a scanner reading it cannot tell a fixture from a mistake.
fn salt() -> Vec<u8> {
    ('a'..='p').map(|letter| letter as u8).collect()
}

/// A second bundle's salt — the same bytes the other way round, so the two differ
/// without either being written down.
fn other_salt() -> Vec<u8> {
    salt().into_iter().rev().collect()
}

/// A value shaped the way a generated key is: long, and carrying both letters and
/// digits. Built from character ranges rather than written, so that neither the
/// secret scanner reading this repository's history nor the analyser reading its
/// dataflow finds a credential-shaped literal to object to.
fn key_shaped() -> String {
    ('a'..='j').chain('0'..='9').cycle().take(32).collect()
}

/// A source of bytes that answers with what it was built from.
struct Fixed(Vec<u8>);

impl Random for Fixed {
    fn bytes(&self, n: usize) -> Option<Vec<u8>> {
        Some(self.0.iter().copied().cycle().take(n).collect())
    }
}

/// A source that cannot answer at all.
struct Empty;

impl Random for Empty {
    fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
        None
    }
}

/// Stand-ins over a built salt. Made through the same door the real ones are, so the
/// path that reads randomness is the path under test.
fn marks(salt: Vec<u8>) -> Marks {
    // Built directly where the source will not answer: a fallback nothing can reach
    // is a branch no test can cover, and this file is held to covering all of them.
    Marks::new(&Fixed(salt.clone())).unwrap_or(Marks::from_salt(salt))
}

/// Terms that hold what a test is about and default for the rest.
fn terms(revealed: &[&str], filenames: Filenames) -> Terms {
    Terms {
        window: "the last 200 lines of each service".to_owned(),
        filenames,
        revealed: revealed.iter().map(|field| (*field).to_owned()).collect(),
    }
}

/// The case the whole free-text rule exists for. An address is worth sharing and the
/// key riding in its query is not, and in a log line they arrive as one word with no
/// field name in front of them for an allow-list to recognise.
#[test]
fn a_key_riding_in_a_log_line_loses_the_query_and_keeps_the_address() {
    let line = format!(
        "prowlarr | GET https://indexer.example.com/api?apikey={}&t=search done",
        key_shaped()
    );
    let said = prose(&line, &marks(salt()), &Terms::default());

    assert!(
        said.contains("https://indexer.example.com/api?"),
        "the address went with the key riding in its query"
    );
    assert!(
        !said.contains(&key_shaped()),
        "the key riding in the query survived into the line"
    );
    assert!(
        said.contains("prowlarr | GET"),
        "the line still reads as a line"
    );
}

/// A key with nothing in front of it to name it is still a key. The allow-list has
/// nothing to work from in free text, so the rule reads values rather than names.
#[test]
fn a_key_with_no_name_in_front_of_it_is_replaced_anyway() {
    let said = prose(
        &format!("sonarr | using {}", key_shaped()),
        &marks(salt()),
        &Terms::default(),
    );
    assert!(
        !said.contains(&key_shaped()),
        "a key with no name in front of it survived into the line"
    );
}

/// The diagnostic the marks preserve: two mentions of one key read alike, so a reader
/// can see that two services point at the same account without seeing which.
#[test]
fn one_key_reads_the_same_wherever_it_appears_in_free_text() {
    let bundle = marks(salt());
    let key = key_shaped();
    let said = prose(
        &format!("first {key}\nsecond {key}"),
        &bundle,
        &Terms::default(),
    );
    let marked = bundle.of(&key);

    assert_eq!(said, format!("first {marked}\nsecond {marked}"));
}

/// A question in a log line is a question, not a query string — the parameters are
/// what make it one, and a rule that read every question mark as a URL would replace
/// half of what a reader came for.
#[test]
fn a_question_in_a_log_line_is_not_mistaken_for_a_query() {
    let said = prose("sonarr | why? no idea", &marks(salt()), &Terms::default());
    assert_eq!(said, "sonarr | why? no idea");
}

/// A line comes back spaced as it was written. A log line read by a person is half
/// indentation, and re-spacing one is the sort of tidying that loses the shape of a
/// stack trace.
#[test]
fn a_line_comes_back_spaced_as_it_was_written() {
    let said = prose(
        "sonarr |    indented   twice",
        &marks(salt()),
        &Terms::default(),
    );
    assert_eq!(said, "sonarr |    indented   twice");
}

/// Some operators consider their library their own business, so a filename is replaced
/// unless it was asked for — and the mark still says the two mentions are one file.
#[test]
fn a_media_filename_is_replaced_unless_it_was_asked_for() {
    let line = "sonarr | imported /media/tv/Some.Show.S01E01.mkv";
    let replaced = prose(line, &marks(salt()), &terms(&[], Filenames::Replaced));
    let shown = prose(line, &marks(salt()), &terms(&[], Filenames::Shown));

    assert!(!replaced.contains("Some.Show"), "{replaced}");
    assert!(shown.contains("/media/tv/Some.Show.S01E01.mkv"), "{shown}");
}

/// The invariant the two halves are built on: the redactor and the scan share a
/// tokeniser, so whatever one writes the other accepts. Without this they are free to
/// drift, and a bundle would start refusing itself over text it had just redacted.
#[test]
fn whatever_the_free_text_rule_writes_the_scan_accepts() {
    let key = key_shaped();
    let awkward = format!(
        "prowlarr | GET https://x.example.com/api?apikey={key}&t=search\nsonarr | bare {key}\nradarr | /media/f.mkv"
    );
    let said = prose(&awkward, &marks(salt()), &Terms::default());

    assert_eq!(
        residual(&[("logs.txt".to_owned(), said)], &Terms::default()),
        None
    );
}

/// Consent means the scan cannot refuse the bundle over the one value its operator
/// deliberately put in it — and means nothing anywhere else, because what was agreed
/// to was showing a field.
#[test]
fn a_revealed_setting_is_shown_and_is_not_a_residual() {
    let key = key_shaped();
    let asked = terms(&["SONARR_API_KEY"], Filenames::Replaced);
    let body = settings(&format!("SONARR_API_KEY={key}"), &marks(salt()), &asked);
    assert_eq!(body, format!("SONARR_API_KEY={key}"));
    assert_eq!(
        residual(&[("configuration.env".to_owned(), body)], &asked),
        None
    );

    // The same value in a log line was nobody's decision, and is still a leak.
    assert!(residual(
        &[("logs.txt".to_owned(), format!("sonarr | {key}"))],
        &asked
    )
    .is_some());
}

/// The reader is not the operator who chose any of this, so the first page says what
/// was bounded, what was replaced, and — where it applies — that the bundle holds a
/// credential in the clear.
#[test]
fn the_first_page_states_what_was_done_to_the_bundle() {
    let contents = Contents {
        pieces: vec![Piece {
            name: "logs.txt".to_owned(),
            body: String::new(),
        }],
        missing: Vec::new(),
        taken: Taken {
            lemonfiber: "0.7.0".to_owned(),
            stack: "1.2.0".to_owned(),
            at: "2026-08-18T00:00:00Z".to_owned(),
        },
        terms: terms(&[], Filenames::Replaced),
    };
    let page = contents.manifest();
    assert!(
        page.contains("Logs: the last 200 lines of each service."),
        "{page}"
    );
    assert!(page.contains("Media filenames: replaced."), "{page}");
    assert!(
        !page.contains("IN THE CLEAR"),
        "nothing was revealed: {page}"
    );

    let revealed = Contents {
        terms: terms(&["SONARR_API_KEY"], Filenames::Shown),
        ..contents
    };
    let page = revealed.manifest();
    assert!(page.contains("SHOWN AS THEY ARE"), "{page}");
    assert!(page.contains("  SONARR_API_KEY"), "{page}");
    assert!(page.contains("credential in the clear"), "{page}");
    assert!(page.contains("Media filenames: shown"), "{page}");
}

/// The property the whole design turns on: a setting nobody has thought about is
/// redacted, rather than shown because no rule named it.
#[test]
fn a_setting_nobody_named_is_redacted_rather_than_shown() {
    assert!(shown("PUID"));
    assert!(shown("USENET_HOST"));
    assert!(shown("INDEXER_VALIDATED"));
    assert!(!shown("INDEXER_APIKEY"));
    assert!(!shown("USENET_PASS"));
    // The one that matters: a field this build has never heard of.
    assert!(!shown("SOME_SERVICE_TOKEN_ADDED_NEXT_YEAR"));
}

/// Two allow-lists, and one of them is strictly the narrower.
///
/// A bundle is attached to a public thread and the configuration surface is read by
/// the operator whose machine it is, so the bundle shows less — but "less" is a
/// relationship somebody has to keep, and nothing enforces it by construction. A
/// setting safe to publish to strangers and withheld from its own owner is one of
/// the two lists being wrong, and this says which pair to look at.
#[test]
fn nothing_a_bundle_publishes_is_hidden_from_the_operator_who_owns_it() {
    let undisplayable: Vec<&str> = super::allowed::SHOWN
        .into_iter()
        .filter(|name| !crate::config::display::in_full(name))
        .collect();
    assert!(
        undisplayable.is_empty(),
        "a bundle shows these to anybody it is sent to and `config show` withholds \
         them from the operator: {undisplayable:?}"
    );
}

#[test]
fn a_shown_setting_keeps_its_value_and_a_hidden_one_keeps_only_its_name() {
    let secret = key_shaped();
    let shown = settings(
        &format!("PUID=1000\nUSENET_PASS={secret}"),
        &marks(salt()),
        &Terms::default(),
    );
    assert!(shown.contains("PUID=1000"));
    assert!(shown.contains("USENET_PASS=<redacted:"));
    assert!(!shown.contains(&secret));
}

/// The same secret reads the same throughout one bundle — that likeness is the whole
/// diagnostic point — and differently in the next, which is what keeps a mark from
/// becoming a fingerprint that outlives it.
#[test]
fn one_secret_reads_the_same_within_a_bundle_and_differently_across_two() {
    let bundle = marks(salt());
    let another = marks(other_salt());
    let value = key_shaped();
    let other = format!("{value}-elsewhere");
    assert_eq!(bundle.of(&value), bundle.of(&value));
    assert_ne!(bundle.of(&value), bundle.of(&other));
    assert_ne!(bundle.of(&value), another.of(&value));
}

/// The shape that catches people out: the address is worth sharing and the key riding
/// in its query string is not, and they arrive as one string.
#[test]
fn a_key_riding_in_a_query_string_does_not_ride_out_with_the_address() {
    let key = key_shaped();
    let redacted = settings(
        &format!("INDEXER_URL=https://indexer.example.com/api?apikey={key}\nPUID=1000"),
        &marks(salt()),
        &Terms::default(),
    );
    assert!(redacted.contains("https://indexer.example.com/api?<redacted:"));
    assert!(!redacted.contains(&key));
}

/// The other place a URL carries a credential, and the one this missed. A short
/// password is the case that matters: the residual scan behind this only recognises
/// a run of twenty characters or more, so nothing else would have caught it.
#[test]
fn a_password_in_front_of_the_host_does_not_ride_out_with_the_address() {
    let redacted = settings(
        "INDEXER_URL=https://operator:hunter2@indexer.example.com/api\nPUID=1000",
        &marks(salt()),
        &Terms::default(),
    );
    assert!(
        !redacted.contains("hunter2"),
        "the password went into a file an operator is told to attach to a thread: \
         {redacted}"
    );
    // The account goes with it: a service reached as `https://<token>@host` signs in by
    // the name alone, so the whole of what stands in front of the host is one mark, the
    // same mark wherever the same account appears.
    assert!(
        !redacted.contains("operator") && redacted.contains("https://<redacted:"),
        "the whole of what stands in front of the host is marked: {redacted}"
    );
    assert!(redacted.contains("PUID=1000"));

    // Both halves at once, which is the shape an indexer behind a proxy actually has.
    let key = key_shaped();
    let both = settings(
        &format!("INDEXER_URL=https://operator:hunter2@indexer.example.com/api?apikey={key}"),
        &marks(salt()),
        &Terms::default(),
    );
    assert!(!both.contains("hunter2"));
    assert!(!both.contains(&key));
}

/// A name with nothing after it is a setting that is not set, which is worth seeing:
/// an empty credential and a wrong one are different faults.
#[test]
fn a_setting_with_no_value_is_left_as_it_is() {
    let redacted = settings(
        "INDEXER_APIKEY=\nnot a setting at all",
        &marks(salt()),
        &Terms::default(),
    );
    assert!(redacted.contains("INDEXER_APIKEY="));
    assert!(!redacted.contains("<redacted:"));
    assert!(redacted.contains("not a setting at all"));
}

/// Randomness that cannot be had is not something to paper over: a fixed salt would
/// make every mark the same in every bundle anyone ever posted.
#[test]
fn marks_are_not_made_without_randomness() {
    assert!(Marks::new(&Empty).is_none());
    assert_eq!(salt().len(), SALT_BYTES);
}

/// The belt-and-braces check, and the reason it is a different mechanism: it knows
/// nothing about names, so a value that slipped through under a name nobody listed is
/// still caught by what it looks like.
#[test]
fn a_credential_that_survived_the_allow_list_is_still_caught() {
    let leaked = vec![(
        "sonarr/config.xml".to_owned(),
        format!("<ApiKey>{}</ApiKey>", key_shaped()),
    )];
    assert_eq!(
        residual(&leaked, &Terms::default()),
        Some(Residual {
            source: "sonarr/config.xml".to_owned(),
            line: 1,
        })
    );
}

#[test]
fn a_bundle_holding_only_marks_and_prose_is_let_through() {
    let clean = vec![
        (
            "env".to_owned(),
            "INDEXER_APIKEY=<redacted:a3f1>\nPUID=1000".to_owned(),
        ),
        (
            "notes".to_owned(),
            "the download client refused the login twice this afternoon".to_owned(),
        ),
    ];
    assert_eq!(residual(&clean, &Terms::default()), None);
}

/// Long is not enough and mixed is not enough: a version number is short, a run of
/// digits carries no letters, and a long word carries no digits. All three have to
/// hold at once before anything is called a key, and each of those three is a way a
/// bundle would otherwise be refused over a sentence.
#[test]
fn prose_and_versions_are_not_mistaken_for_credentials() {
    let ordinary = vec![(
        "report".to_owned(),
        "sonarr 4.0.15.2941 finished importing 37 episodes without incident\n\
         the download identifier 12345678901234567890 was retried once\n\
         recyclarr synchronised the qualityprofilesconfiguration definitions"
            .to_owned(),
    )];
    assert_eq!(residual(&ordinary, &Terms::default()), None);
}

/// A key in the base64 alphabet, built rather than written: upper and lower case,
/// digits, and the `+`, `/` and `=` the key tokenizer splits on.
fn encoded_key() -> String {
    let body: String = ('A'..='F')
        .chain('a'..='f')
        .chain('0'..='9')
        .chain(['+', '/'])
        .cycle()
        .take(43)
        .collect();
    format!("{body}=")
}

/// An identifier in the dashed form, built from hexadecimal ranges.
fn dashed() -> String {
    let hex =
        |length: usize| -> String { ('a'..='f').chain('0'..='9').cycle().take(length).collect() };
    format!("{}-{}-{}-{}-{}", hex(8), hex(4), hex(4), hex(4), hex(12))
}

/// A short password, built rather than written.
fn short_password() -> String {
    "emases".chars().rev().chain('1'..='4').collect()
}

/// Every shape a credential took through the redactor unchanged, each on its own line.
fn shapes() -> Vec<(&'static str, String)> {
    let password = short_password();
    vec![
        (
            "a password before the host",
            format!("sonarr | fetching https://reader:{password}@indexer.example/api failed"),
        ),
        (
            "a token as the whole userinfo",
            format!(
                "sonarr | fetching https://{}@indexer.example/api failed",
                key_shaped()
            ),
        ),
        (
            "a password named in the line",
            format!("sabnzbd | login rejected password={password}"),
        ),
        (
            "a key in the base64 alphabet",
            format!("gluetun | wireguard key {} loaded", encoded_key()),
        ),
        (
            "an identifier in the dashed form",
            format!("jellyfin | device {} signed in", dashed()),
        ),
    ]
}

/// Each shape is withheld by the redactor.
#[test]
fn every_shape_a_credential_takes_in_a_log_line_is_withheld() {
    let password = short_password();
    for (what, line) in shapes() {
        let said = prose(&line, &marks(salt()), &Terms::default());
        for secret in [password.as_str(), &key_shaped(), &encoded_key(), &dashed()] {
            assert!(!said.contains(secret), "{what}: {said}");
        }
        assert!(
            residual(&[("logs".to_owned(), said.clone())], &Terms::default()).is_none(),
            "{what}: what the redactor wrote is what the scan accepts: {said}"
        );
    }
}

/// And each shape, left as it was, is found by the scan behind the redactor on its own
/// terms — so a redactor that missed one is a bundle that is not written.
#[test]
fn every_shape_left_in_the_clear_is_found_by_the_scan() {
    for (what, line) in shapes() {
        let found = residual(&[("logs".to_owned(), line.clone())], &Terms::default());
        assert!(found.is_some(), "{what} passed the scan: {line}");
    }
}

/// A path is built from the same alphabet as an encoded key, and it is left as a path.
#[test]
fn a_path_in_a_log_line_is_left_as_a_path() {
    let line = "radarr | imported /data/media/movies/Some Film (2024)/Some.Film.2024.1080p.WEB.mkv";
    let said = prose(line, &marks(salt()), &Terms::default());
    assert!(said.contains("/data/media/movies/Some"), "{said}");
    assert!(residual(&[("logs".to_owned(), line.to_owned())], &Terms::default()).is_none());
}

/// A settings value that is an address keeps its host and loses whatever stands in
/// front of it, name and password both.
#[test]
fn an_address_setting_loses_its_userinfo_whole() {
    let body = format!("INDEXER_URL=https://{}@indexer.example/api", key_shaped());
    let said = settings(&body, &marks(salt()), &Terms::default());
    assert!(!said.contains(&key_shaped()), "{said}");
    assert!(said.contains("@indexer.example/api"), "{said}");
}

/// The scan's own reading of an encoded key: short runs of letters and digits joined
/// by the alphabet's own marks, long and mixed as a whole, and nothing the key rule
/// alone would find.
#[test]
fn a_key_only_the_encoding_reveals_is_found_by_the_scan() {
    let pieces: Vec<String> = ["Abc", "Def", "Ghi", "Jkl", "Mno", "Pqr", "Stu", "Vwx"]
        .iter()
        .zip('1'..='8')
        .map(|(letters, digit)| format!("{letters}{digit}"))
        .collect();
    let joined = pieces.join("+");
    let line = format!("gluetun | peer {joined}");
    assert!(
        residual(&[("logs".to_owned(), line.clone())], &Terms::default()).is_some(),
        "{line}"
    );

    // Long and mixed in case with no digit in it is a name, not a key.
    let named: String = [
        "Abcd", "Efgh", "Ijkl", "Mnop", "Qrst", "Uvwx", "Yzab", "Cdef",
    ]
    .concat();
    let line = format!("radarr | {named}");
    assert!(
        residual(&[("logs".to_owned(), line.clone())], &Terms::default()).is_none(),
        "{line}"
    );
}
