codes! {
    /// Raised when a key is asked for under a word that cannot name one.
    BAD_NAME = "KEY-1" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "That word cannot name a key. A key's name travels in an address, an alert and \
            the journal, so it is lower-case letters, digits, dots, dashes and underscores, \
            beginning with a letter or a digit, and at most 64 characters.",
        remedy: "Name it as the program that holds it, as in `home-assistant`.",
    }
    /// Raised when a key is asked for with a scope that is none of the three.
    NOT_A_SCOPE = "KEY-2" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "That is not a scope a key can have. A key has exactly one scope, and it \
            decides everything the key admits.",
        remedy: "Give it one of `read`, `act` or `member:<account>`.",
    }
    /// Raised when a key is asked for with a purpose that is none of the three.
    NOT_A_PURPOSE = "KEY-3" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "That is not a purpose a key can carry. A key says what it is for, so the \
            listing can tell keys apart.",
        remedy: "Give it one of `home-assistant`, `mcp` or `other`.",
    }
    /// Raised when the record of keys is there and does not read as keys.
    UNREADABLE = "KEY-4" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The record of this machine's keys is there and does not read as keys. Nothing \
            is written over it, because it may still hold keys somebody depends on, and every \
            key it held is refused until it reads again. The message names the file.",
        remedy: "Look at the file, and move it aside if it is damaged.",
    }
    /// Raised when a key is asked for under a name another key holds.
    NAME_TAKEN = "KEY-5" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "A key by that name already exists, active or revoked. A name identifies one \
            key for good, so an alert or a journal entry naming it always means the same key.",
        remedy: "Mint it under another name.",
    }
    /// Raised when a member's key names an account the household does not hold.
    NO_SUCH_MEMBER = "KEY-6" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "Nobody in the household goes by that name. A member's key admits exactly what \
            that member's own account does, so it is minted for an account the household holds.",
        remedy: "Name them as they appear in the household, as `lemonfiber household` lists it.",
    }
    /// Raised when the household could not be asked about a member's account.
    UNASKED = "KEY-7" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The household could not be asked about that account, because the media server \
            did not answer, so nothing was minted. A member's key is minted for an account the \
            media server holds.",
        remedy: "Try again once the media server is running.",
    }
    /// Raised when a household member asks for a key, or a revoke, that is not theirs
    /// alone.
    NOT_FOR_YOURSELF = "KEY-8" {
        severity: Warning,
        status: 400,
        since: "0.17.0",
        meaning: "A household member mints and revokes only keys of their own. A member's key \
            admits what their own account does and nothing more, so the only key a member may \
            mint or revoke is one scoped to themselves.",
        remedy: "Ask for `member:<your own account>`, or ask whoever looks after this machine.",
    }
    /// Raised when this machine would not supply the bytes a secret is made of.
    NO_SECRET = "KEY-9" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "lemonfiber could not mint a secret for that key. A key's secret is \
            unpredictable bytes this machine supplies, and it would not supply them, so nothing \
            was minted.",
        remedy: "Try again. If it happens twice, the operating system's own random source is at \
            fault.",
    }
    /// Raised when no active key holds the name a revoke gave.
    NO_SUCH_KEY = "KEY-10" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "There is no active key by that name: none was minted under it, or it was \
            revoked already, and the message says when.",
        remedy: "List the keys with `lemonfiber key list`, and revoke one by the name it shows.",
    }
    /// Raised when a household member asks to mint a key and the operator has not
    /// allowed members to.
    MEMBERS_MAY_NOT_MINT = "KEY-11" {
        severity: Warning,
        status: 409,
        since: "0.17.0",
        meaning: "Whoever looks after this machine has not allowed household members to mint \
            keys, so nothing was minted. A member's key carries that member's requests and \
            viewing to whatever program holds it, so minting one is something the operator \
            allows first.",
        remedy: "Ask whoever looks after this machine to allow it, with `lemonfiber config set \
            LEMONFIBER_MEMBER_KEYS on`.",
    }
}
