//! The three accounts a plugin's things appear *on* rather than beside.
//!
//! A plugin holds secrets, reaches hosts and changes settings. Each of those already
//! has one listing an operator reads, and the cheap way to add a plugin's is a second
//! listing of its own. The cost lands on whoever is later trying to find out what this
//! machine holds: the answer is in two places, and the one they know about is the one
//! that no longer has all of it.
//!
//! So the rule is about *declared names* rather than about intent, because a name is
//! the only evidence there is before the second account has any callers. A type called
//! `PluginCredentials`, a function called `plugin_hosts`, a report called
//! `PluginSettings` — each is the second account arriving, and each is refused here
//! rather than after something reads it.
//!
//! **The half that matters is what it must not refuse.** Attributing a plugin *on* the
//! existing listing puts a plugin word and an account word in the same file and often
//! in the same function, so a rule reading files would refuse exactly the change that
//! answers the requirement, and would do it while looking like it was working. That is
//! why this reads declarations, and why the accepting half is driven over a fixture of
//! its own rather than left to hope: a rule tested only by what it rejects passes just
//! as happily when it rejects everything.

mod source_tree;

use source_tree::{shipped, workspace_root};

/// One account a plugin's things belong on.
struct Account {
    /// What it is an account of, in the words the refusal uses.
    named: &'static str,
    /// Every spelling a second listing of it would be built around.
    spelled: &'static [&'static str],
    /// Where the one account already is, so a refusal says where to put it instead.
    kept: &'static str,
}

/// The three, and there are three because those are the three a plugin brings
/// something to. A wiring and a check are attributed rather than accounted for, and a
/// second listing of either would be caught by the settings spelling it shares.
const ACCOUNTS: &[Account] = &[
    Account {
        named: "the secrets this stack holds",
        spelled: &["credential", "secret"],
        kept: "crates/lemonfiber-core/src/credential.rs",
    },
    Account {
        named: "what leaves this machine",
        spelled: &["outbound", "egress", "hosts"],
        kept: "crates/lemonfiber-core/src/outbound.rs",
    },
    Account {
        named: "the settings in force",
        spelled: &["setting"],
        kept: "crates/lemonfiber-core/src/config/store.rs",
    },
];

/// What marks a declaration as being about a plugin's own things.
const BROUGHT: &[&str] = &["plugin"];

/// The keywords a name worth reading follows.
///
/// A declaration rather than a use: `fn plugin_secrets` is a second account being
/// built and `plugin::secrets(..)` at a call site is that same one being read, so
/// catching the declaration catches it once instead of once per caller.
const DECLARED: &[&str] = &["struct ", "enum ", "fn ", "trait ", "type ", "mod "];

/// One declaration that would be a second account, and what it is an account of.
#[derive(Debug, PartialEq, Eq)]
struct Second {
    /// The declared name.
    named: String,
    /// What it would be a second account of.
    of: &'static str,
    /// Where the first one is.
    kept: &'static str,
}

/// Every declaration in one file that names a plugin and an account together.
fn second_accounts(text: &str) -> Vec<Second> {
    let mut found = Vec::new();
    for name in declarations(text) {
        let lowered = name.to_lowercase();
        if !BROUGHT.iter().any(|word| lowered.contains(word)) {
            continue;
        }
        for account in ACCOUNTS {
            if account.spelled.iter().any(|word| lowered.contains(word)) {
                found.push(Second {
                    named: name.clone(),
                    of: account.named,
                    kept: account.kept,
                });
                break;
            }
        }
    }
    found
}

/// What may stand between a visibility and the keyword.
///
/// `unsafe` is forbidden crate-wide and is here anyway: this rule is read by whoever
/// is adding a declaration, and one that silently stopped looking at a shape the
/// compiler still accepts would be a gap nobody could see from the rule.
const MODIFIERS: &[&str] = &["async ", "const ", "unsafe "];

/// The name each declaration in a file declares.
fn declarations(text: &str) -> Vec<String> {
    text.lines().filter_map(declared_on).collect()
}

