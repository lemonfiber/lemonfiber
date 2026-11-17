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
        household: fault,
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
        household: fault,
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
        household: fault,
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
        household: {
            said: "You're not signed in to the house.",
            remedy: "Sign in again.",
        },
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
        household: {
            said: "The house didn't recognise where this came from.",
            remedy: "Open the house from the place you were given.",
        },
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
        household: {
            said: "That isn't yours to open.",
            remedy: "Ask whoever runs the house if you need it.",
        },
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
        household: {
            said: "Your library isn't working right now, and whoever runs the house has been told. \
                You're still signed in.",
            remedy: "Try again a little later.",
        },
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
        household: {
            said: "That name and password don't match.",
            remedy: "Check them and try again.",
        },
    }
    /// Raised when the door has been given too many wrong passwords lately.
    TOO_MANY_ATTEMPTS = "ADMIT-9" {
        severity: Warning,
        status: 429,
        since: "0.17.0",
        meaning: "Too many wrong passwords. The door has stopped looking at passwords for a \
            while, and another attempt now makes the wait longer. The message says how long, and \
            so does the answer's Retry-After header.",
        remedy: "Wait as long as it says, then try once more.",
        household: {
            said: "Too many tries didn't match, so the house is waiting a while.",
            remedy: "Wait a little, then try once more.",
        },
    }
    /// Raised when what was offered at the door is not a password.
    NOT_A_PASSWORD = "ADMIT-10" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "What was offered at the door is not a password. The door reads a password, and \
            a household member's name beside it, and the body carried neither in a form it can \
            read.",
        remedy: "Send the password as the body's password.",
        household: {
            said: "That sign-in couldn't be read.",
            remedy: "Enter your name and password, and try again.",
        },
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
            the key with lemonfiber key revoke if it may have been read.",
        household: {
            said: "This app reached the house over a connection that isn't private, so nothing was \
                sent.",
            remedy: "Connect again from the app you paired with the house.",
        },
    }
    /// Raised when a key asked for something its scope does not reach.
    NOT_FOR_A_KEY = "ADMIT-12" {
        severity: Warning,
        status: 403,
        since: "0.17.0",
        meaning: "The key asked for something its scope does not reach, and nothing was done. A \
            key has one scope — read, act or one member's own — and admits that and nothing \
            more.",
        remedy: "Call only what the contract lists as callable by a key, or ask whoever looks \
            after this machine.",
        household: {
            said: "That isn't something this app may do for you.",
            remedy: "Ask whoever runs the house if you need it.",
        },
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
        household: {
            said: "This invitation can't be used any more.",
            remedy: "Ask whoever invited you for a new one.",
        },
    }
    /// Raised when the password chosen at a claim is shorter than the least this takes.
    SHORT_CHOICE = "ADMIT-14" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The password chosen is too short, and nothing was set. The invitation still \
            stands and can be claimed with a longer one.",
        remedy: "Choose a password of at least twelve characters.",
        household: {
            said: "That password is too short.",
            remedy: "Choose one of at least twelve characters.",
        },
    }
    /// Raised when a sign-in by a code names a handle that waits on nothing for this caller.
    CODE_NOT_WAITING = "ADMIT-15" {
        severity: Error,
        status: 401,
        since: "0.18.0",
        meaning: "This sign-in by a code can no longer be finished, and no session was begun. The \
            same is said whether the code lapsed, was used already, was asked for by another \
            session or was never one, so a guess learns nothing about which.",
        remedy: "Ask for a new code and approve it before it lapses.",
        household: {
            said: "That sign-in code can't be used any more.",
            remedy: "Ask for a new code and try again.",
        },
    }
    /// Raised when one source has asked for as many codes to sign in by as it may.
    TOO_MANY_CODES = "ADMIT-16" {
        severity: Warning,
        status: 429,
        since: "0.18.0",
        meaning: "This device or session has asked for as many sign-in codes as it may for now, \
            so no code was asked for. Other devices are still given codes. The message says how \
            long is left, and so does the answer's Retry-After header.",
        remedy: "Wait as long as it says, or finish a code already shown.",
        household: {
            said: "Too many sign-in codes were asked for, so the house is waiting a while.",
            remedy: "Wait a little, then try once more.",
        },
    }
    /// Raised when a code to sign in by is asked of a household whose members sign in with a
    /// password.
    SIGNS_IN_BY_PASSWORD = "ADMIT-17" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "This household keeps its own accounts, so its members sign in with their name \
            and password, and no code was asked for.",
        remedy: "Sign in with the member's name and password.",
        household: {
            said: "This house signs in with your name and password.",
            remedy: "Enter your name and password instead.",
        },
    }
    /// Raised when what was sent to ask for or finish a sign-in by a code cannot be read.
    NOT_A_CODE_REQUEST = "ADMIT-18" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "What was sent to sign in by a code cannot be read. An empty object asks for a \
            code, and an object naming the handle as waiting finishes one.",
        remedy: "Send {} to ask for a code, or the handle as waiting to finish one.",
        household: {
            said: "That sign-in couldn't be read.",
            remedy: "Try again.",
        },
    }
}
