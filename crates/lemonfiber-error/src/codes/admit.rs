codes! {
    /// Raised when a password is too short to stand in front of this.
    TOO_SHORT = "ADMIT-1" {
        severity: Error,
        status: 500,
        since: "0.10.0",
        meaning: "That password is too short to be the one. It is the only thing standing in \
            front of a surface that can start, stop and reconfigure everything, and what is on \
            the other side of it is a program that guesses without getting bored.",
        remedy: "Use at least twelve characters. Several unrelated words are easier to keep and \
            harder to guess than one word with substitutions in it.",
    }
    /// Raised when this machine will not supply the salt a record is made with.
    NO_SALT = "ADMIT-2" {
        severity: Error,
        status: 500,
        since: "0.10.0",
        meaning: "The password could not be recorded. Every stored password is mixed with \
            unpredictable bytes so that two of them are never written down the same way, and \
            this machine would not supply any.",
        remedy: "Try again. If it happens twice, the operating system's own random source is at \
            fault.",
    }
    /// Raised when the two answers were not the same word.
    MISTYPED = "ADMIT-3" {
        severity: Error,
        status: 500,
        since: "0.10.0",
        meaning: "The two passwords typed were not the same, and nothing was changed. It is \
            asked for twice because nothing here can read one back afterwards, so the second \
            answer is the only check there is that the first was typed the way it was meant.",
        remedy: "Set it again, typing the same password both times.",
    }
    /// Raised when a request carried no token, session or key this run admits.
    NOT_ADMITTED = "ADMIT-4" {
        severity: Error,
        status: 403,
        since: "0.17.0",
        meaning: "The request carried no token or session this run admits, and nothing was \
            answered. The sentence does not say which it was: it answers somebody who has proved \
            nothing, and naming what was wrong would tell them what to keep guessing at.",
        remedy: "Sign in again, or open lemonfiber from the address it printed when it started.",
    }
    /// Raised when a request said it came from somewhere this server is not.
    ELSEWHERE = "ADMIT-5" {
        severity: Error,
        status: 403,
        since: "0.17.0",
        meaning: "The request said it came from somewhere this server is not. The address it \
            named, or the page it came from, is not the one this server is listening on, and \
            nothing was answered.",
        remedy: "Reach lemonfiber at the address it is listening on.",
    }
    /// Raised when an account asked for something that is not its to ask for.
    NOT_YOURS = "ADMIT-6" {
        severity: Warning,
        status: 403,
        since: "0.17.0",
        meaning: "This is not something this account may ask for. Nothing is wrong with the \
            account: what was asked for belongs to somebody else, or to whoever looks after this \
            machine.",
        remedy: "Ask whoever looks after this machine, if you need it.",
    }
    /// Raised when the media server could not say whether an account is still one.
    UNCONFIRMED = "ADMIT-7" {
        severity: Warning,
        status: 403,
        since: "0.17.0",
        meaning: "The account could not be checked with the media server, so nobody was \
            identified and nothing was answered. The account has not been removed, the session \
            has not ended, and nothing about the account changed.",
        remedy: "Try again once the media server is running.",
    }
    /// Raised when the password offered at the door was wrong, or none is set.
    NOT_THE_PASSWORD = "ADMIT-8" {
        severity: Error,
        status: 401,
        since: "0.17.0",
        meaning: "That is not the password for this machine. Nothing was opened, and no session \
            was begun. The same is said where no password is set, so whoever is knocking cannot \
            tell whether there is anything here to guess at.",
        remedy: "Check the password and try again.",
    }
    /// Raised when the door has been given too many wrong passwords lately.
    TOO_MANY_ATTEMPTS = "ADMIT-9" {
        severity: Warning,
        status: 429,
        since: "0.17.0",
        meaning: "Too many wrong passwords. The door has stopped looking at passwords for a \
            while, and another attempt now makes the wait longer. The message says how long, and \
            so does the answer's `Retry-After` header.",
        remedy: "Wait as long as it says, then try once more.",
    }
    /// Raised when what was offered at the door is not a password.
    NOT_A_PASSWORD = "ADMIT-10" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "What was offered at the door is not a password. The door reads a password, and \
            a household member's name beside it, and the body carried neither in a form it can \
            read.",
        remedy: "Send the password as the body's `password`.",
    }
    /// Raised when a key arrived from another machine over a connection its pin does
    /// not verify.
    KEY_IN_THE_CLEAR = "ADMIT-11" {
        severity: Error,
        status: 403,
        since: "0.17.0",
        meaning: "A key, or a request to mint one, arrived from another machine over a \
            connection its pin does not verify, and nothing was answered or minted. A key or a \
            secret crosses a network only over the encrypted connection whose certificate the \
            key was minted with, because one sent any other way may have been read on the way.",
        remedy: "Connect over https, pinning the certificate the key was minted with, and revoke \
            the key with `lemonfiber key revoke` if it may have been read.",
    }
    /// Raised when a key asked for something its scope does not reach.
    NOT_FOR_A_KEY = "ADMIT-12" {
        severity: Warning,
        status: 403,
        since: "0.17.0",
        meaning: "The key asked for something its scope does not reach, and nothing was done. A \
            key has one scope — `read`, `act` or one member's own — and admits that and nothing \
            more.",
        remedy: "Call only what the contract lists as callable by a key, or ask whoever looks \
            after this machine.",
    }
    /// Raised when a claim names an invitation that is not open.
    NOT_OPEN = "ADMIT-13" {
        severity: Error,
        status: 401,
        since: "0.18.0",
        meaning: "This invitation can no longer be claimed, and nothing was set. The same is said \
            whether it was claimed already, ran out, was declined or was never one, so a guess \
            learns nothing about which.",
        remedy: "Ask whoever invited you for a new invitation.",
    }
    /// Raised when the password chosen at a claim is shorter than the least this takes.
    SHORT_CHOICE = "ADMIT-14" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The password chosen is too short, and nothing was set. The invitation still \
            stands and can be claimed with a longer one.",
        remedy: "Choose a password of at least twelve characters.",
    }
}
