// SPDX-License-Identifier: MPL-2.0

use std::io::{BufRead, BufReader, Write};

use crate::BridgeError;

pub trait Transport: Send {
    fn round_trip(&mut self, request: &str) -> Result<String, BridgeError>;
}

pub struct LineTransport<R, W> {
    reader: R,
    writer: W,
}

impl<R: BufRead, W: Write> LineTransport<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        LineTransport { reader, writer }
    }
}

impl<R: BufRead + Send, W: Write + Send> Transport for LineTransport<R, W> {
    fn round_trip(&mut self, request: &str) -> Result<String, BridgeError> {
        writeln!(self.writer, "{request}")?;
        self.writer.flush()?;

        let mut line = String::new();
        if self.reader.read_line(&mut line)? == 0 {
            return Err(BridgeError::Disconnected);
        }
        Ok(line)
    }
}

/// Endpoint names come from a plugin manifest, so they are untrusted input and
/// are pasted into a filesystem path. Anything but this alphabet could escape
/// the namespace the endpoint is supposed to live in.
pub fn is_valid_endpoint_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(windows)]
pub fn endpoint_path(name: &str) -> String {
    format!(r"\.\pipe\idlewarden.{name}")
}

#[cfg(unix)]
pub fn endpoint_path(name: &str) -> String {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_owned());
    format!("{dir}/idlewarden.{name}.sock")
}

#[cfg(windows)]
pub fn connect(name: &str) -> Result<Box<dyn Transport>, BridgeError> {
    use std::fs::OpenOptions;

    let path = endpoint_path(name);
    let writer = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|source| BridgeError::Connect {
            endpoint: path,
            source,
        })?;
    let reader = writer.try_clone()?;

    Ok(Box::new(LineTransport::new(BufReader::new(reader), writer)))
}

#[cfg(unix)]
pub fn connect(name: &str) -> Result<Box<dyn Transport>, BridgeError> {
    use std::os::unix::net::UnixStream;

    let path = endpoint_path(name);
    let writer = UnixStream::connect(&path).map_err(|source| BridgeError::Connect {
        endpoint: path,
        source,
    })?;
    let reader = writer.try_clone()?;

    Ok(Box::new(LineTransport::new(BufReader::new(reader), writer)))
}
