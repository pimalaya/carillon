//! # WebDAV
//!
//! The transport half [`crate::config::caldav`] and
//! [`crate::config::carddav`] share: the server, its credential and the
//! poll.

use anyhow::{Result, bail};
use pimalaya_config::secret::Secret;
use serde::{Deserialize, Deserializer, de::Error as _};
use url::Url;

use crate::config::{PollWatchConfig, parse_url, proxy::ProxyConfig, tls::TlsConfig};

// NOTE: CalDAV and CardDAV are WebDAV, so the transport half is one
// shape; what differs is the domain the collection holds, which is what
// names the events. Two blocks rather than one is what lets a card hook
// on a calendar be refused when the file is read, and they are written
// out rather than flattened because serde cannot deny unknown fields
// across a flatten.

/// The transport half of a DAV backend, shared by a calendar and an
/// addressbook.
#[derive(Clone, Copy)]
pub struct DavServer<'a> {
    /// The URL discovery starts from, unread when `home` is pinned.
    pub server: Option<&'a str>,
    /// The home set pinned, skipping discovery.
    pub home: Option<&'a Url>,
    pub tls: &'a TlsConfig,
    pub alpn: Option<&'a [String]>,
    pub proxy: Option<&'a ProxyConfig>,
    pub auth: &'a DavAuthConfig,
}

impl DavServer<'_> {
    /// Refuses a block naming neither the URL discovery starts from nor
    /// the home set it would find, `backend` naming the block.
    pub fn validate(&self, backend: &str) -> Result<()> {
        if self.server.is_none() && self.home.is_none() {
            bail!("`{backend}` needs `server`, or `home` to pin its home set");
        }

        Ok(())
    }
}

/// Deserializes an optional URL through [`parse_url`].
pub(crate) fn ascii_url<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Url>, D::Error> {
    let input = String::deserialize(deserializer)?;
    let url = parse_url(&input).map_err(D::Error::custom)?;
    Ok(Some(url))
}

/// The credential presented to the DAV server.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum DavAuthConfig {
    /// No `Authorization` header at all.
    #[default]
    None,
    /// HTTP Basic (RFC 7617), what most DAV servers ask for.
    Basic {
        #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
        username: String,
        password: Secret,
    },
    /// HTTP Bearer (RFC 6750), for a server behind OAuth.
    Bearer { token: Secret },
}

/// How a WebDAV account learns about a change.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum DavWatchConfig {
    /// Ask `sync-collection` on an interval, what WebDAV offers a client
    /// with no public endpoint.
    Poll(PollWatchConfig),
}
