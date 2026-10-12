//! Attestations recorded from real phones, for the tests that verify one against the
//! platform roots a build ships.
//!
//! The two Android chains are `WebAuthn` `android-key` registrations from
//! `duo-labs/py_webauthn`'s tests, one read back to Google's RSA root and one to its
//! elliptic-curve root; the App Attest object is from `bas-d/appattest`'s tests. Both
//! repositories are BSD-3-Clause. Each certificate is valid only for a while, so each
//! recording carries the moment it is read at.

/// An Android key attestation chain, the key's own certificate first.
pub struct AndroidRecording {
    /// Each certificate's DER.
    pub chain: &'static [&'static [u8]],
    /// Seconds since the epoch at which every certificate in it is valid.
    pub at: u64,
}

/// An App Attest attestation object, and what it was made over.
pub struct AppleRecording {
    /// The attestation object's CBOR.
    pub object: &'static [u8],
    /// The client data whose SHA-256 the object was attested with.
    pub client_data: &'static [u8],
    /// The App ID it vouches for, `<Team ID>.<bundle ID>`.
    pub app_id: &'static str,
    /// Seconds since the epoch at which its certificates are valid.
    pub at: u64,
}

/// A chain from a phone whose attestation keys Google's RSA root signed.
pub const ANDROID_RSA_ROOT: AndroidRecording = AndroidRecording {
    chain: &[
        include_bytes!("attestations/android-rsa-root/0.der"),
        include_bytes!("attestations/android-rsa-root/1.der"),
        include_bytes!("attestations/android-rsa-root/2.der"),
        include_bytes!("attestations/android-rsa-root/3.der"),
        include_bytes!("attestations/android-rsa-root/4.der"),
    ],
    at: 1_736_294_400,
};

/// A chain from a phone whose attestation keys Google's elliptic-curve root signed.
pub const ANDROID_EC_ROOT: AndroidRecording = AndroidRecording {
    chain: &[
        include_bytes!("attestations/android-ec-root/0.der"),
        include_bytes!("attestations/android-ec-root/1.der"),
        include_bytes!("attestations/android-ec-root/2.der"),
        include_bytes!("attestations/android-ec-root/3.der"),
        include_bytes!("attestations/android-ec-root/4.der"),
    ],
    at: 1_777_161_600,
};

/// An App Attest attestation from a genuine instance of a test app.
pub const APPLE: AppleRecording = AppleRecording {
    object: include_bytes!("attestations/apple-app-attest.cbor"),
    client_data: b"attestation-test",
    app_id: "35MFYY2JY5.co.chiff.attestation-test",
    at: 1_618_394_120,
};
