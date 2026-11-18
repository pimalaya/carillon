//! # Scripted stream
//!
//! The stream the vendor API tests hand a client in place of a
//! connection: each request it is sent is answered with the next of its
//! scripted answers, so a client meets a server's refusal exactly as it
//! reads one off the wire.

use std::{
    collections::VecDeque,
    io::{self, Cursor, Read, Write},
    mem,
};

/// The status of an answer that went through.
pub const OK: &str = "200 OK";

/// A stream answering each request with the next of its answers, a
/// status and a JSON body, and keeping the requests it was sent.
#[derive(Default)]
pub struct Scripted {
    answers: VecDeque<(&'static str, &'static str)>,
    answer: Cursor<Vec<u8>>,
    request: Vec<u8>,
    /// The request line of every request sent.
    pub seen: Vec<String>,
    /// The head of every request sent, its request line and headers.
    pub heads: Vec<String>,
}

impl Scripted {
    /// A stream answering with `answers` in order, then `200 OK` with an
    /// empty body.
    pub fn answering(answers: &[(&'static str, &'static str)]) -> Self {
        Self {
            answers: answers.iter().copied().collect(),
            ..Default::default()
        }
    }
}

impl Write for Scripted {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.request.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for Scripted {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.request.is_empty() {
            let request = mem::take(&mut self.request);
            let request = String::from_utf8_lossy(&request);
            let line = request.lines().next().unwrap_or_default();
            self.seen.push(line.to_string());
            let head = request.split("\r\n\r\n").next().unwrap_or_default();
            self.heads.push(head.to_string());

            let (status, body) = self.answers.pop_front().unwrap_or((OK, ""));
            let length = body.len();
            let answer = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}"
            );
            self.answer = Cursor::new(answer.into_bytes());
        }

        self.answer.read(buf)
    }
}
