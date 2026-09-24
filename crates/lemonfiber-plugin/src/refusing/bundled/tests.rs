use super::{answering, declared, hostnames, named, proxied, publishing, services, STACK};

/// The register found the stack, rather than finding nothing and passing.
///
/// Held to the shipped description by two facts it would be absurd to lose
/// quietly — that there are services at all, and that the one every other part
/// of this crate's fixtures is written around is among them — because the
/// failure this guards is not a wrong answer. It is no answer: a parse that
/// stopped working leaves every rule over this register walking an empty list,
/// and each of them would go on reporting that a manifest collides with nothing.
#[test]
fn the_register_holds_the_stack_this_build_ships() {
    // Counted before the assertion rather than inside its message. A value a
    // message reaches for is read only where the message is built, which is only
    // where the assertion fails — so the reading is a line no passing run enters,
    // and the coverage gate is right to call it one.
    let shipped = services().len();
    assert!(
        shipped > 10,
        "the shipped stack description was not read, so every rule over this register is \
             walking nothing: {shipped} services"
    );
    assert!(
        named("jellyfin").is_some(),
        "the stack ships a media server"
    );
    assert_eq!(named("jellyfin").and_then(|held| held.port), Some(8096));
}

/// A description this build cannot read holds nothing, and holds it visibly.
///
/// The reading is asked both ways here because the register above cannot ask
/// it either way: it reads one file, once, and an answer of *no services* from
/// it is the same silence whether the file was unreadable or empty.
#[test]
fn a_stack_description_that_cannot_be_read_holds_nothing() {
    assert!(declared("= not a stack description").is_empty());
    assert!(!declared(STACK).is_empty());
}

/// And the names it answers on, for the same reason.
#[test]
fn the_register_holds_the_names_the_proxy_answers_on() {
    let answered = hostnames();
    assert!(
        answered.len() > 5,
        "the shipped proxy configuration was not read, so a plugin could take any name it \
             already answers on: {answered:?}"
    );
    assert!(answering("watch"), "the stanza in force is read");
    assert!(answering("sonarr"), "and the one written out disabled");
    assert!(!answering("comics"), "and nothing it does not name");
}

/// A name nothing declares is not held.
#[test]
fn nothing_the_stack_does_not_ship_is_reserved() {
    assert!(named("komga").is_none());
    assert!(publishing(25600).is_none());
}

/// A port the stack publishes is found by the service that publishes it.
#[test]
fn a_port_the_stack_publishes_names_the_service_holding_it() {
    assert_eq!(
        publishing(8096).map(|held| held.id.as_str()),
        Some("jellyfin")
    );
}

/// The reader takes a label from a stanza and nothing from anything else.
///
/// On text planted here rather than only on the shipped file, because the
/// shipped file is one shape and the thing worth knowing is which shapes this
/// takes and which it leaves — a reader that took the prose as well would
/// reserve words nobody had written a stanza for.
#[test]
fn a_label_is_taken_from_a_stanza_and_from_nothing_else() {
    let planted = "\
watch.{$DOMAIN:home.local} {\n\
\treverse_proxy jellyfin:8096\n\
}\n\
# sonarr.{$DOMAIN:home.local}      { reverse_proxy sonarr:8989 }\n\
# Set DOMAIN in .env, and the {$DOMAIN placeholder is filled in.\n\
watch.{$DOMAIN:home.local} {\n\
}\n\
UPPER.{$DOMAIN:home.local} {\n\
}\n";
    assert_eq!(
        proxied(planted),
        vec!["watch".to_owned(), "sonarr".to_owned()]
    );
}

/// And a file with no stanza in it yields nothing, rather than a word.
#[test]
fn a_configuration_with_no_stanza_in_it_reserves_no_name() {
    assert!(proxied("# nothing here\nreverse_proxy jellyfin:8096\n").is_empty());
}
