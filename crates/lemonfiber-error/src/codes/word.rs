codes! {
    /// A word this product does not explain, as a refusal that says what it does.
    ///
    /// Through the error model rather than a bare line, so it carries a code and a way
    /// forward like every other refusal — and the way forward is the list itself, which
    /// is short enough to be the answer rather than a pointer at one.
    ///
    /// It lies in the naming: the word is the whole of what was asked for, and there is
    /// no entry for it. A surface that reported this as its own failure would be telling
    /// a caller to try again at something that will never work.
    UNRECOGNISED = "WORD-1" {
        severity: Error,
        status: 404,
        since: "0.9.0",
        meaning: "The word you asked about is not one this product explains. What it explains is \
            this ecosystem's own vocabulary — the words that are load-bearing and cannot be \
            guessed. Having no entry is not the same as meaning nothing, and nothing is wrong \
            with the stack.",
        remedy: "Ask about one of the words its reports use. The message lists every word it \
            knows.",
    }
}
