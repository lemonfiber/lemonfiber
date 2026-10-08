codes! {
    /// A capability was named that no service in this stack provides.
    NO_SUCH_FILLER = "WIRE-1" {
        severity: Error,
        status: 404,
        since: "0.16.0",
        meaning: "There is no service by that name in this stack, so it cannot fill anything. \
            Nothing was changed.",
        remedy: "List the services with `lemonfiber catalogue`, then see what is wired with \
            `lemonfiber wiring`.",
    }
    /// The service named cannot do the thing it was asked to fill.
    CANNOT_FILL = "WIRE-2" {
        severity: Error,
        status: 400,
        since: "0.16.0",
        meaning: "Two readings under one code. As an error: the service does not declare that \
            capability, and pointing everything that asked at it would point them at something \
            that cannot answer. As an advisory: the service already fills it, so nothing was \
            changed and nothing needed to be.",
        remedy: "See which services declare it with `lemonfiber plugin capabilities`, and what \
            is wired now with `lemonfiber wiring`.",
    }
    /// Nothing in this stack asks for the capability, so a choice would change nothing.
    NOTHING_ASKS = "WIRE-3" {
        severity: Warning,
        status: 400,
        since: "0.16.0",
        meaning: "Nothing in this stack asks for that capability, so choosing who fills it would \
            record a setting no wiring reads. Nothing was changed.",
        remedy: "See what the stack does ask for with `lemonfiber wiring`.",
    }
    /// The setting recording the choice could not be written.
    CHOICE_UNWRITABLE = "WIRE-4" {
        severity: Error,
        status: 500,
        since: "0.16.0",
        meaning: "Which service fills a capability is a setting, and there is no settings file \
            to record it in. Nothing was changed.",
        remedy: "Set this machine up first, with `lemonfiber setup`.",
    }
    /// Raised when a choice answers an offer that was read against a wiring that has
    /// since moved.
    WIRING_MOVED = "WIRE-5" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The choice answers a reading of the wiring that has since moved, or what is \
            installed changed while the choice was being made. Agreeing to it now would be \
            agreeing to something nobody saw, so nothing was changed; the message names what \
            moved.",
        remedy: "Read the choice again, and answer the name it prints.",
    }
    /// Raised when the reason given for a choice is longer than a reason may be, or
    /// holds a line break or another control character.
    UNREASONABLE = "WIRE-6" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The reason given for the choice cannot be recorded with it. A reason is read \
            on one line beside the choice it explains, so it may be at most 280 characters and \
            hold no line break or other control character. Nothing was changed.",
        remedy: "Say why in one shorter line, or make the choice with no reason.",
    }
    /// Raised where the service chosen already fills the capability, so there is nothing
    /// to change.
    ALREADY_FILLS = "WIRE-7" {
        severity: Advisory,
        status: 400,
        since: "0.18.0",
        meaning: "The service asked for already fills the capability, so nothing was changed, \
            and nothing needed to be.",
        remedy: "Nothing to do. `lemonfiber wiring` lists what fills each capability.",
    }
}
