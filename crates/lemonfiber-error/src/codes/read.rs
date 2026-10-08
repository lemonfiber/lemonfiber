codes! {
    /// Raised where a read was given a parameter its answer has nowhere to put.
    UNWANTED = "READ-1" {
        severity: Error,
        status: 400,
        since: "0.9.0",
        meaning: "The read you asked for takes no parameter by that name. It is refused rather \
            than ignored, because ignoring it would answer a wider question than the one that \
            was asked, and a wider answer reads like the answer.",
        remedy: "Ask again, naming only what this read takes. The message lists them.",
    }
    /// Raised where a parameter carrying one value was given more than once.
    REPEATED = "READ-2" {
        severity: Error,
        status: 400,
        since: "0.9.0",
        meaning: "A parameter that names one thing was given more than once. Which of them was \
            meant is not something this can work out, and answering for one of them would drop \
            the others without saying so.",
        remedy: "Ask again, naming it once.",
    }
    /// Raised where no read goes by the name that was asked for.
    NO_SUCH_READ = "READ-3" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "There is no read by that name. Every read this surface answers is named in the \
            contract, and this name is not one of them.",
        remedy: "Ask for one of the reads the contract names. [The \
            envelope](https://docs.lemonfiber.app/api/the-envelope/) sets them out.",
    }
    /// Raised where a trace was asked for and named nothing to follow.
    NO_TERM = "READ-4" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A trace was asked for and named nothing to follow.",
        remedy: "Ask again, naming what to follow.",
    }
    /// Raised where the season to narrow a trace to is not a number.
    NOT_A_SEASON = "READ-5" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The season to narrow a trace to is not a number.",
        remedy: "Ask again with the season as a number.",
    }
    /// Raised where a setting was asked for by an empty name.
    NO_SETTING = "READ-6" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A setting was asked for by an empty name. Naming no setting asks for every \
            one; an empty name would match none of them, and come back as no such setting about \
            a setting nobody named.",
        remedy: "Name the setting, or leave it off to read every one.",
    }
    /// Raised where a household member was asked for by an empty name.
    NO_MEMBER = "READ-7" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A household member was asked for by an empty name. Naming nobody asks about \
            the whole household; an empty name would match nobody, and read as nobody having \
            asked for anything.",
        remedy: "Name the member, or leave it off to read the whole household.",
    }
    /// Raised where a shelf was asked for and nobody was named whose it is.
    NO_SHELF_WITHOUT_A_MEMBER = "READ-8" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A shelf was asked for and nobody was named whose it is. There is no everybody \
            to fall back to: a shelf is what one account may watch, and no two accounts need \
            hold the same one.",
        remedy: "Ask again, naming whose shelf to read.",
    }
    /// Raised where how many holdings to answer with is not a whole number.
    NOT_A_COUNT = "READ-9" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "How many holdings to answer with is not a whole number above nought. A shelf \
            of no holdings is a request for an answer that says nothing.",
        remedy: "Ask again with a whole number, or leave it off.",
    }
    /// Raised where more holdings were asked for than one read answers with.
    TOO_MANY_AT_ONCE = "READ-10" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "More holdings were asked for than one read answers with. It is refused rather \
            than cut down, because a shorter answer wearing the shape of the whole one reads as \
            the whole shelf.",
        remedy: "Ask again for fewer.",
    }
    /// Raised where a diagnosis was narrowed to a group or check that is not one.
    NO_SUCH_GROUP = "READ-11" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A diagnosis was narrowed to a group of checks, or a check, that is not one. \
            Reading the word as the nearest one would answer something that was not asked.",
        remedy: "Name a group of checks, or one check by the name a finding gives it.",
    }
    /// Raised where a removal was named that is none of the four there are.
    NO_SUCH_REMOVAL = "READ-12" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A removal was named that is none of the four there are: stop, services, \
            configuration or media. Reading the word as the nearest one would answer something \
            that was not asked.",
        remedy: "Name one of the four, or none to read the one that removes nothing.",
    }
    /// Raised where moving forward was asked about and neither stack nor self named.
    NO_UPDATE_OBJECT = "READ-13" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "Moving forward was asked about and neither `stack` nor `self` was named. \
            Neither is the smaller case of the other — one moves your services, the other this \
            program — so answering with either would answer the wrong question.",
        remedy: "Ask again, naming `stack` or `self`.",
    }
    /// Raised where how many log lines to begin with is not a number within the ceiling.
    NOT_A_LINE_COUNT = "READ-14" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "How many log lines to begin with is not a number, or is more than one read \
            begins with. The message names the ceiling.",
        remedy: "Ask again with a number no larger than the message names.",
    }
    /// Raised where a parameter that takes a yes or a no is neither true nor false.
    NOT_A_CHOICE = "READ-15" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "Whether to keep reading is neither true nor false.",
        remedy: "Ask again with `true` or `false`, or leave it off.",
    }
    /// Raised where a household read named a member and asked for the household's
    /// defaults as well.
    MEMBER_AND_DEFAULTS = "READ-16" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A household read named a member and asked for the household's defaults as \
            well. Those are two different answers, and one reply cannot be both.",
        remedy: "Ask again for one of them: a member, or the household's defaults.",
    }
}
