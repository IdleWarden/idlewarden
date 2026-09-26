// SPDX-License-Identifier: MPL-2.0
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::StatusCode;
use tungstenite::{Message, WebSocket};

use crate::transport::{is_valid_endpoint_name, Transport};
use crate::BridgeError;

pub const DEFAULT_PORT: u16 = 47825;

pub(crate) const POLL: Duration = Duration::from_millis(50);

pub struct WebSocketTransport(WebSocket<TcpStream>);

impl Transport for WebSocketTransport {
    fn round_trip(&mut self, request: &str) -> Result<String, BridgeError> {
        self.0
            .send(Message::text(request))
            .map_err(|error| lost(&error))?;

        loop {
            match self.0.read().map_err(|error| lost(&error))? {
                Message::Text(answer) => return Ok(answer.to_string()),
                Message::Close(_) => return Err(BridgeError::Disconnected),
                Message::Binary(_) => {
                    return Err(BridgeError::Unexpected {
                        expected: "text",
                        got: "binary",
                    })
                }
                _ => continue,
            }
        }
    }
}

pub fn bind(port: u16) -> Result<TcpListener, BridgeError> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Waits for the mod to connect. The endpoint name is the request path, and a
/// remote page is turned away at the handshake (ADR-0018).
pub fn accept(
    listener: &TcpListener,
    name: &str,
    wait: Duration,
) -> Result<Box<dyn Transport>, BridgeError> {
    if !is_valid_endpoint_name(name) {
        return Err(BridgeError::InvalidEndpoint {
            endpoint: name.to_owned(),
        });
    }

    let deadline = Instant::now() + wait;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(transport) = greet(stream, name) {
                    return Ok(transport);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(BridgeError::Io(error)),
        }

        if Instant::now() >= deadline {
            return Err(BridgeError::Connect {
                endpoint: format!("ws://127.0.0.1/{name}"),
                source: std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "no mod connected before the wait expired",
                ),
            });
        }
        std::thread::sleep(POLL);
    }
}

#[allow(clippy::result_large_err)]
fn greet(stream: TcpStream, name: &str) -> Option<Box<dyn Transport>> {
    if stream.set_nonblocking(false).is_err() {
        return None;
    }

    let path = format!("/{name}");
    let socket = tungstenite::accept_hdr(stream, |request: &Request, response: Response| {
        if request.uri().path() != path {
            return Err(refuse(
                StatusCode::NOT_FOUND,
                "this endpoint does not serve that bridge",
            ));
        }
        if !is_local_document(request) {
            return Err(refuse(
                StatusCode::FORBIDDEN,
                "a bridge only answers a page loaded from the game itself",
            ));
        }
        Ok(response)
    });

    match socket {
        Ok(socket) => Some(Box::new(WebSocketTransport(socket))),
        Err(error) => {
            tracing::warn!(%error, "a connection to the bridge was turned away");
            None
        }
    }
}

fn is_local_document(request: &Request) -> bool {
    let Some(origin) = request.headers().get("origin") else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    origin.is_empty()
        || origin.eq_ignore_ascii_case("null")
        || origin.starts_with("file://")
        || origin.starts_with("app://")
}

/// The body stays empty on purpose: a client that reads one has no
/// `Content-Length` to stop at, and waits for a close that a refused handshake
/// does not always send.
fn refuse(status: StatusCode, reason: &'static str) -> ErrorResponse {
    tracing::warn!(reason, "a connection to the bridge was refused");
    let mut response = ErrorResponse::new(None);
    *response.status_mut() = status;
    response
}

fn lost(error: &tungstenite::Error) -> BridgeError {
    match error {
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
            BridgeError::Disconnected
        }
        other => BridgeError::Io(std::io::Error::other(other.to_string())),
    }
}
