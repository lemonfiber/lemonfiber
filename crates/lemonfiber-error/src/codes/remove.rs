codes! {
    /// Said where the removal is for nobody: the name is blank, or only spaces.
    NOBODY_NAMED = "REMOVE-1" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "No name was given, so there is nobody to remove. Removing somebody takes the \
            name their account is held under.",
        remedy: "Name the person, as they appear in `lemonfiber household`.",
    }
    /// Said where the stack holds no media server: there is no account to remove.
    NO_MEDIA_SERVER = "REMOVE-2" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "This stack has no media server, so there is no household to remove anybody \
            from. A household member is an account on the media server; without one there is \
            nobody to take away.",
        remedy: "Add a media server to the stack and run setup.",
    }
    /// Said where the media server will not say who it holds.
    UNREADABLE = "REMOVE-3" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The media server would not say who holds an account, so nobody was removed. \
            Removing somebody starts by finding their account, and that read did not answer.",
        remedy: "Check the media server is running, then run this again.",
    }
    /// Said where nobody by that name is in the household.
    NOBODY_HERE = "REMOVE-4" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "Nobody by that name is in this household. Nothing was removed — the name has \
            to match an account the media server holds, though not its capitalisation.",
        remedy: "Run `lemonfiber household` to see who is here.",
    }
    /// Said where the account named administers the server.
    RUNS_THE_SERVER = "REMOVE-5" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The account named administers the media server, so it is not one to remove. \
            The server refuses to be left without an administrator, and this is also the account \
            lemonfiber signs in as.",
        remedy: "Remove a household member instead. To hand the server to somebody else, do it \
            in the media server's own settings first.",
    }
    /// Said where the media server refused to remove the account.
    WOULD_NOT_REMOVE = "REMOVE-6" {
        severity: Error,
        status: 500,
        since: "0.11.0",
        meaning: "The media server would not remove that account, so nothing was removed. \
            Nothing else was touched: the request service is only asked once the media server's \
            account is gone.",
        remedy: "Check the media server is running, then run this again.",
    }
}
