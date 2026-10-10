codes! {
    /// Said where the words searched for are too few or too many to search with.
    NOT_A_SEARCH = "WISH-1" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A search needs at least two letters, and takes at most a hundred.",
        remedy: "Type at least two letters of the title, and no more than a hundred.",
    }
    /// Said where the place a search was asked to go on from is not one a page answered.
    NOT_A_PLACE = "WISH-2" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The search was asked to go on from a place no page of it gave.",
        remedy: "Search again from the start.",
    }
    /// Said where no title this member may ask for goes by the id and kind named.
    NOT_ASKABLE = "WISH-3" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "There is no such title to ask for. A title nobody knows and a title this \
            member may not ask for are answered alike.",
        remedy: "Search for it, and ask for a title the search shows.",
    }
    /// Said where the house cannot fetch that kind of title, or anything, right now.
    CANNOT_FETCH = "WISH-4" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "The house cannot fetch that kind of title right now.",
        remedy: "Ask whoever runs the house about it.",
    }
    /// Said where the title asked for is already in the house.
    ALREADY_HERE = "WISH-5" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "It is already in the house, so there is nothing to ask for.",
        remedy: "Play it from the house.",
    }
    /// Said where the member has asked for all their allowance lets them, until it frees up.
    NOTHING_LEFT = "WISH-6" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "This member has asked for all they can for now. Their allowance frees up as \
            earlier asks drop out of the count.",
        remedy: "Ask again once the allowance frees up.",
    }
    /// Said where the house has no room for new things, which is the house's and nobody's
    /// limit.
    NO_ROOM = "WISH-7" {
        severity: Error,
        status: 503,
        since: "0.18.0",
        meaning: "The house has no room for new things right now. That is the house, not \
            anybody's allowance.",
        remedy: "Whoever runs the house has been told. Ask again later.",
    }
    /// Said where what answers the household's asks did not answer, so nothing was asked.
    NOT_WORKING = "WISH-8" {
        severity: Error,
        status: 503,
        since: "0.18.0",
        meaning: "Asking isn't working right now, so nothing was searched or asked for.",
        remedy: "Whoever runs the house has been told. Try again later.",
    }
    /// Said where the house has nothing to search or ask with at all.
    NOT_SET_UP = "WISH-9" {
        severity: Error,
        status: 503,
        since: "0.18.0",
        meaning: "This house isn't set up for asking for new things.",
        remedy: "Ask whoever runs the house about it.",
    }
    /// Said where a season named is not one the title has, or seasons were named for a film.
    NOT_ITS_SEASON = "WISH-10" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That is not a season this title has. A film has none to choose.",
        remedy: "Choose from the seasons the title shows.",
    }
    /// Said where nobody is named for something only a member can do.
    NOBODY_ASKING = "WISH-11" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "Nobody was named. A search and an ask are a member's own, so they are made \
            for somebody in the household.",
        remedy: "Name the member, as the household lists them.",
    }
    /// Said where the member named is not somebody in the household.
    NOT_A_MEMBER = "WISH-12" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "Nobody in this household is known by that.",
        remedy: "Name somebody the household lists, by their name or their id.",
    }
}
