//! What the stack this build ships already holds, which a plugin may not take.
//!
//! Read out of the shipped stack description and the proxy configuration beside it,
//! rather than written down here as a list. A list would be a second answer to a
//! question those two files already answer, and it would be free to disagree with
//! them: a service dropped from the stack would go on reserving its id and its port,
//! one added would reserve neither, and nothing would say either had happened. These
//! are the files the binary is built from, so the pin moves and this moves with it.
//!
//! **A register that reads empty refuses nothing, and cannot notice that about
//! itself.** Every rule over one walks what it found, so a register that found
//! nothing passes exactly as quietly as a stack a manifest happens not to collide
//! with. What holds this one up is therefore not in it: the tests below ask it what
//! it found and hold that to what the shipped files declare, so a read that silently
//! stopped working is a failure rather than a silence.

use std::sync::LazyLock;

use lemonfiber_manifest::{Manifest, Service};

/// The stack description this build ships, which says which ids and ports are taken.
const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

/// The proxy configuration beside it, which says which names the stack answers on.
///
/// The hostnames are here and not in the stack description because they are the
/// proxy's own vocabulary — a label in front of the operator's domain rather than a
/// property of a service — and the shipped file is where they are written.
const PROXY: &str = include_str!("../../../../assets/media-stack/config/caddy/Caddyfile");

/// What a proxied name is written in front of, in the shipped configuration.
const DOMAIN: &str = ".{$DOMAIN";

/// Every service the shipped stack declares.
///
/// Read once, from a file this binary is compiled with. Deliberately not a latch:
/// a latch is settled by one caller and read by the rest, and a read that settled
/// it would make a later settle silently do nothing. There is no caller to hand
/// this a value — the answer is a function of the shipped description alone — so
/// the construct that says so is the one with no settle to be ignored.
pub(super) fn services() -> &'static [Service] {
    static SHIPPED: LazyLock<Vec<Service>> = LazyLock::new(|| declared(STACK));
    &SHIPPED
}

/// The services one stack description declares, or none where it cannot be read.
///
/// Apart from the register above so that both of its answers can be asked for. An
/// unreadable description yields no services, and every rule over the register then
/// walks nothing — which is invisible from inside those rules and has to be a test's
/// job, so the reading is a function a test can hand a description to.
fn declared(text: &str) -> Vec<Service> {
    Manifest::from_toml(text).map_or_else(|_| Vec::new(), |stack| stack.services)
}

/// Every name the shipped proxy is written to answer on.
///
/// The stanzas that are commented out count. They are not dead text: the shipped
/// file writes each one out with what enabling it would mean, so they are names the
/// operator has already been offered — and a plugin holding one would turn that
/// offer into a collision on the day it was taken up, in a file the operator is
/// editing by hand and would have no reason to suspect.
pub(super) fn hostnames() -> &'static [String] {
    static ANSWERED: LazyLock<Vec<String>> = LazyLock::new(|| proxied(PROXY));
    &ANSWERED
}

/// The shipped service with this id, where the stack ships one.
pub(super) fn named(id: &str) -> Option<&'static Service> {
    services().iter().find(|service| service.id == id)
}

/// The shipped service published on this port, where the stack publishes one there.
pub(super) fn publishing(port: u16) -> Option<&'static Service> {
    services().iter().find(|service| service.port == Some(port))
}

/// Whether the shipped proxy is written to answer on this name.
pub(super) fn answering(label: &str) -> bool {
    hostnames().iter().any(|held| held == label)
}

/// Every label the proxy configuration puts in front of the operator's domain.
///
/// Read off the one shape the file is written in — a label, the domain placeholder,
/// and a block — with a leading comment marker allowed, because a disabled stanza is
/// still a name the file has spoken for. Anything else in the file says nothing
/// about a name, including the prose that mentions the placeholder without putting a
/// label in front of it.
fn proxied(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in text.lines() {
        let said = line.trim_start().trim_start_matches('#').trim_start();
        let Some((label, _)) = said.split_once(DOMAIN) else {
            continue;
        };
        if label.is_empty() || !label.chars().all(is_label) {
            continue;
        }
        if !found.iter().any(|held| held == label) {
            found.push(label.to_owned());
        }
    }
    found
}

/// Whether one character may appear in a DNS label.
fn is_label(letter: char) -> bool {
    letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '-'
}

#[cfg(test)]
mod tests {
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
}
