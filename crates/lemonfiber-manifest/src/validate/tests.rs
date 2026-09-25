use super::{identifiers, validate, Date, Violation, OSI};
use crate::Manifest;

/// The stack's own copy of the same list, which this one follows.
const THEIRS: &str = include_str!("../../../../assets/media-stack/scripts/spdx_osi.txt");

/// Two copies of one list, and a sentence at the top of ours saying they agree.
///
/// They do today. Nothing made them, and the drift is silent in the direction it
/// would actually happen: the stack adds a licence, ships a service under it, and
/// this validator refuses a manifest the stack's own validator accepted — with an
/// error naming the licence, which is the one thing that is not wrong.
///
/// The submodule is pinned, so this asks at the moment it is worth asking: when
/// the stack this binary carries is moved forward.
#[test]
fn the_licence_list_holds_what_the_stack_s_own_copy_holds() {
    let ours = identifiers(OSI);
    let theirs = identifiers(THEIRS);
    let only_ours: Vec<&str> = ours.difference(&theirs).copied().collect();
    let only_theirs: Vec<&str> = theirs.difference(&ours).copied().collect();
    assert!(
        only_ours.is_empty() && only_theirs.is_empty(),
        "the two vendored licence lists have drifted, and the stack's copy is the \
             one a maintainer edits: here and not there {only_ours:?}, there and not \
             here {only_theirs:?}"
    );
}

const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

/// After every date the stack records, and not the real clock.
///
/// A service records the day its upstream last released, and a date after this
/// one is a fault — so bumping a pin to a newer release puts the stack ahead of
/// this and the fixture reports a fault that is nobody's mistake.
/// `the_day_these_tests_use_is_after_everything_the_stack_records` is the guard;
/// when it fails, move this forward rather than pinning an older image.
const TODAY: Date = Date {
    year: 2026,
    month: 10,
    day: 1,
};

fn check(text: &str) -> Vec<Violation> {
    Manifest::from_toml(text)
        .ok()
        .map(|manifest| validate(&manifest, TODAY))
        .unwrap_or_default()
}

fn messages(text: &str) -> Vec<String> {
    check(text).iter().map(ToString::to_string).collect()
}

/// Replace the first occurrence of `from` with `to`.
fn edited(from: &str, to: &str) -> String {
    assert!(STACK.contains(from), "the fixture must contain {from:?}");
    STACK.replacen(from, to, 1)
}

/// The day these tests use is after everything the stack records.
///
/// Every fixture here validates the committed stack against `TODAY`, and the
/// stack is a submodule that moves forward while this constant does not. Without
/// this, a pin bump makes `the_stack_this_binary_ships_is_valid` fail for a
/// reason that has nothing to do with the stack being invalid.
#[test]
fn the_day_these_tests_use_is_after_everything_the_stack_records() {
    let latest = STACK
        .lines()
        .filter_map(|line| line.trim().strip_prefix("last_release = "))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .max()
        .unwrap_or_default();
    let said = format!("{:04}-{:02}-{:02}", TODAY.year, TODAY.month, TODAY.day);

    assert!(
        said.as_str() >= latest.as_str(),
        "these tests validate against {said} and the stack records a release on \
             {latest}; move TODAY forward past it — the fixtures report a fault that \
             is nobody's mistake otherwise."
    );
}

#[test]
fn the_stack_this_binary_ships_is_valid() {
    assert_eq!(
        check(STACK),
        Vec::new(),
        "the committed stack has no faults"
    );
}

#[test]
fn a_servarr_service_without_an_api_version_is_caught() {
    // The servarr shape spans two API versions, so one omitting it cannot be
    // reached at a known path; dropping the first version the stack records
    // (Sonarr's v3) must be a surfaced fault, not a silent exclusion.
    let text = edited(
        r#"path = "/config/config.xml", version = 3 }"#,
        r#"path = "/config/config.xml" }"#,
    );
    let caught = messages(&text)
        .iter()
        .any(|message| message.contains("names no api.version"));
    assert!(caught);
}

