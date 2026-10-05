//! The media server sessions this run has signed in to, kept for the reads after.
//!
//! A token is filed under the server it came from and the credential that minted
//! it, so a password changed since is a different credential with no token yet, and
//! the old token is never carried by the new one. The password itself is not kept
//! here: what is kept is a digest of it, enough to tell one credential from another
//! and no use for signing in.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// What a token is filed under: the server, the account, and a digest of the
/// password that signed in.
type Key = (String, String, Vec<u8>);

/// The tokens this run holds, by the server and credential that minted each.
#[derive(Debug, Default)]
pub struct Sessions(Mutex<BTreeMap<Key, String>>);

impl Sessions {
    /// The token held for this server and credential, where there is one.
    pub(crate) fn held(&self, server: String, account: &str, password: &str) -> Option<String> {
        let key = filed(server, account, password);
        self.0.lock().ok()?.get(&key).cloned()
    }

    /// Hold this token for this server and credential.
    pub(crate) fn keep(&self, server: String, account: &str, password: &str, token: &str) {
        if let Ok(mut held) = self.0.lock() {
            held.insert(filed(server, account, password), token.to_owned());
        }
    }

    /// Let go of the token held for this server and credential.
    pub(crate) fn forget(&self, server: String, account: &str, password: &str) {
        if let Ok(mut held) = self.0.lock() {
            held.remove(&filed(server, account, password));
        }
    }
}

/// The key a token for this server and credential is filed under.
fn filed(server: String, account: &str, password: &str) -> Key {
    let digest = ring::digest::digest(&ring::digest::SHA256, password.as_bytes());
    (server, account.to_owned(), digest.as_ref().to_vec())
}