/// The name one line declares, where it declares one.
///
/// The keyword has to *open* the declaration once a visibility and any modifiers are
/// off the front, which is what keeps the same words inside a doc comment from being
/// read as a declaration — the words this rule watches for are exactly the words its
/// own page is written in.
fn declared_on(line: &str) -> Option<String> {
    let mut rest = without_modifiers(without_visibility(line.trim_start()));
    for keyword in DECLARED {
        let Some(after) = rest.strip_prefix(keyword) else {
            continue;
        };
        rest = after;
        let name: String = rest
            .chars()
            .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
            .collect();
        return (!name.is_empty()).then_some(name);
    }
    None
}

/// The line with any leading visibility taken off.
///
/// Parsed rather than listed, because the restricted forms are open-ended:
/// `pub(crate)`, `pub(super)` and `pub(in crate::app)` are all one, and a rule that
/// enumerated the first two would be a rule somebody gets past by writing the third.
fn without_visibility(line: &str) -> &str {
    let Some(rest) = line.strip_prefix("pub") else {
        return line;
    };
    let rest = match rest.strip_prefix('(') {
        Some(scoped) => match scoped.find(')') {
            Some(close) => rest.get(close + 2..).unwrap_or(""),
            None => return line,
        },
        None => rest,
    };
    // A visibility is a whole word, so what follows it is a space. Without that,
    // `published` would read as `pub` and leave `lished` to be matched against the
    // keywords — which finds nothing today and is the sort of thing that finds
    // something later.
    rest.strip_prefix(' ').unwrap_or(line)
}

/// The line with any leading modifiers taken off, however many there are.
fn without_modifiers(line: &str) -> &str {
    let mut rest = line;
    let mut peeling = true;
    while peeling {
        peeling = false;
        for modifier in MODIFIERS {
            if let Some(after) = rest.strip_prefix(modifier) {
                rest = after;
                peeling = true;
                break;
            }
        }
    }
    rest
}

/// Nothing shipped declares a second account for what a plugin holds.
///
/// The direction that would go unnoticed, because a second account works. It answers
/// the question it was built for, its own tests pass, and the listing an operator
/// already knows quietly stops being the whole answer.
#[test]
fn nothing_declares_a_second_account_for_what_a_plugin_brings() {
    // One crawl, read twice: the floors below and the sweep have to be about the same
    // tree, and two calls are two answers to what this workspace contains.
    let shipped = shipped();
    assert!(
        shipped.len() > 10,
        "the crawl read {} files, which means it is reading the wrong tree",
        shipped.len()
    );
    let counted: usize = shipped.values().map(|text| declarations(text).len()).sum();
    assert!(
        counted > 100,
        "the crawl found {counted} declarations across {} files, which means it is \
         reading them wrong and would find nothing whatever the tree held",
        shipped.len()
    );

    let mut refused = Vec::new();
    for (path, text) in &shipped {
        for second in second_accounts(text) {
            refused.push(format!(
                "  {path} declares `{}` — a second account of {}, which is kept in {}",
                second.named, second.of, second.kept
            ));
        }
    }
    assert!(
        refused.is_empty(),
        "a plugin's things belong on the account that already exists, attributed to \
         the plugin, rather than on one of their own:\n{}",
        refused.join("\n")
    );
}

/// Each account this names is one the workspace still keeps.
///
/// The honesty half. A moved or renamed listing leaves a row pointing at nothing, and
/// a refusal that sends somebody to a file that is not there is worse than none —
/// they conclude the rule is stale and go around it.
#[test]
fn every_account_this_names_is_one_this_workspace_still_keeps() {
    let root = workspace_root();
    assert!(!ACCOUNTS.is_empty(), "there is nothing to hold anything to");
    for account in ACCOUNTS {
        assert!(
            root.join(account.kept).is_file(),
            "the rule sends a second account of {} to {}, which this workspace no \
             longer carries — point it at wherever that listing went",
            account.named,
            account.kept
        );
        assert!(
            !account.spelled.is_empty(),
            "{} is watched for no spelling at all, so nothing can ever match it",
            account.named
        );
    }
}

