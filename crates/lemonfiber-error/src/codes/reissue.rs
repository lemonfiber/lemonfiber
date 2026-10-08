codes! {
    /// Said where the media server will not say who holds an account.
    UNREADABLE = "REISSUE-1" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The media server would not say who holds an account, so nothing was reset. \
            Making an account claimable again starts by finding it, and that read did not \
            answer.",
        remedy: "Check the media server is running, then run this again.",
    }
    /// Said where nobody by that name is in the household.
    NOBODY_HERE = "REISSUE-2" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "Nobody by that name is in this household. Nothing was reset — the name has to \
            match an account the media server holds, though not its capitalisation.",
        remedy: "Run `lemonfiber household` to see who is here.",
    }
    /// Said where the account named administers the server.
    RUNS_THE_SERVER = "REISSUE-3" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The account named administers the media server, so its password is not one to \
            reset. This is the account lemonfiber signs in as, and taking its password away \
            would leave nothing to sign in with.",
        remedy: "Reset a household member instead. To change the administrator's own password, \
            do it in the media server's settings.",
    }
    /// Said where the media server refused to make the account claimable again.
    WOULD_NOT_REISSUE = "REISSUE-4" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The media server would not reset that password, so nothing changed. Their \
            existing password still works and the account is untouched.",
        remedy: "Check the media server is running, then run this again.",
    }
}
