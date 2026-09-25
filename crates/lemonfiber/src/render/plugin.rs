//! Everything under the word `plugin`, on a terminal.
//!
//! Two readers, and the pages read differently for each. **For a plugin author** what
//! is wanted is not *is my stack well* but *what may I write down*, so a capability
//! leads with the prose a claimant is held to and the probes a claim has to bind, and
//! a point leads with what a row carries — the parts somebody is about to copy into a
//! manifest. Every one of those pages opens by saying which generation it is
//! reporting: an author comparing what they were told with what their manifest was
//! refused for needs to know whether the difference is their build or their file, and
//! the generation is the only thing that answers that.
//!
//! **For an operator** the question is what a stranger's plugin is doing on their
//! machine, so the install leads with the image and the digest that pins it, and says
//! where each service keeps its state and how it is reached. No generation there: the
//! answer is about this machine rather than about what a manifest may declare.

use lemonfiber_core::filling::Filling;
use lemonfiber_core::plugin::{
    Asserted, Capabilities, Claimed, Claiming, Credential, Evidence, Points, Probe, Provenance,
    Ran, Verdict, Vouched,
};

// The operator's half: what one machine has installed, and what installing one came
// to. Its own file because it answers a different question from a different source —
// this one reads the documents the build publishes, that one reads one machine's
// record — and because two readers in one file is two files nobody split.
mod machine;

pub(crate) use machine::installs;

use super::Lines;

/// A document going out to whatever asked for it, exactly as it is committed.
pub(crate) fn document(text: &str) -> Lines {
    let mut lines = Lines::for_a_parser();
    lines.block(text);
    lines
}

/// What a service can be asked for, and what claiming one undertakes.
pub(crate) fn capabilities(published: &Capabilities) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "The capabilities a service can claim — generation {}, {} of them.",
        published.vocabulary_version,
        published.capabilities.len()
    ));
    for capability in &published.capabilities {
        lines.spaced(capability.name.to_owned());
        lines.put(format!("  {}", capability.summary));
        lines.put(format!("  {}", capability.contract));
        lines.put(format!(
            "  declared by  {}",
            capability.declared_by.join(", ")
        ));
        for probe in capability.probes {
            lines.put(format!("  probe {} — {}", probe.id, probe.title));
            lines.put(format!("    asks     {}", probe.asks));
            lines.put(format!("    asked as {}", asked_as(probe.credential)));
            lines.put(format!("    answers  {}", answers(probe)));
        }
    }
    lines.spaced(
        "A name here is the only kind you may claim unnamespaced. Your own capability is \
         written <plugin-id>:<name>, and nothing asks for one yet.",
    );
    lines
}

/// Who a probe is asked as, in the words an author would use.
fn asked_as(credential: Credential) -> &'static str {
    match credential {
        Credential::None => "anybody, presenting nothing",
        Credential::Operator => "the operator, with the credential they hold",
    }
}

/// What a probe accepts as an answer.
fn answers(probe: &Probe) -> String {
    let statuses: Vec<String> = probe
        .requires
        .status
        .iter()
        .map(ToString::to_string)
        .collect();
    if probe.requires.body.is_empty() {
        // A refusal is the one answer no port proxy can produce, which is why this
        // probe is allowed to constrain nothing but the status. Said rather than left
        // blank, so it does not read as a gap in the document.
        return format!("{} — the status is the whole of it", statuses.join(" or "));
    }
    let kinds: Vec<String> = probe
        .requires
        .body
        .iter()
        .map(|constraint| constraint.as_str().to_owned())
        .collect();
    format!("{}, and one of {}", statuses.join(" or "), kinds.join(", "))
}

/// Where a plugin may put a row, and what a row there carries.
pub(crate) fn points(published: &Points) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!(
        "The places a plugin may extend lemonfiber — generation {}, {} of them.",
        published.extension_points_version,
        published.points.len()
    ));
    for point in &published.points {
        lines.spaced(point.name.to_owned());
        lines.put(format!("  {}", point.summary));
        lines.put(format!("  joins     {}", point.register));
        lines.put(format!("  read by   {}", point.engine));
        lines.put(format!("  asks for  {}", point.requires));
        lines.put(format!("  required  {}", point.row.required.join(", ")));
        lines.put(format!("  optional  {}", point.row.optional.join(", ")));
        for bound in point.row.bounds {
            lines.put(format!(
                "  {} is {} to {}, and {} where a row does not say",
                bound.field, bound.limits.min, bound.limits.max, bound.limits.default
            ));
        }
        for closed in point.row.enums {
            lines.put(format!(
                "  {} is one of  {}",
                closed.field,
                closed.values.join(", ")
            ));
        }
        lines.put(format!("  taken     {}", taken(&point.occupied)));
    }
    lines.spaced(
        "Every identity you contribute is namespaced with your plugin's id, so it cannot \
         collide with one of those.",
    );
    lines
}

