//! # TLS
//!
//! The `tls` table each network backend takes, turned into the handle its
//! connection negotiates with.

use std::path::PathBuf;

use pimalaya_config::toml::opt_shell_expanded_path;
use pimalaya_stream::tls::{Rustls, RustlsCrypto, Tls, TlsProvider, TlsServer};
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
            client: None,
        }
    }
}
