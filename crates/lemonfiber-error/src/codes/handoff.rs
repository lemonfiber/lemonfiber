codes! {
    /// Said where the hand-off is for nobody: the name is blank, or only spaces.
    NOBODY_NAMED = "HANDOFF-1" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The hand-off is for nobody — the name was blank. The name is the account their \
            device signs in to, so a blank one leads nowhere.",
        remedy: "Give the name they sign in as, as in `lemonfiber household handoff ana`.",
    }
    /// Said where the stack holds no media server for a device to sign in to.
    NO_MEDIA_SERVER = "HANDOFF-2" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "This stack has no media server, so there is nothing for a device to sign in \
            to. A hand-off points somebody's device at the media server and proves it signed in.",
        remedy: "Add a media server to the stack and run setup.",
    }
    /// Said where the media server's own account was never recorded.
    NOT_SET_UP = "HANDOFF-3" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The media server's own account has not been set up yet. Finding somebody's \
            account and the devices signed in to it is done as the administrator, and this \
            machine has not recorded one.",
        remedy: "Run `lemonfiber setup`, so the media server's account is made and recorded.",
    }
    /// Said where the account named administers the media server.
    RUNS_THE_SERVER = "HANDOFF-4" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The account named administers the media server, so it is not one to hand over. \
            This is the account lemonfiber signs in as, and a device handed it could change what \
            everybody else in the household may watch.",
        remedy: "Invite the person under a name of their own, and hand that over.",
    }
}
