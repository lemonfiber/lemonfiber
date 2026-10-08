codes! {
    /// Raised when there is nowhere to keep what pairing a phone needs.
    NOWHERE = "PAIR-1" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "There is nowhere on this machine to keep what pairing a phone needs. The \
            certificate a phone pins and the name it knows this stack by are kept beside the \
            configuration, and this machine would not say where its configuration directory is.",
        remedy: "Run it as a user with a home directory, so there is a configuration directory.",
    }
    /// Raised when the web surface has not been served encrypted on the network.
    NOT_SERVED = "PAIR-2" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "lemonfiber has not been served encrypted on your network, so a phone has \
            nothing to reach. A phone refuses an address that presents no certificate, and \
            reaches this machine from the network rather than from here. The material names the \
            port the surface was last served on that way, and it has not been.",
        remedy: "Serve the web interface encrypted and on your network, on a port that stays the \
            same: `lemonfiber ui --lan --tls --port <port>`.",
    }
    /// Raised when the certificate the surface presents cannot be read or made.
    NO_CERTIFICATE = "PAIR-3" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "The certificate this machine presents to a phone could not be read, or none \
            has been made; the message says which. A phone pins that certificate, so it is not \
            made again on its own — a new one is one every paired phone refuses.",
        remedy: "Replace it with `lemonfiber companion certificate --confirm`, knowing every \
            paired phone will need pairing again.",
    }
    /// Raised when this machine has no address a phone could reach it at.
    NO_ADDRESS = "PAIR-4" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "This machine has no address a phone could reach it at. Pairing material names \
            the address a phone reaches, and this machine answers to no name on the network and \
            has none written down.",
        remedy: "Record the address your household reaches this machine at, with `lemonfiber \
            config set HOUSEHOLD_HOST <address>`.",
    }
    /// Raised when the stack's identifier cannot be read or made.
    UNNAMED = "PAIR-5" {
        severity: Error,
        status: 500,
        since: "0.17.0",
        meaning: "This stack's own identifier could not be read or made. A phone knows this \
            stack by an identifier it keeps whatever else changes; the message says what went \
            wrong.",
        remedy: "Check that the configuration directory can be written, and try again.",
    }
}
