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
    /// Raised where a change would leave a setting holding a value outside its choices.
    OUTSIDE_ITS_CHOICES = "SETTING-7" {
        severity: Error,
        status: 400,
        leaves: Validation,
        since: "0.18.0",
        meaning: "A setting would hold a value that is not one of its choices, given what the \
            other settings it depends on would hold. Nothing was changed; the message names \
            the setting, the value and the choices it has.",
        remedy: "Give one of the choices the message names, moving the settings it depends \
            on in the same act where they have to change too.",
    }
    /// Raised where a service is held back until the operator gives what it needs to start.
    HELD_BACK = "SETTING-8" {
        severity: Warning,
        status: 409,
        since: "0.18.0",
        meaning: "A service is held back until the operator gives what it needs to start: a \
            setting only the operator holds, a secret nobody has entered, or a setting holding \
            a value outside its choices. The rest of the stack runs, and so does nothing that \
            runs through the service; the message names each service and what it waits for.",
        remedy: "Give each setting the message names with lemonfiber settings set; the act that \
            gives the last thing a service waits for starts it.",
    }
    /// Raised where `PLEX_LINK_MODE` would enter or leave `owner` for a request service already
    /// set up.
    OWNER_SET_UP = "SETTING-9" {
        severity: Error,
        status: 409,
        leaves: Validation,
        since: "0.18.0",
        meaning: "PLEX_LINK_MODE would enter or leave owner, which changes which Plex account \
            the request service's owner is, and the request service is already set up. It \
            keeps the owner it was set up with and lemonfiber never sets it up again, so \
            nothing was changed.",
        remedy: "Keep the mode, or move between managed and fronted, which share an owner. \
            Entering or leaving owner takes setting the request service up again.",
    }
    /// Raised on every diagnosis while `PLEX_LINK_MODE` is `owner`, as a standing notice.
    OWNER_HELD = "SETTING-10" {
        severity: Warning,
        status: 409,
        since: "0.18.0",
        meaning: "PLEX_LINK_MODE is owner, so the request service, a household-tier service, \
            holds a token of your own Plex account. Anything that took the request service \
            over could act as that account at plex.tv, claiming and sharing servers and \
            reading the account. You chose this, and it is said on every diagnosis while it \
            stands.",
        remedy: "Signing the request service's device out at plex.tv takes the token back. \
            Only before the request service is set up can the mode move to managed or \
            fronted, which hold no token of yours.",
    }
}
