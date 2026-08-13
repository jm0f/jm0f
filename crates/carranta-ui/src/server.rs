//! A local HTTP server, on `std` alone.
//!
//! Deliberately small and deliberately local. It binds the loopback address
//! only, serves one page and three endpoints, and holds one game in memory.
//! It is not the server of §6.2 — there is no auth, no persistence, no
//! concurrency beyond a mutex — and it should not grow into it. What it is for
//! is putting the engine in front of a person.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;

use carranta_core::state::TradeMode;

use crate::game::Session;
use crate::json;
use crate::view;

const PAGE: &str = include_str!("../assets/index.html");

/// Largest request body accepted. A click is a few dozen bytes; anything
/// larger is a mistake or a probe, and is refused rather than buffered.
const MAX_BODY: usize = 4 * 1024;

pub struct Server {
    session: Mutex<Session>,
}

impl Server {
    pub fn new(seats: u8, seed: u64, mode: TradeMode) -> Self {
        Server {
            session: Mutex::new(Session::new(seats, seed, mode)),
        }
    }

    /// Serve until the process is stopped.
    pub fn serve(&self, listener: TcpListener) {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    // One connection at a time: there is one game and one
                    // player, and a thread pool would be pretending otherwise.
                    if let Err(e) = self.handle(s) {
                        eprintln!("connection: {e}");
                    }
                }
                Err(e) => eprintln!("accept: {e}"),
            }
        }
    }

    fn handle(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut request = String::new();
        if reader.read_line(&mut request)? == 0 {
            return Ok(());
        }
        let mut parts = request.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("/").to_string();

        // Headers, for the body length only.
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line
                .strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
            {
                length = v.trim().parse().unwrap_or(0);
            }
        }
        if length > MAX_BODY {
            return respond(&mut stream, 413, "text/plain", b"body too large");
        }
        let mut body = vec![0u8; length];
        if length > 0 {
            reader.read_exact(&mut body)?;
        }
        let body = String::from_utf8_lossy(&body).to_string();

        let (path, query) = match path.split_once('?') {
            Some((p, q)) => (p, q),
            None => (path.as_str(), ""),
        };

        match (method.as_str(), path) {
            ("GET", "/") => respond(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
            ),
            ("GET", "/api/state") => {
                let session = self.session.lock().unwrap();
                let payload = view::render(&session);
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            ("POST", "/api/act") => {
                let mut session = self.session.lock().unwrap();
                let action = json::read_u64(&body, "action");
                let version = json::read_u64(&body, "version");
                let payload = match (action, version) {
                    (Some(a), Some(v)) => match session.act(a as usize, v) {
                        Ok(()) => view::render(&session),
                        Err(e) => view::render_with_note(&session, &format!("{e:?}")),
                    },
                    _ => view::render_with_note(&session, "malformed request"),
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            ("POST", "/api/new") => {
                let mut session = self.session.lock().unwrap();
                let seats = param(query, "seats")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(4);
                let mode = match param(query, "mode").as_deref() {
                    Some("disabled") => TradeMode::Disabled,
                    Some("restricted") => TradeMode::Restricted,
                    _ => TradeMode::Full,
                };
                let seed = param(query, "seed")
                    .and_then(|v| v.parse().ok())
                    // No clock dependency: the previous game's seed advances.
                    .unwrap_or_else(|| session.seed().wrapping_add(1));
                *session = Session::new(seats, seed, mode);
                let payload = view::render(&session);
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            _ => respond(&mut stream, 404, "text/plain", b"not found"),
        }
    }
}

fn param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

fn respond(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {kind}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parameters_are_read_or_ignored() {
        assert_eq!(param("seats=3&mode=full", "seats").as_deref(), Some("3"));
        assert_eq!(param("seats=3&mode=full", "mode").as_deref(), Some("full"));
        assert_eq!(param("seats=3", "seed"), None);
        assert_eq!(param("", "seats"), None);
        assert_eq!(param("broken", "seats"), None);
    }
}
