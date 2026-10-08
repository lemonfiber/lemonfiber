codes! {
    /// Raised when the address the surface was asked to serve on cannot be taken.
    ADDRESS_TAKEN = "SERVE-1" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "The address could not be taken, so there is nowhere for a browser to connect. \
            Usually something else on this machine is already listening there.",
        remedy: "Ask for a different port with `lemonfiber ui --port 7171`, or name no port at \
            all and be given a free one.",
    }
    /// Raised when this machine will not supply the randomness a token is made of.
    NO_TOKEN = "SERVE-2" {
        severity: Error,
        status: 500,
        since: "0.9.0",
        meaning: "A token could not be minted for this run. Every request to this surface has to \
            carry a secret only that run knows, and this machine would not supply the \
            unpredictable bytes it is made of.",
        remedy: "Run it again. If it happens twice, the operating system's own random source is \
            at fault.",
    }
    /// Raised when the network was asked for and nothing here can say who is knocking.
    NO_PASSWORD = "SERVE-3" {
        severity: Error,
        status: 500,
        since: "0.10.0",
        meaning: "The web interface was asked to answer your network and no password is set, so \
            it was not offered. This surface can start, stop and reconfigure everything and \
            reaches every password the system holds, so anything on that network could do all of \
            that.",
        remedy: "Set a password and ask again, with `lemonfiber ui --set-password --lan`. Or \
            leave it as it is, and reach it from this machine.",
    }
    /// Raised when serving encrypted was asked for with no port that stays the same.
    UNSETTLED_PORT = "SERVE-4" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "Serving encrypted was asked for with no port named. Serving encrypted is for a \
            paired phone, and a phone keeps the address it was given, so a port chosen afresh on \
            every run is one it would stop reaching the next time this started.",
        remedy: "Name the port, as in `lemonfiber ui --tls --port <port>`.",
    }
    /// Raised when the certificate to serve encrypted with cannot be read or made.
    NO_CERTIFICATE = "SERVE-5" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "It could not serve encrypted. It presents a certificate it keeps beside its \
            configuration, and that certificate could not be read or made; the message says \
            which.",
        remedy: "Replace the certificate with `lemonfiber companion certificate --confirm`, \
            knowing every paired phone will need pairing again.",
    }
    /// Raised when an answer could not be rendered.
    UNRENDERABLE = "SERVE-6" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "An answer could not be rendered. The request was understood and carried out, \
            and what it came to could not be written down as an answer. Nothing about the \
            request was wrong.",
        remedy: "Ask again. If it keeps happening, send a support bundle, made with `lemonfiber \
            support`.",
    }
    /// Raised when this machine will not supply the randomness a job is named with.
    NO_JOB_NAME = "SERVE-7" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "Work that outlives its request could not be named, because this machine would \
            not supply the randomness a job's name is made of. A job with no name is work \
            nothing could ever be told about, so it was not begun, and nothing was changed.",
        remedy: "Ask again. If it keeps happening, send a support bundle, made with `lemonfiber \
            support`.",
    }
    /// Raised when an action's work ended before it had an answer to give.
    UNANSWERED = "SERVE-8" {
        severity: Error,
        status: 500,
        since: "0.18.0",
        meaning: "An action stopped before it had an answer to give. It may have changed \
            something before it stopped, and sending it again runs it again.",
        remedy: "Ask again. If it keeps happening, send a support bundle, made with `lemonfiber \
            support`.",
    }
}
