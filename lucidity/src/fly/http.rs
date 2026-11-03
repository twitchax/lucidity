//! The little HTTP/1.1 the Machines API needs, over lunatic's TCP and TLS streams: one request per connection, with a
//! bearer token and an optional JSON body.

use std::io::{Read, Write};

use lucidity_core::lunatic::net::{TcpStream, TlsStream};
use serde_json::Value;

/// A response's status, like `422 Unprocessable Entity`, and body.
pub(super) struct Response {
    pub(super) code: u16,
    pub(super) status: String,
    pub(super) body: String,
}

impl Response {
    pub(super) fn is_success(&self) -> bool {
        (200..300).contains(&self.code)
    }
}

/// Sends a `method` request to the `http` or `https` `url`, authorized with `key`, and returns the response.
pub(super) fn request(method: &str, url: &str, key: &str, body: Option<&Value>) -> Result<Response, String> {
    let (tls, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return Err(format!("Unsupported URL {url}."));
    };
    let (authority, path) = rest.find('/').map_or((rest, "/"), |slash| rest.split_at(slash));
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().map_err(|_| format!("Invalid port in URL {url}."))?),
        None => (authority, if tls { 443 } else { 80 }),
    };

    let body = body.map(Value::to_string);
    let mut message = format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer {key}\r\nAccept: application/json\r\nConnection: close\r\n");
    match &body {
        Some(body) => message.push_str(&format!("Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len())),
        // Some servers want a length on every `POST`.
        None if method == "POST" => message.push_str("Content-Length: 0\r\n\r\n"),
        None => message.push_str("\r\n"),
    }

    let response = if tls {
        let stream = TlsStream::connect(host, u32::from(port)).map_err(|e| format!("Failed to connect to {authority}: {e}."))?;
        exchange(stream, message.as_bytes())
    } else {
        let stream = TcpStream::connect(format!("{host}:{port}")).map_err(|e| format!("Failed to connect to {authority}: {e}."))?;
        exchange(stream, message.as_bytes())
    }
    .map_err(|e| format!("Failed to talk to {authority}: {e}."))?;

    parse(&response)
}

/// Writes the request and reads the response until the server closes the connection.
fn exchange(mut stream: impl Read + Write, request: &[u8]) -> std::io::Result<Vec<u8>> {
    stream.write_all(request)?;
    stream.flush()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response)?;
    Ok(response)
}

fn parse(response: &[u8]) -> Result<Response, String> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut parsed = httparse::Response::new(&mut headers);
    let start = match parsed.parse(response) {
        Ok(httparse::Status::Complete(start)) => start,
        Ok(httparse::Status::Partial) => return Err("The response ended in its headers.".to_owned()),
        Err(e) => return Err(format!("Invalid response: {e}.")),
    };
    let code = parsed.code.unwrap_or_default();
    let status = match parsed.reason.unwrap_or_default() {
        "" => code.to_string(),
        reason => format!("{code} {reason}"),
    };
    let header = |name: &str| {
        parsed
            .headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| String::from_utf8_lossy(header.value).trim().to_ascii_lowercase())
    };

    let content = &response[start..];
    let body = if header("transfer-encoding").is_some_and(|encoding| encoding.ends_with("chunked")) {
        dechunk(content)?
    } else if let Some(length) = header("content-length") {
        let length = length.parse::<usize>().map_err(|_| format!("Invalid Content-Length {length}."))?;
        content.get(..length).ok_or("The response ended in its body.")?.to_vec()
    } else {
        content.to_vec()
    };

    Ok(Response {
        code,
        status,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Decodes a chunked body, ignoring chunk extensions and trailers.
fn dechunk(mut content: &[u8]) -> Result<Vec<u8>, String> {
    let truncated = || "The response ended in a chunk.".to_owned();
    let mut body = Vec::new();
    loop {
        let line_end = content.windows(2).position(|window| window == b"\r\n").ok_or_else(truncated)?;
        let line = String::from_utf8_lossy(&content[..line_end]);
        let size = line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size, 16).map_err(|_| format!("Invalid chunk size {size}."))?;
        content = &content[line_end + 2..];
        if size == 0 {
            return Ok(body);
        }
        body.extend_from_slice(content.get(..size).ok_or_else(truncated)?);
        content = content.get(size + 2..).ok_or_else(truncated)?;
    }
}
