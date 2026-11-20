//! # TLS
//!
//! The `tls` table of an account and of each of its network backends, the
//! backend's taking from the account's each field it leaves unset, turned
//! into the handle its connection negotiates with.

use std::path::PathBuf;

use pimalaya_cli::account::BackendCheckTls;
use pimalaya_config::toml::{opt_shell_expanded_path, shell_expanded_path};
use pimalaya_stream::tls::{Rustls, RustlsCrypto, Tls, TlsClient, TlsProvider, TlsServer};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TlsConfig {
    /// The TLS implementation the connection is opened with, unset taking
    /// the first one compiled in.
    pub provider: Option<TlsProviderConfig>,
    /// The rustls options, read by the rustls provider alone.
    #[serde(default)]
    pub rustls: RustlsConfig,
    /// An additional certificate to trust, in PEM format.
    #[serde(default, deserialize_with = "opt_shell_expanded_path")]
    pub cert: Option<PathBuf>,
    /// A client certificate, presented when the server asks for one.
    pub client: Option<TlsClientConfig>,
}

/// A client certificate and its private key, both in PEM format.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TlsClientConfig {
    /// The certificate, optionally followed by its intermediates.
    #[serde(deserialize_with = "shell_expanded_path")]
    pub cert: PathBuf,
    /// The private key, PKCS#8 under native-tls.
    #[serde(deserialize_with = "shell_expanded_path")]
    pub key: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum TlsProviderConfig {
    Rustls,
    NativeTls,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RustlsConfig {
    pub crypto: Option<RustlsCryptoConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum RustlsCryptoConfig {
    Aws,
    Ring,
}

impl TlsConfig {
    /// Takes from `account` each field this table leaves unset: the
    /// provider, the rustls crypto, the trusted certificate and the client
    /// certificate, the last as one so a certificate keeps its key.
    pub fn inherit(&mut self, account: &TlsConfig) {
        if self.provider.is_none() {
            self.provider.clone_from(&account.provider);
        }

        if self.rustls.crypto.is_none() {
            self.rustls.crypto.clone_from(&account.rustls.crypto);
        }

        if self.cert.is_none() {
            self.cert.clone_from(&account.cert);
        }

        if self.client.is_none() {
            self.client.clone_from(&account.client);
        }
    }

    /// The table as `account check` names it, `null` where the build's
    /// default answers.
    pub fn check(&self) -> BackendCheckTls {
        let provider = self.provider.as_ref().map(|provider| match provider {
            TlsProviderConfig::Rustls => "rustls",
            TlsProviderConfig::NativeTls => "native-tls",
        });

        let crypto = self.rustls.crypto.as_ref().map(|crypto| match crypto {
            RustlsCryptoConfig::Aws => "aws",
            RustlsCryptoConfig::Ring => "ring",
        });

        BackendCheckTls {
            provider: provider.map(String::from),
            crypto: crypto.map(String::from),
            cert: self.cert.as_ref().map(|cert| cert.display().to_string()),
            client_cert: self.client.as_ref().map(|c| c.cert.display().to_string()),
        }
    }

    /// Builds the runtime [`Tls`] handle the connect helpers take, folding
    /// in the ALPN list its backend resolved.
    ///
    /// The schema never exposes `tls.rustls.alpn`, the per-backend `alpn`
    /// key standing for it, so a new call site has to say what it
    /// negotiates rather than silently negotiate nothing.
    pub fn into_tls(self, alpn: Vec<String>) -> Tls {
        Tls {
            provider: self.provider.map(|p| match p {
                TlsProviderConfig::Rustls => TlsProvider::Rustls,
                TlsProviderConfig::NativeTls => TlsProvider::NativeTls,
            }),
            rustls: Rustls {
                crypto: self.rustls.crypto.map(|c| match c {
                    RustlsCryptoConfig::Aws => RustlsCrypto::Aws,
                    RustlsCryptoConfig::Ring => RustlsCrypto::Ring,
                }),
                alpn,
            },
            server: self.cert.map(|cert| TlsServer { cert }),
            client: self
                .client
                .map(|TlsClientConfig { cert, key }| TlsClient { cert, key }),
        }
    }
}
