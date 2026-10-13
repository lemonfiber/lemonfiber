codes! {
    /// Raised when a device key is asked for under a word that cannot name one.
    NOT_A_DEVICE_NAME = "DEVICE-1" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That word cannot name a device key. A device key's name travels in an \
            address, an alert and the journal, so it is lower-case letters, digits, dots, dashes \
            and underscores, beginning with a letter or a digit, and at most 64 characters.",
        remedy: "Name it after the phone that holds it, as in pixel-8.",
    }
    /// Raised when a device key is registered under a name another device key holds.
    DEVICE_NAME_TAKEN = "DEVICE-2" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "A device key by that name is already registered, or was and has been revoked. \
            A name identifies one device key for good, so an alert or a journal entry naming \
            it means the same key.",
        remedy: "Register it under another name.",
    }
    /// Raised when a device key is registered with a public half another device key holds.
    PUBLIC_HALF_TAKEN = "DEVICE-3" {
        severity: Error,
        status: 409,
        since: "0.18.0",
        meaning: "That public half is already registered, under the name the message gives. A \
            revoked device key is never made good again, so its public half is not registered \
            a second time.",
        remedy: "Make a new key pair on the phone and register that.",
    }
    /// Raised when the public half given is not a P-256 key.
    NOT_P256 = "DEVICE-4" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "That public half is not a P-256 key. A device key is the public half of a \
            P-256 key pair, as its SubjectPublicKeyInfo in base64url.",
        remedy: "Make the key pair on the P-256 curve, and send its SubjectPublicKeyInfo.",
    }
    /// Raised when a device key is registered with no attestation.
    NO_ATTESTATION = "DEVICE-5" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "A device key was registered with no attestation. A key nothing vouches for \
            is treated as one held in software, which a device key may not be.",
        remedy: "Send the platform's attestation of where the key lives with it.",
    }
    /// Raised when an attestation does not verify, or does not cover the public half.
    ATTESTATION_UNVERIFIED = "DEVICE-6" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The attestation does not verify against the platform roots this build \
            ships, or it does not cover the public half that was sent, so nothing vouches for \
            where the key lives.",
        remedy: "Make a new key pair on the phone and register it with the attestation the \
            platform gives for it.",
    }
    /// Raised when an attestation places the key in software, or says it was not made in
    /// the hardware that keeps it.
    IN_SOFTWARE = "DEVICE-7" {
        severity: Error,
        status: 400,
        since: "0.18.0",
        meaning: "The attestation places the key in software, or says it was made outside the \
            hardware and taken in. A device key has to be made and kept in the phone's \
            hardware, StrongBox, the Secure Enclave or the trusted environment, where the \
            operating system cannot read it.",
        remedy: "Make the key on a phone that holds keys in hardware, or mint the key at the \
            stack's terminal.",
    }
    /// Raised when an attestation names an app this stack does not trust.
    UNTRUSTED_APP = "DEVICE-8" {
        severity: Error,
        status: 403,
        since: "0.18.0",
        meaning: "The attestation names an app this stack does not trust. A device key is \
            registered only from the official companion or from a build the operator has \
            added to LEMONFIBER_DEVICE_KEY_APPS.",
        remedy: "Add the build's identity with lemonfiber settings set \
            LEMONFIBER_DEVICE_KEY_APPS, if it is a build you made.",
    }
    /// Raised when a device key named is not registered, or was revoked.
    NOT_REGISTERED = "DEVICE-9" {
        severity: Error,
        status: 404,
        since: "0.18.0",
        meaning: "No active device key goes by that name: none was registered under it, or it \
            was revoked.",
        remedy: "Register the phone's key again, which asks for the password once.",
    }
    /// Raised when a mint carries a challenge that does not stand.
    CHALLENGE_NOT_STANDING = "DEVICE-10" {
        severity: Warning,
        status: 403,
        since: "0.18.0",
        meaning: "That challenge proves nothing: it was never issued, it was used already, \
            its minute has passed, or it was issued to another session or device key. Which \
            of these is not said.",
        remedy: "Ask for a new challenge and sign the mint within a minute.",
    }
    /// Raised when a mint's signature does not verify against the device key.
    SIGNATURE_WRONG = "DEVICE-11" {
        severity: Error,
        status: 401,
        since: "0.18.0",
        meaning: "The signature does not verify against the device key's public half over \
            that challenge and that mint. It counts against the limit wrong passwords meet.",
        remedy: "Sign the challenge with the mint's name, scope, purpose and kind exactly as \
            sent, in the form the contract publishes.",
    }
    /// Raised when this machine would not supply the bytes a challenge is made of.
    NO_CHALLENGE = "DEVICE-12" {
        severity: Error,
        status: 500,
        since: "0.18.0",
        meaning: "lemonfiber could not make a challenge. A challenge is unpredictable bytes this \
            machine supplies, and it would not supply them.",
        remedy: "Try again. If it happens twice, the operating system's own random source is \
            at fault.",
    }
}
