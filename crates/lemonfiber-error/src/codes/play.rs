codes! {
    /// Said where the title named is not something the media server could hold.
    NOT_AN_ITEM = "PLAY-1" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That does not name anything the media server holds. A title is named by the \
            identifier the shelf lists it under, which is thirty-two letters and digits.",
        remedy: "Name a title by the `id` the shelf answered with.",
    }
    /// Said where the title is not one this member may watch, or is not there at all.
    NOT_ON_THEIR_SHELF = "PLAY-2" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "That title is not on this member's shelf. Either the household does not hold \
            it, or it is outside what this member may watch, and the two are answered alike.",
        remedy: "Choose a title from the shelf this member is shown.",
    }
    /// Said where what names a member's device is not a device id.
    NOT_A_DEVICE = "PLAY-3" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That does not name a device. A player names the device it plays on with an \
            id it chose once and keeps: eight to sixty-four letters, digits and dashes.",
        remedy: "Send the id the player keeps for this device.",
    }
    /// Said where there is no media server to play from, or it was never set up.
    NOTHING_TO_PLAY_FROM = "PLAY-4" {
        severity: Error,
        status: 503,
        since: "0.18.0",
        meaning: "There is no media server to play from, or its own account was never \
            recorded, so nothing can be asked of it for a member.",
        remedy: "Add a media server to the stack and run `lemonfiber setup`.",
    }
    /// Said where the media server would not answer for a member.
    UNANSWERED = "PLAY-5" {
        severity: Error,
        status: 502,
        since: "0.18.0",
        meaning: "The media server did not answer, so nothing was read or recorded for this \
            member. Nothing was changed.",
        remedy: "Check the media server is running with `lemonfiber status`, then ask again.",
    }
    /// Said where nobody is named for something only a member can be.
    NOBODY_NAMED = "PLAY-6" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "Nobody was named. A grant and a player's progress are a member's own, so \
            they are given for somebody in the household.",
        remedy: "Name the member, as the household lists them.",
    }
    /// Said where the member named is not somebody in the household.
    NOT_IN_THE_HOUSEHOLD = "PLAY-7" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "Nobody in this household is known by that, so there is nobody to grant or \
            to record progress for.",
        remedy: "Name somebody the household lists, by their name or their id.",
    }
    /// Said where the media server will not sign a device in by code.
    SIGNS_NO_DEVICE_IN = "PLAY-8" {
        severity: Error,
        status: 503,
        since: "0.18.0",
        meaning: "The media server has signing devices in by code turned off, so no session \
            can be opened for the member's device without their password. Nothing was changed.",
        remedy: "Turn Quick Connect on in the media server's dashboard, then grant again.",
    }
    /// Said where a title on the member's shelf has no picture of the kind asked for.
    NO_SUCH_PICTURE = "PLAY-9" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "That title has no picture of that kind the media server serves as an image of \
            at most two megabytes, so there is nothing to show.",
        remedy: "Draw the title by its name where it has no picture.",
    }
}
