//! # Vendor APIs
//!
//! What the vendor API blocks share: the bearer token and the poll.

use std::time::Duration;

use pimalaya_config::secret::Secret;
use serde::Deserialize;

use crate::config::PollWatchConfig;

// NOTE: the vendor REST APIs authenticate with a bearer token alone and
// push only to a public endpoint or a Pub/Sub topic, so each block takes
// one token and the poll is its one method.

/// The credential a vendor API takes, a bearer token alone.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BearerAuthConfig {
    /// The OAuth 2.0 access token, usually a broker command since it
    /// expires.
    pub token: Secret,
}

/// How a vendor API account learns about a change.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApiWatchConfig {
    /// Read the change feed on an interval, what these APIs offer a
    /// client with no public endpoint.
    Poll(PollWatchConfig),
}

impl ApiWatchConfig {
    /// The poll interval, unset taking the backend default.
    pub fn interval(watch: &Option<Self>) -> Option<Duration> {
        let Some(Self::Poll(poll)) = watch else {
            return None;
        };

        poll.interval()
    }
}
