codes! {
    /// Said where the words searched for are too few or too many to search with.
    NOT_A_SEARCH = "WISH-1" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A search needs at least two letters, and takes at most a hundred.",
        remedy: "Type at least two letters of the title, and no more than a hundred.",
        household: {
            said: "A search needs at least two letters, and takes at most a hundred.",
            remedy: "Type at least two letters of the title, and no more than a hundred.",
        },
    }
    /// Said where the place a search was asked to go on from is not one a page answered.
    NOT_A_PLACE = "WISH-2" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The search was asked to go on from a place no page of it gave.",
        remedy: "Search again from the start.",
        household: {
            said: "That search can't go on from there.",
            remedy: "Search again from the start.",
        },
    }
    /// Said where no title this member may ask for goes by the id and kind named.
    NOT_ASKABLE = "WISH-3" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "There is no such title to ask for. A title nobody knows and a title this \
            member may not ask for are answered alike.",
        remedy: "Search for it, and ask for a title the search shows.",
        household: {
            said: "There's no such title to ask for.",
            remedy: "Search for it, and ask for a title the search shows.",
        },
    }
    /// Said where the house cannot fetch that kind of title, or anything, right now.
    CANNOT_FETCH = "WISH-4" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "The house cannot fetch that kind of title right now.",
        remedy: "Ask whoever runs the house about it.",
        household: {
            said: "The house can't fetch that kind of title right now.",
            remedy: "Ask whoever runs the house about it.",
        },
    }
    /// Said where the title asked for is already in the house.
    ALREADY_HERE = "WISH-5" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "It is already in the house, so there is nothing to ask for.",
        remedy: "Play it from the house.",
        household: {
            said: "It's already in the house, so there's nothing to ask for.",
            remedy: "Play it from your shelf.",
        },
    }
    /// Said where the member has asked for all their allowance lets them, until it frees up.
    NOTHING_LEFT = "WISH-6" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "This member has asked for all they can for now. Their allowance frees up as \
            earlier asks drop out of the count.",
        remedy: "Ask again once the allowance frees up.",
        household: {
            said: "You've asked for all you can for now. Your allowance frees up as earlier asks \
                drop out of the count.",
            remedy: "Ask again once your allowance frees up.",
        },
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
        household: {
            said: "The house has no room for new things right now. That's the house, not your \
                allowance, and whoever runs the house has been told.",
            remedy: "Ask again later.",
        },
    }
    /// Said where what answers the household's asks did not answer, so nothing was asked.
    NOT_WORKING = "WISH-8" {
        severity: Error,
        status: 503,
        since: "0.18.0",
        meaning: "Asking isn't working right now, so nothing was searched or asked for.",
        remedy: "Whoever runs the house has been told. Try again later.",
        household: {
            said: "Asking isn't working right now, and whoever runs the house has been told. \
                Nothing was searched or asked for.",
            remedy: "Try again a little later.",
        },
    }
    /// Said where the house has nothing to search or ask with at all.
    NOT_SET_UP = "WISH-9" {
        severity: Error,
        status: 503,
        since: "0.18.0",
        meaning: "This house isn't set up for asking for new things.",
        remedy: "Ask whoever runs the house about it.",
        household: {
            said: "This house isn't set up for asking for new things.",
            remedy: "Ask whoever runs the house about it.",
        },
    }
    /// Said where a season named is not one the title has, or seasons were named for a film.
    NOT_ITS_SEASON = "WISH-10" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That is not a season this title has. A film has none to choose.",
        remedy: "Choose from the seasons the title shows.",
        household: {
            said: "That isn't a season this title has.",
            remedy: "Choose from the seasons the title shows.",
        },
    }
    /// Said where nobody is named for something only a member can do.
    NOBODY_ASKING = "WISH-11" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "Nobody was named. A search and an ask are a member's own, so they are made \
            for somebody in the household.",
        remedy: "Name the member, as the household lists them.",
        household: fault,
    }
    /// Said where the member named is not somebody in the household.
    NOT_A_MEMBER = "WISH-12" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "Nobody in this household is known by that.",
        remedy: "Name somebody the household lists, by their name or their id.",
        household: fault,
    }
    /// Said where a title a member may ask for has no picture to show them.
    NO_PICTURE = "WISH-13" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "That title has no picture to show.",
        remedy: "The title can still be asked for by its name.",
        household: {
            said: "That title has no picture to show.",
            remedy: "You can still ask for it by its name.",
        },
    }
}