#[test]
fn a_duplicate_profile_id_is_caught() {
    let text = edited(r#"id = "usenet""#, r#"id = "search""#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("another profile already has this id")));
}

#[test]
fn two_profiles_claiming_one_protocol_is_caught() {
    let text = edited(r#"protocol = "usenet""#, r#"protocol = "torrent""#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("already carries this protocol")));
}

#[test]
fn a_form_naming_an_undeclared_profile_is_caught() {
    let text = edited(r#"profiles = ["search"]"#, r#"profiles = ["telly"]"#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("names profile telly, which is not declared")));
}

#[test]
fn a_service_in_an_undeclared_profile_is_caught() {
    let text = edited(r#"profile = "search""#, r#"profile = "telly""#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("is in profile telly, which is not declared")));
}

#[test]
fn a_floating_tag_is_caught() {
    for floating in ["latest", "stable", "nightly", "main"] {
        let text = STACK.replacen("tag = ", &format!("tag = \"{floating}\" # "), 1);
        assert!(
            messages(&text)
                .iter()
                .any(|m| m.contains("that is not a pin")),
            "{floating} should not be accepted as a pin"
        );
    }
}

#[test]
fn an_empty_tag_is_caught() {
    let text = STACK.replacen("tag = ", "tag = \"\" # ", 1);
    assert!(
        messages(&text)
            .iter()
            .any(|m| m.contains("empty image tag")),
        "an empty tag is a broken image reference, not a pin"
    );
}

#[test]
fn a_published_port_with_no_binding_is_caught() {
    let text = edited("bind = \"loopback\"\n", "");
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("does not say which interface")));
}

#[test]
fn a_memory_estimate_of_nothing_is_caught() {
    let text = edited("memory_mib = 600\n", "memory_mib = 0\n");
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("needs no memory at all")));
}

#[test]
fn a_licence_outside_the_osi_list_is_caught() {
    let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("not a recognised OSI identifier")));
}

#[test]
fn a_malformed_last_release_is_caught() {
    let text = STACK.replacen("last_release = ", "last_release = \"26-07-01\" # ", 1);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("which is not YYYY-MM-DD")));
}

#[test]
fn a_last_release_in_the_future_is_caught() {
    let text = STACK.replacen("last_release = ", "last_release = \"2099-01-01\" # ", 1);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("which is in the future")));
}

