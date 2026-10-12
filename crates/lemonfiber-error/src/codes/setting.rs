codes! {
    /// Raised where nothing in this stack reads a setting by the name given.
    NO_SUCH_SETTING = "SETTING-1" {
        severity: Error,
        status: 404,
        leaves: Validation,
        since: "0.18.0",
        meaning: "No service in this stack reads a setting by that name, and it is not one of \
            the core's own, so writing it would record a value nothing reads. Nothing was \
            changed.",
        remedy: "List the settings with lemonfiber settings.",
    }
    /// Raised where the name given is a secret's, which this command never writes.
    A_SECRET = "SETTING-2" {
        severity: Error,
        status: 400,
        leaves: Validation,
        since: "0.18.0",
        meaning: "That name is a secret's. A secret is entered where it is first needed and \
            replaced at the terminal, never written as a setting, so nothing was changed.",
        remedy: "Replace it at the terminal with lemonfiber credentials --rotate.",
    }
    /// Raised where the name given belongs to the other of the two commands that write
    /// settings.
    TAKEN_ELSEWHERE = "SETTING-3" {
        severity: Error,
        status: 400,
        leaves: Validation,
        since: "0.18.0",
        meaning: "One name has one writer. That name is taken by the other of lemonfiber \
            settings and lemonfiber config, so nothing was changed; the message names the \
            command and the name that take it.",
        remedy: "Run the command the message names.",
    }
    /// Raised where a value cannot be written as the setting asked for.
    UNWRITABLE_VALUE = "SETTING-4" {
        severity: Error,
        status: 400,
        leaves: Validation,
        since: "0.18.0",
        meaning: "The value cannot be written as that setting: a setting is one line with no \
            control character, and a core setting is held to the form its table names. \
            Nothing was changed.",
        remedy: "Give the value on one line, in the form the message names.",
    }
    /// Raised when a change answers an offer read against settings that have since moved.
    SETTINGS_MOVED = "SETTING-5" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The change answers a reading of the settings that has since moved, or a \
            document it regenerates changed while it was being made. Agreeing to it now would \
            be agreeing to something nobody saw, so nothing was changed; the message names \
            what moved.",
        remedy: "Read the change again, and answer the name it prints.",
    }
    /// Raised where the setting already holds what the change asks for.
    ALREADY_HOLDS = "SETTING-6" {
        severity: Advisory,
        status: 400,
        since: "0.18.0",
        meaning: "The setting already holds what was asked for, so nothing was changed, and \
            nothing needed to be.",
        remedy: "Nothing to do. lemonfiber settings lists what each setting holds.",
    }
}