/// What is already standing in a register, or that nothing is.
fn taken(occupied: &[String]) -> String {
    if occupied.is_empty() {
        return "nothing yet".to_owned();
    }
    format!("{} — {}", occupied.len(), occupied.join(", "))
}

/// What one plugin's source claims, in whichever form was asked for.
///
/// The machine-readable form is the report as it stands rather than a second shape
/// written beside it: what an author's CI branches on and what a person reads are the
/// same answer, and two renderings of one answer is the disagreement this avoids.
pub(crate) fn claimed(read: &Claimed, json: bool) -> Option<Lines> {
    if json {
        return serde_json::to_string_pretty(read)
            .ok()
            .as_deref()
            .map(document);
    }
    Some(claims(read))
}

/// What anybody has said about the images a plugin pins, in whichever form was asked.
pub(crate) fn vouched(read: &Vouched, json: bool) -> Option<Lines> {
    if json {
        return serde_json::to_string_pretty(read)
            .ok()
            .as_deref()
            .map(document);
    }
    Some(provenance(read))
}

/// Three answers, each said as itself.
///
/// The words are the whole point. *Unproven* is not a softer *refused* and not a
/// quieter *signed*: it is the answer that says nobody has claimed this image, and an
/// operator deciding whether to go on needs it kept apart from a claim that did not
/// hold. So each line says which of the three it is before it says anything else.
fn provenance(read: &Vouched) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} — what its images are vouched for", read.id));
    lines.put(String::new());
    for one in &read.images {
        lines.put(format!("  {}  {}", one.held.as_str(), one.service));
        lines.put(format!("    {}@{}", one.image, one.digest));
        let why = match &one.held {
            Provenance::Signed { by } => format!("signed, and the key that made it is {by}"),
            Provenance::Unproven { why } | Provenance::Refused { why } => why.clone(),
        };
        lines.put(format!("    {why}"));
        lines.put(String::new());
    }
    if read.images.is_empty() {
        lines.put("  it pins no image, so nothing was asked about anything.".to_owned());
        lines.put(String::new());
    }
    lines.put(if read.installable {
        "nothing said about these images stops an install.".to_owned()
    } else {
        "this would not be installed as it stands.".to_owned()
    });
    lines
}

/// What one plugin's source claims, and what this build makes of it.
///
/// The refusals come first and are the whole answer where there are any: a manifest
/// that contradicts what this build publishes is refused outright rather than partly
/// applied, so reporting what its claims would have come to would be describing an
/// install that is not going to happen.
fn claims(read: &Claimed) -> Lines {
    let mut lines = Lines::default();
    lines.put(format!("{} {} — {}", read.id, read.version, read.name));
    lines.put(format!(
        "held to capability vocabulary generation {} and extension points generation {}.",
        read.vocabulary_version, read.extension_points_version
    ));

    if !read.refusals.is_empty() {
        lines.spaced(format!(
            "Refused, {}:",
            counted(read.refusals.len(), "violation")
        ));
        for refusal in &read.refusals {
            lines.put(format!("  {} — {}", refusal.location, refusal.message));
        }
    }

    if read.capabilities.is_empty() {
        lines.spaced("It claims no capabilities, so nothing can ask for what it installs.");
    } else {
        lines.spaced("What it can do");
        for claiming in &read.capabilities {
            capability(&mut lines, claiming);
        }
    }

    if !read.proofs.is_empty() {
        lines.spaced("What must hold before it is installed");
        for proof in &read.proofs {
            asserted(&mut lines, proof);
        }
    }

    if !read.checks.is_empty() {
        lines.spaced("What it would check, every day after");
        for check in &read.checks {
            asserted(&mut lines, check);
        }
        // Said here rather than left to be inferred from the verdicts, because a check
        // its own recording refuses reads exactly like a claim its recordings refute and
        // is the opposite news: the recording is of a machine in the state the check
        // exists to find, and the check found it.
        lines.put(
            "  A check reports on a running stack rather than gating one, so none of these \
             decides whether this is installed."
                .to_owned(),
        );
    }

    if !read.contributions.is_empty() {
        lines.spaced("What it adds to lemonfiber's own registers");
        for row in &read.contributions {
            let about = row
                .about
                .as_ref()
                .map_or_else(String::new, |check| format!("  (for {check})"));
            lines.put(format!("  {} {}{about}", row.at, row.id));
            if !row.says.is_empty() {
                lines.put(format!("    {}", row.says));
            }
        }
    }

    // A match rather than a sentence written here, so that the day a verdict can come
    // from a service this stops compiling instead of going on saying the wrong thing.
    // And said on every run rather than only on the one that passed: the refused
    // report is the one whose reader is about to go and change their service, and it
    // was the report that never told them nothing had been asked of it.
    lines.spaced(match read.against {
        Evidence::Recordings => {
            "No service was asked anything: every verdict above is against the \
             recordings this plugin ships."
        }
        // Not reachable from this page and written anyway, because the alternative is
        // a wildcard — and a wildcard here is what would let a verdict reached against
        // a service be shown under a sentence saying none was asked.
        Evidence::Service => {
            "Every verdict above is against the service itself, running on this \
             machine and asked."
        }
    });
    lines.put(if read.installable {
        "Nothing here stops it being installed."
    } else {
        "As it stands this would not be installed."
    });
    lines
}

