//! The credential core presents when it connects.
//!
//! Core knows only an authentication mechanism and its secret. It never
//! unlocks a keyring and never mints or refreshes OAuth. That resolution
//! happens upstream, so the secret is handed in already resolved and OAuth
//! is, to core, just a token.

use secrecy::SecretString;

/// A resolved credential presented to a server on connect.
pub enum CarillonCredential {
    /// A SASL LOGIN or HTTP Basic password.
    Password(SecretString),
    /// A pre-minted OAuth 2.0 access token, used as XOAUTH2 or a bearer.
    /// Upstream mints and refreshes it; core only presents it.
    Bearer(SecretString),
}
