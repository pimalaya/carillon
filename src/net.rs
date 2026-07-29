//! Shared TLS connection helper for the watch driver and the resolver.
//!
//! Both open an implicit-TLS IMAP connection the same way; this is the one
//! place that builds it, so the ALPN and timeout policy stay consistent.

use std::time::Duration;

use anyhow::{Context, Result};
use pimalaya_stream::{std::stream::StreamStd, tls::Tls};

/// Opens an implicit-TLS IMAP connection to `host:port`. A `read_timeout`,
/// when set, bounds each blocking read so a caller can poll a shutdown flag
/// between reads.
pub fn connect(host: &str, port: u16, read_timeout: Option<Duration>) -> Result<StreamStd> {
    let mut tls = Tls::default();
    tls.rustls.alpn = vec![String::from("imap")];
    let stream = StreamStd::connect_tls(host, port, &tls).context("cannot open TLS connection")?;
    if read_timeout.is_some() {
        stream.set_read_timeout(read_timeout).ok();
    }
    Ok(stream)
}

/// Whether an I/O error is a read timeout (an idle poll), not a real failure.
pub fn is_timeout(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}