/// One proof or contributed check, and what its recording came to.
fn asserted(lines: &mut Lines, one: &Asserted) {
    lines.put(format!("  {} — {}", one.id, one.says));
    lines.put(format!("    {} — {}", one.service, came_to(&one.verdict)));
}

/// One capability, its probes, and what asking for it would come to.
fn capability(lines: &mut Lines, claiming: &Claiming) {
    // The service, always, rather than only where a plugin declares two. A reader
    // holding a report with one line per capability has no way to tell which container
    // answered, and a plugin may declare two services that both offer its own
    // namespaced name — which is two identical lines if this says nothing.
    lines.put(format!(
        "  {}  [{}]  on {}",
        claiming.name,
        claiming.shown.as_str(),
        claiming.service
    ));
    for ran in &claiming.probes {
        lines.put(format!("    probe {} — {}", ran.probe, verdict(ran)));
    }
    if !claiming.shown.can_fill() && claiming.filling.is_some() {
        // What follows is what asking for the capability comes to, and this claim is not
        // part of it. Said before the line rather than after, because a list of
        // claimants under a refuted claim otherwise reads as the claim standing.
        lines.put(format!(
            "    this claim is {} and does not fill it",
            claiming.shown.as_str()
        ));
    }
    // Which of the two kinds this is decides the line, rather than whether there is an
    // answer about what fills it. There is no answer for a core name the vocabulary
    // does not carry either, and calling that one *this plugin's own* would describe a
    // capability the refusal above has just said does not exist.
    if claiming.own {
        lines.put(
            "    this plugin's own, and inert: nothing asks for it, and no published \
             contract defines it"
                .to_owned(),
        );
        return;
    }
    match &claiming.filling {
        None => lines
            .put("    nothing published carries this name, so nothing can ask for it".to_owned()),
        Some(Filling::By { service }) => {
            lines.put(format!("    asking for it reaches {service}"));
        }
        Some(Filling::Contested { claimants }) => {
            lines.put(format!(
                "    contested between {} — nothing wires to it until the operator chooses, and \
                 lemonfiber does not choose by install order",
                claimants.join(", ")
            ));
        }
        Some(Filling::Unfilled) => lines.put(
            "    nothing fills it: this claim is not one that can, and nothing else claims it"
                .to_owned(),
        ),
    }
}

/// What one probe came to, in one line.
fn verdict(ran: &Ran) -> String {
    came_to(&ran.verdict)
}

/// What one verdict came to, in one line.
///
/// Named for what it answers rather than for `reached`, which is the name the
/// service-side renderer below has and is about being reachable. Two functions
/// called the same thing in one file, taking different types and meaning
/// different things, is a rename waiting to be made by whoever next reads only
/// one of them.
fn came_to(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Passed => "the recording answers it".to_owned(),
        Verdict::Failed { faults } => format!("refuted: {}", faults.join("; ")),
        Verdict::Unproven { why } => format!("unproven: {why}"),
    }
}

/// A count and the thing counted, pluralised where it is not one.
fn counted(number: usize, thing: &str) -> String {
    if number == 1 {
        format!("{number} {thing}")
    } else {
        format!("{number} {thing}s")
    }
}

#[cfg(test)]
mod tests;
