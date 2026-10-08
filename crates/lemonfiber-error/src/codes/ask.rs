codes! {
    /// Raised where no action goes by the name that was asked for.
    NO_SUCH_ACTION = "ASK-1" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "There is no action by that name. An action here is a command the command line \
            offers, and this surface offers nothing else. The message names what was asked for.",
        remedy: "Ask for one of the actions the contract names.",
    }
    /// Raised where an action was not given an argument it needs.
    MISSING_ARGUMENT = "ASK-2" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "An action was not given an argument it needs, so it was not carried out. The \
            message names the action and the argument.",
        remedy: "Ask again with the arguments the action takes.",
    }
    /// Raised where an argument was given a value that names nothing.
    UNRECOGNISED_ARGUMENT = "ASK-3" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "An argument was given a value that names nothing this stack knows. The message \
            names the argument, what it said, and what it could have said instead.",
        remedy: "Ask again with the arguments the action takes.",
    }
    /// Raised where an action was given an argument its command has nowhere to put.
    UNWANTED_ARGUMENT = "ASK-4" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "An action was given an argument its command has nowhere to put. It is refused \
            rather than dropped, because dropping it would carry out a different request from \
            the one asked for.",
        remedy: "Ask again with the arguments the action takes.",
    }
    /// Raised where two arguments that each name a different request arrived together.
    ARGUMENTS_TOGETHER = "ASK-5" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "An action was given two arguments that each name a different request. A run \
            carrying both would have to pick one of them and answer something nobody asked for. \
            The message names the two.",
        remedy: "Ask again with the arguments the action takes, choosing one of the two.",
    }
    /// Raised where the body of an action is not arguments it can read.
    NOT_ARGUMENTS = "ASK-6" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The body of the request is not arguments the action can read. What the reader \
            could not take from it — a field it did not know, or where the text stopped being \
            JSON — is given as the detail.",
        remedy: "Ask again with the arguments the action takes.",
    }
    /// Raised where a job was asked about that this run did not start.
    NO_SUCH_JOB = "ASK-7" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "No work in this run goes by that name. Jobs are named by the run that starts \
            them, and work from an earlier run is not tracked here.",
        remedy: "Ask about a job this run started.",
    }
    /// Raised where the body of a setup step is not an answer it can read.
    NOT_AN_ANSWER = "ASK-8" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The body of a setup step is not one of setup's answers, nor a way out of an \
            interrupted apply, so setup did not move. What arrived is not repeated back, because \
            an answer can carry a credential.",
        remedy: "Answer the question setup is asking.",
    }
    /// Raised where a path under the endpoints is one no endpoint answers.
    NO_ENDPOINT = "ASK-9" {
        severity: Error,
        status: 404,
        since: "0.17.0",
        meaning: "No endpoint answers this path. Every endpoint this surface answers is named in \
            the contract, and this path is not one of them.",
        remedy: "Ask for one of the endpoints the contract names.",
    }
    /// Raised where an endpoint was asked with a method it does not answer.
    WRONG_METHOD = "ASK-10" {
        severity: Error,
        status: 405,
        since: "0.17.0",
        meaning: "The path is one this surface answers, asked with a method it does not answer \
            it with, and nothing was done.",
        remedy: "Ask again with the method the contract names.",
    }
    /// Raised where the body of a mint is not a key's name, scope, purpose and the
    /// password.
    NOT_A_KEY_REQUEST = "ASK-11" {
        severity: Error,
        status: 400,
        since: "0.17.0",
        meaning: "The body of a request to mint a key is not a key's name, scope and purpose \
            with the password, so no key was minted.",
        remedy: "Send the name, scope, purpose and password as the body's four fields.",
    }
    /// Raised where an action's `Idempotency-Key` is not one to 255 visible
    /// characters, or is given more than once.
    NOT_AN_IDEMPOTENCY_KEY = "ASK-12" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "An action's `Idempotency-Key` is not one to 255 visible characters, or was \
            given more than once. The action was not carried out, and nothing was changed: a key \
            that cannot be read is one a second send could not be recognised by.",
        remedy: "Send the key once, as up to 255 visible characters with no spaces.",
    }
    /// Raised where an `Idempotency-Key` already sent with one action and its
    /// arguments is sent with another.
    IDEMPOTENCY_KEY_REUSED = "ASK-13" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "This `Idempotency-Key` was already sent with another action or other \
            arguments. Nothing was carried out under it a second time; the attempt it names \
            asked for something else, and that is what it was answered with.",
        remedy: "Send a new key with each new attempt, and the same key only with the same \
            action sent again.",
    }
}
