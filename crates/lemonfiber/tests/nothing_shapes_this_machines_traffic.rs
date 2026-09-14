//! Nothing shipped reaches for a traffic shaper.
//!
//! Its own file because it is a claim about what this program *is* rather than
//! about how it is arranged. Shaping a host's traffic needs a privilege this
//! program should not hold and a reach over software that is none of its business —
//! the browser, the work laptop's backup, somebody's call. Limits belong inside
//! lemonfiber's own download clients, through those clients' own interfaces.
//!
//! Invisible in a diff, which is the argument for a guard: a shaper reached from
//! one line of one adapter reads like plumbing.

mod source_tree;

use source_tree::{production, sources};

/// Nothing here shapes the machine's traffic, and this is what keeps it that way.
///
/// A bandwidth feature has one obvious cheat in it. The tools that shape a host's
/// traffic are a shell command away, they work on every application at once, and
/// they would make every figure in this product's own report come true without any
/// of the reading back that makes it honest. What they need is a privilege this
/// program should not hold and a reach over software that is none of its business —
/// the browser, the work laptop's backup, somebody's call.
///
/// The rule is that limits are set inside lemonfiber's own download clients through
/// those clients' own interfaces, and nowhere else. That rule is invisible in a
/// diff: a shaper reached from one line of one adapter reads like plumbing and
/// changes what this program *is*. So the names are held against the shipped half of
/// the tree, where anything reaching one would have to appear.
///
/// A word here is refused as a word rather than as a command, so the guard cannot
/// be walked around by building the same call out of pieces — and each is spelled
/// where a shell would have to spell it, with the separators a path or an argument
/// puts around it.
#[test]
fn nothing_shipped_reaches_for_a_traffic_shaper() {
    /// What a host's traffic is shaped with, on the platforms this ships to.
    const SHAPERS: [&str; 6] = [
        "wondershaper",
        "trickle",
        "/sbin/tc",
        "pfctl",
        "dnctl",
        "iptables",
    ];

    let mut reaching: Vec<String> = Vec::new();
    for (path, text) in sources() {
        if path.to_string_lossy().contains("tests") {
            continue;
        }
        let shipped = production(&text);
        for shaper in SHAPERS {
            if shipped.contains(shaper) {
                reaching.push(format!("{} names {shaper}", path.display()));
            }
        }
    }
    assert!(
        reaching.is_empty(),
        "limits belong inside lemonfiber's own download clients and nowhere else: {reaching:?}"
    );
}
