use super::{read, written};

/// A plain value is written as it is, so the settings an operator reads stay as they
/// always looked.
#[test]
fn a_plain_value_is_written_as_it_is() {
    for value in [
        "1000",
        "/srv/media",
        "Europe/Amsterdam",
        "a1b2c3",
        "http://x:8989",
        "",
    ] {
        assert_eq!(written(value), value);
        assert_eq!(read(&written(value)), value);
    }
}

/// Nothing Compose would expand survives into what it reads: a reference to another
/// setting, a bare `$`, a quote and a backslash all come back as exactly themselves.
#[test]
fn nothing_compose_would_expand_survives_the_writing() {
    let values = [
        "${WIREGUARD_PRIVATE_KEY}",
        "$OTHER",
        "pa$$word",
        "it's",
        "say \"hi\"",
        "back\\slash",
        "\\$",
        "ends with \\",
        "a # not a comment",
        "  padded  ",
        "'quoted'",
    ];
    for value in values {
        let spelled = written(value);
        assert!(spelled.starts_with('"'), "{value} is quoted: {spelled}");
        assert!(
            !spelled.contains("${") || spelled.contains("\\${"),
            "no reference is left for Compose to expand: {spelled}"
        );
        assert_eq!(read(&spelled), value, "{value} reads back as itself");
    }
}

/// What is spelled for a reference is exactly the escaped form Compose reads literally.
#[test]
fn a_reference_is_spelled_escaped() {
    assert_eq!(written("${OTHER}"), "\"\\${OTHER}\"");
    assert_eq!(written("a\"b"), "\"a\\\"b\"");
}

/// A value is read the way Compose reads it, whoever wrote it.
#[test]
fn a_value_is_read_the_way_compose_reads_it() {
    // Unquoted: up to ` #`, trailing space gone, `$$` for one `$`.
    assert_eq!(read("value # a note"), "value");
    assert_eq!(read(" spaced  "), "spaced");
    assert_eq!(read("a#b"), "a#b");
    assert_eq!(read("pa$$word"), "pa$word");
    // Single quotes: as written, apart from an escaped quote.
    assert_eq!(read("'$LITERAL'"), "$LITERAL");
    assert_eq!(read(r"'it\'s'"), "it's");
    assert_eq!(read(r"'a\nb'"), r"a\nb");
    // Double quotes: the escapes, an escaped quote, and `$$` for one `$`.
    assert_eq!(read(r#""a\tb\nc""#), "a\tb\nc");
    assert_eq!(read(r#""\a\b\f\r\v""#), "\u{7}\u{8}\u{c}\r\u{b}");
    assert_eq!(read(r#""say \"hi\"""#), "say \"hi\"");
    assert_eq!(read(r#""\q stays""#), r"\q stays");
    assert_eq!(read(r#""back\\slash""#), r"back\slash");
    assert_eq!(read(r#""cost $$5""#), "cost $5");
    assert_eq!(read(r#""quoted" # a note"#), "quoted");
}

/// An opening quote nothing closes is kept as written rather than guessed at.
#[test]
fn an_unclosed_quote_is_kept_as_written() {
    assert_eq!(read("\"open"), "\"open");
    assert_eq!(read(r#""ends \""#), r#""ends \""#);
    assert_eq!(read("'open\\"), "'open\\");
}
