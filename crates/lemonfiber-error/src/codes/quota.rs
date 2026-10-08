codes! {
    /// Raised where the request service would not answer, so nothing was changed.
    UNREACHABLE = "QUOTA-1" {
        severity: Error,
        status: 500,
        since: "0.12.0",
        meaning: "The request service would not answer, so nothing was changed. What was in \
            force before is still in force.",
        remedy: "Check the request service is running, then run this again.",
    }
    /// Raised where a policy that lives inside a limit was chosen without one.
    NO_LIMIT = "QUOTA-2" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "A policy that lives inside a limit was chosen without one. Living within a \
            limit needs a limit.",
        remedy: "Say how many requests a period allows, and how long the period is.",
    }
    /// Raised where the request named is not one that is waiting on anybody.
    NOT_WAITING = "QUOTA-4" {
        severity: Error,
        status: 404,
        since: "0.12.0",
        meaning: "The request named is not one that is waiting on anybody, so there is nothing \
            to rule on.",
        remedy: "Ask what the household has asked for, to see what is still waiting.",
    }
    /// Raised where a request was turned down and the reason said nothing.
    NO_REASON = "QUOTA-5" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "A request was turned down and the reason given was blank. The reason is passed \
            on to whoever asked, so a blank one tells them nothing.",
        remedy: "Say why in a few words, and pass them on to whoever asked.",
    }
    /// Raised where nobody in the household goes by the name that was given.
    NOBODY = "QUOTA-6" {
        severity: Error,
        status: 404,
        since: "0.12.0",
        meaning: "Nobody in this household goes by the name that was given, so nothing was \
            changed.",
        remedy: "Name somebody who is here. The message lists the household.",
    }
    /// Raised where the request service holds no account for somebody who has one here.
    NEVER_HERE = "QUOTA-7" {
        severity: Warning,
        status: 404,
        since: "0.12.0",
        meaning: "The request service holds no account for somebody who has one here. It learns \
            of somebody the first time they sign in to it, and until then there is no account of \
            theirs for a limit to sit on — what the household is held to applies to them \
            meanwhile.",
        remedy: "Ask them to open the request service once, then set this again.",
    }
    /// Raised where a run was asked to close what has waited too long and the household has
    /// never said how long that is.
    NOTHING_AGREED = "QUOTA-8" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "A run was asked to close what has waited too long, and this household has \
            never said how long that is. A request closed against a period nobody named is one \
            nobody agreed to close.",
        remedy: "Say how many days a request may wait, as in `lemonfiber household expiring \
            --after 30`.",
    }
    /// Raised where the period named would close a request nobody was ever reminded about.
    TOO_SOON = "QUOTA-9" {
        severity: Error,
        status: 400,
        since: "0.12.0",
        meaning: "The period named would close a request nobody was ever reminded about. The \
            reminder and the closing are one arrangement, and a request that goes before the \
            reminder is one nobody saw waiting.",
        remedy: "Name a period of a week or more, so the reminder is reached first.",
    }
}