#[test]
fn a_grant_outside_the_allow_list_is_caught() {
    let text = edited(r#"grants = ["NET_ADMIN"]"#, r#"grants = ["SYS_ADMIN"]"#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("kernel capability SYS_ADMIN, which is not allowed")));
}

/// The line every service in the shipped stack has, which these cases add to.
const AN_ID: &str = "id = \"prowlarr\"";

/// The first `provides` the shipped stack declares, which these cases stand in for.
///
/// Read out of the stack rather than written here, so a stack that stops declaring
/// it is a test that fails loudly instead of one quietly proving nothing.
const OFFERS: &str = "provides = [\"indexer.search\"]";

/// The stack with one service declaring the given capabilities instead.
///
/// Replaced rather than added beside: every service something can ask for declares
/// one now, and a second key in the same table is a parse failure rather than the
/// rule under test.
fn offering(named: &str) -> String {
    edited(OFFERS, &format!("provides = [{named}]"))
}

/// A core name is `area.verb`, and the three ways it can fail to be one.
///
/// Together rather than one test each, because they are one rule read three ways
/// and what matters is that each is *named* — a service told its capability is
/// wrong without being told which one is a service somebody has to go and diff.
#[test]
fn a_capability_that_is_not_a_core_name_is_caught() {
    for wrong in [
        "\"mediaserve\"",
        "\"media.serve.now\"",
        "\"komga:opds\"",
        "\"Media.Serve\"",
        "\"media.\"",
    ] {
        let said = messages(&offering(wrong));
        assert!(
            said.iter().any(
                |fault| fault.contains("which is not a core capability name")
                    && fault.contains(wrong.trim_matches('"'))
            ),
            "{wrong} was not named: {said:?}"
        );
    }
}

/// One service declaring the same capability twice says it once and confuses the
/// count of who declares it.
#[test]
fn a_capability_declared_twice_by_one_service_is_caught() {
    let said = messages(&offering("\"media.serve\", \"media.serve\""));
    assert!(
        said.iter()
            .any(|fault| fault.contains("provides media.serve more than once")),
        "{said:?}"
    );
}

/// The shape rule and nothing beyond it: a core name this crate has never heard of
/// passes here, because what the vocabulary carries is the vocabulary's question.
#[test]
fn a_core_name_this_crate_knows_nothing_about_is_left_alone() {
    // Joined rather than searched through a closure, which an empty list never
    // enters — and an assertion whose only interesting half is a line no run
    // reaches is an assertion the coverage gate is right to call missing.
    let said = messages(&offering("\"media.serve\", \"nothing.here\"")).join("\n");
    assert!(!said.contains("provides"), "{said}");
}

/// Every capability the shipped stack declares is a core name, and none is twice.
///
/// The stack is the only caller of this rule that anybody actually reads, so the
/// case that matters most is the one where it is right — four of the twenty
/// services declare nothing at all, which is an answer rather than an omission.
#[test]
fn every_capability_the_shipped_stack_declares_passes_the_shape_rule() {
    let said = messages(STACK).join("\n");
    assert!(!said.contains("provides"), "{said}");
}

/// The first of the two outbound lines the shipped stack carries, and the second.
///
/// Read out of the stack rather than written here, so a stack that rewords either
/// of them is a test that fails loudly instead of one quietly proving nothing.
const REACHES: &str = "reaches = ";
const ASKS_FOR: &str = "asks_for = ";

/// The stack with the first line starting `prefix` removed from it.
fn without(prefix: &str) -> String {
    let at = STACK
        .lines()
        .position(|line| line.starts_with(prefix))
        .unwrap_or_default();
    assert!(at > 0, "the fixture must declare {prefix:?}");
    STACK
        .lines()
        .enumerate()
        .filter_map(|(number, line)| (number != at).then_some(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Half a declaration is a misleading answer rather than a smaller one, so it is
/// refused in either direction.
///
/// Made by taking a half away rather than by adding one. Every service the stack
/// ships now declares both, so a case built by adding `reaches` to one of them
/// would be a second copy of a key the entry already has — which the parser
/// refuses before this rule is ever asked.
#[test]
fn declaring_where_a_service_reaches_without_what_it_asks_for_is_caught() {
    let said = messages(&without(ASKS_FOR));
    assert!(
        said.iter().any(|fault| fault.contains("what it asks for")),
        "{said:?}"
    );
}

#[test]
fn declaring_what_a_service_asks_for_without_where_it_reaches_is_caught() {
    let said = messages(&without(REACHES));
    assert!(
        said.iter().any(|fault| fault.contains("where it reaches")),
        "{said:?}"
    );
}

/// Both, or neither, and neither is what every stack written before this says.
#[test]
fn a_service_that_declares_both_or_neither_says_nothing_is_wrong() {
    let both = messages(&edited(
        AN_ID,
        "id = \"prowlarr\"\nreaches = \"the indexers you configured\"\n\
             asks_for = \"Runs the searches\"",
    ));
    assert!(both.is_empty(), "{both:?}");
    // And an empty destination is an answer rather than an absence: a service
    // that reaches nothing says so, with what it does instead.
    let quiet = messages(&edited(
        AN_ID,
        "id = \"prowlarr\"\nreaches = \"\"\nasks_for = \"Nothing leaves this machine\"",
    ));
    assert!(quiet.is_empty(), "{quiet:?}");
}

#[test]
fn a_dependency_across_a_profile_boundary_is_caught() {
    let text = edited(
        r#"depends_on = ["gluetun"]"#,
        r#"depends_on = ["prowlarr"]"#,
    );
    assert!(
        messages(&text)
            .iter()
            .any(|m| m.contains("which is in profile search rather than torrent")),
        "a dependency across a profile boundary must be named"
    );
}

#[test]
fn a_dependency_on_nothing_at_all_is_caught() {
    let text = edited(r#"depends_on = ["gluetun"]"#, r#"depends_on = ["ghost"]"#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("depends on ghost, which is not a service here")));
}

#[test]
fn every_violation_is_reported_rather_than_only_the_first() {
    let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#).replacen(
        "last_release = ",
        "last_release = \"2099-01-01\" # ",
        1,
    );
    let reported = messages(&text);
    assert!(
        reported.len() >= 2,
        "fixing a fork should not be a guessing game: {reported:?}"
    );
}

#[test]
fn every_violation_says_where_it_is() {
    let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#);
    assert!(check(&text)
        .iter()
        .all(|violation| !violation.location.is_empty()));
}

#[test]
fn a_duplicate_form_id_is_caught() {
    let text = edited(r#"id = "dl""#, r#"id = "search""#);
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("another form already has this id")));
}

/// One removal recorded against the shipped stack, which declares the service it
/// names as a replacement.
///
/// Named for nothing that ever existed, on purpose. The stack is a submodule that
/// moves, and it is about to start recording removals of its own — an id borrowed
/// from a real one would collide with the stack's own entry the day its pin moves,
/// and these tests would report a duplicate that is nobody's mistake.
const DROPPED: &str = r#"
[[removed]]
id = "an-old-thing"
removed_in = "0.1.0"
reason = "Discontinued upstream in 2025; the project is archived and releases nothing."
replaced_by = "bindery"
"#;

/// The shipped stack with one removal appended to it.
fn with_removal(record: &str) -> String {
    format!("{STACK}{record}")
}

/// A record that says what went, why, and what took its place is a record with
/// nothing wrong with it — which is the half every other assertion here rests on.
#[test]
fn a_removal_recorded_properly_is_no_fault_at_all() {
    assert_eq!(check(&with_removal(DROPPED)), Vec::new());
}

#[test]
fn a_removal_that_is_still_a_declared_service_is_caught() {
    let text = with_removal(&DROPPED.replace(r#"id = "an-old-thing""#, r#"id = "bindery""#));
    assert!(
        messages(&text)
            .iter()
            .any(|m| m.contains("is recorded as removed and is still declared as a service")),
        "a stack that goes on starting what it says it dropped must be named"
    );
}

/// Emptiness rather than presence: an empty reason passes a parse and records
/// nothing, which is the shape a table filled in to satisfy a rule takes.
#[test]
fn a_removal_with_an_empty_reason_is_caught() {
    let text = with_removal(&DROPPED.replacen("reason = ", "reason = \"\" # ", 1));
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("records no reason for going")));
}

#[test]
fn a_replacement_this_stack_knows_nothing_about_is_caught() {
    let text =
        with_removal(&DROPPED.replace(r#"replaced_by = "bindery""#, r#"replaced_by = "ghost""#));
    assert!(
        messages(&text)
            .iter()
            .any(|m| m
                .contains("says ghost replaced it, and this stack neither declares nor records")),
        "a replacement pointing at nothing reads as an answer and is not one"
    );
}

/// Replacements chain. A stack that dropped the thing that replaced the thing it
/// dropped has recorded two true facts, and refusing the second would make the
/// record less honest the longer the stack lives.
#[test]
fn a_removal_replaced_by_another_removal_is_accepted() {
    let chained = format!(
        "{}{}",
        DROPPED.replace(
            r#"replaced_by = "bindery""#,
            r#"replaced_by = "an-older-thing""#
        ),
        r#"
[[removed]]
id = "an-older-thing"
removed_in = "0.1.0"
reason = "Unmaintained upstream; Audiobookshelf covers what it did."
replaced_by = "audiobookshelf"
"#
    );
    assert_eq!(check(&with_removal(&chained)), Vec::new());
}

/// Empty is not the same as absent, and the difference is the whole of what a
/// replacement field records. A removal that names nothing has said the service
/// went and nothing took over, which is a fact. One naming an empty string has
/// filled the box in to get past the rule, and an operator reading the record
/// afterwards is told there was a successor whose name nobody wrote down.
#[test]
fn a_replacement_named_as_an_empty_string_is_caught() {
    let text =
        with_removal(&DROPPED.replace(r#"replaced_by = "bindery""#, r#"replaced_by = "  ""#));
    assert!(
        messages(&text)
            .iter()
            .any(|m| m.contains("names an empty replacement rather than none at all")),
        "a blank successor reads as a successor and is not one"
    );
}

#[test]
fn a_removal_recorded_as_replacing_itself_is_caught() {
    let text = with_removal(&DROPPED.replace(
        r#"replaced_by = "bindery""#,
        r#"replaced_by = "an-old-thing""#,
    ));
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("is recorded as having replaced itself")));
}

/// The version it went in is checked for holding something, the way the reason is:
/// a record that cannot be placed in the stack's own history answers *when* with
/// nothing at all.
#[test]
fn a_removal_with_no_stack_version_behind_it_is_caught() {
    let text = with_removal(&DROPPED.replacen("removed_in = ", "removed_in = \"\" # ", 1));
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("records no stack version it went in")));
}

#[test]
fn a_duplicate_removal_id_is_caught() {
    let text = with_removal(&format!("{DROPPED}{DROPPED}"));
    assert!(messages(&text)
        .iter()
        .any(|m| m.contains("another removal already has this id")));
}

/// The removals are walked in the same pass the services are, so a fork that got
/// both wrong hears about both — which is the whole reason this file reports
/// rather than returns at the first thing it finds.
#[test]
fn a_fault_in_a_removal_arrives_beside_a_fault_in_a_service() {
    let text = edited(r#"license = "GPL-3.0-only""#, r#"license = "Nonesuch-1.0""#);
    let both = messages(&format!(
        "{text}{}",
        DROPPED.replacen("reason = ", "reason = \"\" # ", 1)
    ));
    assert!(
        both.iter()
            .any(|m| m.contains("not a recognised OSI identifier")),
        "{both:?}"
    );
    assert!(
        both.iter()
            .any(|m| m.contains("records no reason for going")),
        "{both:?}"
    );
}

/// Every violation says where it is, and a removal's location names the removal
/// rather than the service it shares an id with — there is no such service, which
/// is the point.
#[test]
fn a_removal_s_fault_is_placed_by_the_removal_it_is_about() {
    let text = with_removal(&DROPPED.replacen("reason = ", "reason = \"\" # ", 1));
    assert!(check(&text)
        .iter()
        .any(|violation| violation.location == "removed an-old-thing"));
}

#[test]
fn a_duplicate_service_id_is_caught() {
    let text = edited(r#"id = "sonarr""#, r#"id = "prowlarr""#);
    assert!(
        messages(&text)
            .iter()
            .any(|m| m.contains("another service already has this id")),
        "a repeated service id must be named"
    );
}