/// The refusing half, one planted declaration per account.
///
/// Driven over fixtures rather than over the tree, because the defect this exists to
/// catch is one the tree does not contain — and a rule proved only by a tree that is
/// already clean is a rule proved by nothing.
#[test]
fn the_rule_refuses_a_second_account_of_each_of_the_three() {
    for account in ACCOUNTS {
        let spelling = first(account.spelled);
        let planted = planted(&format!("Plugin{spelling}s"));
        // Compared as one whole value rather than a count, a first element and three
        // fields. It says what arrived and what was wanted in one line, it needs no
        // arm for a list of one that has no first, and it keeps the read-back out of
        // a failure message — which matters here more than it usually would: the
        // declaration a case plants is named after the word the account is watched
        // for, so a scanner reading a panic format sees a thing called *secret* on
        // its way into a log, and no comment will talk it out of that.
        let read: Vec<(String, &str, &str)> = second_accounts(&planted)
            .into_iter()
            .map(|second| (second.named.to_lowercase(), second.of, second.kept))
            .collect();
        assert_eq!(
            read,
            vec![(format!("plugin{spelling}s"), account.named, account.kept)]
        );
    }
}

/// Every shape a second account might be declared in is read as one.
///
/// A rule that caught the struct and not the function would be a rule somebody gets
/// past by writing a function, which is the easier of the two to write.
#[test]
fn a_second_account_is_refused_whatever_it_is_declared_as() {
    for shape in [
        "pub struct PluginCredentials {",
        "pub fn plugin_secrets() -> Vec<String> {",
        "enum PluginOutbound {",
        "pub(crate) fn plugin_hosts() {",
        "pub type PluginSettings = Vec<String>;",
        "pub mod plugin_credentials;",
        "pub async fn plugin_secrets() {",
        "    pub(crate) const fn plugin_settings() -> u8 {",
        "pub(in crate::app) fn plugin_credentials() {",
        "pub(super) struct PluginOutbound;",
    ] {
        assert_eq!(
            second_accounts(shape).len(),
            1,
            "`{shape}` is a second account and was not read as one"
        );
    }
}

/// The accepting half, and the one a rule like this usually gets wrong.
///
/// Every line here is the change the requirement actually asks for — a plugin named
/// *on* the listing that already exists — and a rule that refused any of them would
/// be refusing the answer while reporting that it was working.
#[test]
fn attribution_on_the_account_that_already_exists_is_not_a_second_one() {
    let attributed = "\
/// One credential this stack holds, and which plugin brought it.
pub struct Held {
    pub name: String,
    pub origin: Origin,
}

/// Every credential, the plugin ones among them rather than beside them.
pub fn taken(plugin: Option<&str>) -> Vec<Held> {
    let _ = plugin;
    Vec::new()
}

/// Where a value came from: this build, the operator, or a named plugin.
pub enum Origin {
    Bundled,
    Operator,
    Plugin { named: String },
}

/// The hosts this stack reaches, each attributed to whatever declared it.
pub fn leaving() -> Vec<Elsewhere> {
    Vec::new()
}

/// One setting, with the plugin that set it named beside its value.
pub struct SettingReport {
    pub origin: Origin,
}

/// What a plugin declares, which is not an account of anything.
pub struct PluginManifest {
    pub id: String,
}

/// Reading a plugin's own claims is not a listing of secrets or hosts.
pub fn plugin_claims() {}
";
    assert_eq!(
        second_accounts(attributed),
        Vec::new(),
        "attributing a plugin on the listing that already exists is the answer, not \
         the defect"
    );
}

/// Prose is not a declaration.
///
/// The words this rule watches for are exactly the words its own module documentation
/// is written in, so a reading that took any line containing them would refuse every
/// file that explained itself.
#[test]
fn a_comment_naming_a_plugin_and_an_account_declares_nothing() {
    let prose = "\
//! A plugin's secrets appear on the credentials listing, and a plugin's hosts on the
//! account of what leaves this machine. There is no plugin settings listing and there
//! is not going to be one.

// struct PluginCredentials would be a second account, which is why this says so.
/// See also: the plugin secret and plugin outbound discussion above.
pub struct Held {}
";
    assert_eq!(second_accounts(prose), Vec::new(), "{prose}");
}

/// The first spelling an account is watched for, which is the one a planted
/// declaration is built from.
fn first<'a>(spelled: &[&'a str]) -> &'a str {
    let Some(word) = spelled.first() else {
        unreachable!("every account is watched for at least one spelling")
    };
    word
}

/// A file declaring one type by the given name, and nothing else.
fn planted(named: &str) -> String {
    format!("pub struct {named} {{\n    pub held: Vec<String>,\n}}\n")
}
