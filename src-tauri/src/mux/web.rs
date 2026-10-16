//! The web client: the consoles the sessions daemon keeps, in a browser —
//! a phone's — on another computer of the tailnet (see remote.rs).
//!
//! Served on the same port as the daemon's own protocol, told apart by how
//! a connection starts (`GET `). The page is static (web-client/, with
//! xterm.js from the app's own dependencies); it talks to the daemon over
//! a WebSocket carrying the same frames as any other client, bridged into
//! the same connection handling, so it gets in only as far as the remote
//! access list says.
//!
//! A page from anywhere else, open in the same browser, must not reach the
//! daemon with the user's tailnet identity: the WebSocket is only accepted
//! from this page (its Origin matches the Host it was served from) and only
//! under this computer's own tailnet names (no DNS rebinding).

use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{Message, WebSocket};

use super::server::{Daemon, Role};

const INDEX: &[u8] = include_bytes!("../../web-client/index.html");
const APP: &[u8] = include_bytes!("../../web-client/app.js");
const STYLE: &[u8] = include_bytes!("../../web-client/style.css");
const XTERM_JS: &[u8] = include_bytes!("../../../node_modules/@xterm/xterm/lib/xterm.js");
const XTERM_CSS: &[u8] = include_bytes!("../../../node_modules/@xterm/xterm/css/xterm.css");
const FIT_JS: &[u8] = include_bytes!("../../../node_modules/@xterm/addon-fit/lib/addon-fit.js");

/// Whether a connection is a browser's: it starts with a request line.
pub fn is_http(stream: &TcpStream) -> bool {
    let mut start = [0u8; 4];
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    matches!(stream.peek(&mut start), Ok(4) if &start == b"GET ")
}

/// The path asked for, from the request line, without consuming it.
fn requested_path(stream: &TcpStream) -> Option<String> {
    let mut head = [0u8; 2048];
    let read = stream.peek(&mut head).ok()?;
    let line = head[..read].split(|byte| *byte == b'\r').next()?;
    let line = std::str::from_utf8(line).ok()?;
    let mut parts = line.split(' ');
    (parts.next()? == "GET").then_some(())?;
    Some(parts.next()?.split('?').next()?.to_string())
}

/// A browser's request, already let in (see Daemon::admit): the page, or
/// the WebSocket to the daemon.
pub fn serve(daemon: Arc<Daemon>, mut stream: TcpStream, own: &[String], role: Role) {
    let path = requested_path(&stream).unwrap_or_default();
    if path == "/ws" {
        let own = own.to_vec();
        let checked = move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            if same_origin_on_own_host(request, &own) {
                Ok(response)
            } else {
                let mut refused = ErrorResponse::new(Some("forbidden".into()));
                *refused.status_mut() = tungstenite::http::StatusCode::FORBIDDEN;
                Err(refused)
            }
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        if let Ok(ws) = tungstenite::accept_hdr(stream, checked) {
            let _ = bridge(&daemon, ws, role);
        }
        return;
    }
    // The request head, read out before answering.
    let _ = read_head(&mut stream);
    let file = match path.as_str() {
        "/" | "/index.html" => Some((INDEX, "text/html; charset=utf-8")),
        "/app.js" => Some((APP, "text/javascript; charset=utf-8")),
        "/style.css" => Some((STYLE, "text/css; charset=utf-8")),
        "/xterm.js" => Some((XTERM_JS, "text/javascript; charset=utf-8")),
        "/xterm.css" => Some((XTERM_CSS, "text/css; charset=utf-8")),
        "/addon-fit.js" => Some((FIT_JS, "text/javascript; charset=utf-8")),
        _ => None,
    };
    let _ = match file {
        Some((body, kind)) => respond(&mut stream, "200 OK", kind, body),
        None => respond(&mut stream, "404 Not Found", "text/plain; charset=utf-8", b"not found"),
    };
}

/// Turned away (not on the list, or not on the tailnet): a short page that
/// says so, in both of Termo's languages.
pub fn refuse(mut stream: TcpStream, waiting: bool) {
    let _ = read_head(&mut stream);
    let body: &[u8] = if waiting {
        b"<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width'>\
          <body style='font-family:system-ui;padding:24px;background:#1e1e1e;color:#ddd'>\
          <h3>Termo</h3><p>Esta computadora todav\xc3\xada no te dej\xc3\xb3 entrar: tu pedido qued\xc3\xb3 esperando respuesta.</p>\
          <p>This computer hasn't let you in yet: your request is waiting for an answer.</p>"
    } else {
        b"<!doctype html><meta charset=utf-8><body><p>Termo</p>"
    };
    let _ = respond(&mut stream, "403 Forbidden", "text/html; charset=utf-8", body);
}

fn read_head(stream: &mut TcpStream) -> io::Result<()> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && head.len() < 16 * 1024 {
        if stream.read(&mut byte)? == 0 {
            break;
        }
        head.push(byte[0]);
    }
    Ok(())
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\n\
         Content-Security-Policy: default-src 'self'; connect-src 'self' ws: wss:; style-src 'self' 'unsafe-inline'; \
         frame-ancestors 'none'\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

/// The WebSocket's Origin is the page's own, and the Host it was asked under
/// is one of this computer's own names on the tailnet.
fn same_origin_on_own_host(request: &Request, own: &[String]) -> bool {
    let header = |name: &str| request.headers().get(name).and_then(|value| value.to_str().ok()).map(str::to_string);
    let (Some(host), Some(origin)) = (header("host"), header("origin")) else {
        return false;
    };
    let same_origin = origin == format!("http://{host}") || origin == format!("https://{host}");
    let hostname = host.rsplit_once(':').map_or(host.as_str(), |(name, _)| name).trim_matches(['[', ']']);
    same_origin && own.iter().any(|name| name.eq_ignore_ascii_case(hostname))
}

/// This computer's names on the tailnet, which a page may be asked under:
/// its address, and its MagicDNS name, short and full. Asked of Tailscale
/// now and then.
pub fn own_names(address: Option<IpAddr>) -> Vec<String> {
    static CACHE: Mutex<Option<(Instant, Vec<String>)>> = Mutex::new(None);
    let mut names: Vec<String> = address.map(|ip| ip.to_string()).into_iter().collect();
    let mut cache = CACHE.lock().unwrap();
    let fresh = cache.as_ref().is_some_and(|(at, _)| at.elapsed() < Duration::from_secs(60));
    if !fresh {
        let found = super::tailscale::status()
            .map(|status| vec![status.me.name.clone(), status.me.dns_name.clone()])
            .unwrap_or_default();
        *cache = Some((Instant::now(), found));
    }
    if let Some((_, found)) = cache.as_ref() {
        names.extend(found.iter().filter(|name| !name.is_empty()).cloned());
    }
    names
}

/// Carries a browser's WebSocket to a connection handled like any other —
/// as the same someone, with the same access: each WebSocket message holds
/// bytes of the daemon's own frame stream, one way and the other.
fn bridge(daemon: &Arc<Daemon>, mut ws: WebSocket<TcpStream>, role: Role) -> io::Result<()> {
    let (mut inner, outer) = pair()?;
    {
        let daemon = Arc::clone(daemon);
        std::thread::spawn(move || daemon.handle_connection(outer, role));
    }
    ws.get_mut().set_read_timeout(None)?;
    ws.get_mut().set_nonblocking(true)?;
    inner.set_nonblocking(true)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let mut idle = true;
        match ws.read() {
            Ok(Message::Binary(data)) => {
                idle = false;
                inner.set_nonblocking(false)?;
                let written = inner.write_all(&data);
                inner.set_nonblocking(true)?;
                written?;
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => idle = false,
            Err(tungstenite::Error::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }
        match inner.read(&mut buf) {
            Ok(0) => break,
            Ok(read) => {
                idle = false;
                match ws.send(Message::Binary(buf[..read].to_vec().into())) {
                    Ok(()) => {}
                    // Queued; it goes out with a later flush.
                    Err(tungstenite::Error::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }
        if idle {
            match ws.flush() {
                Ok(()) => {}
                Err(tungstenite::Error::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => break,
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let _ = inner.shutdown(std::net::Shutdown::Both);
    let _ = ws.close(None);
    Ok(())
}

/// Two connected sockets on this computer — a socket pair — through a
/// listener that takes nothing but this one connection.
fn pair() -> io::Result<(TcpStream, TcpStream)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))?;
    let ours = TcpStream::connect(listener.local_addr()?)?;
    let expected = ours.local_addr()?;
    let (theirs, peer) = listener.accept()?;
    if peer != expected {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "someone else connected"));
    }
    Ok((ours, theirs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(host: &str, origin: Option<&str>) -> Request {
        let mut builder = Request::builder().uri("/ws").header("Host", host);
        if let Some(origin) = origin {
            builder = builder.header("Origin", origin);
        }
        builder.body(()).unwrap()
    }

    #[test]
    fn only_this_page_under_this_computers_names_gets_the_websocket() {
        let own = vec!["100.101.102.103".to_string(), "desktop".to_string(), "desktop.tail1234.ts.net".to_string()];
        let ok = |host: &str, origin: &str| same_origin_on_own_host(&request(host, Some(origin)), &own);
        assert!(ok("100.101.102.103:47830", "http://100.101.102.103:47830"));
        assert!(ok("desktop.tail1234.ts.net:47830", "http://desktop.tail1234.ts.net:47830"));
        assert!(ok("desktop:47830", "http://desktop:47830"));
        // Another site's page, in the same browser.
        assert!(!ok("100.101.102.103:47830", "https://evil.example"));
        // DNS rebinding: the attacker's name, resolving to this computer.
        assert!(!ok("evil.example:47830", "http://evil.example:47830"));
        assert!(!same_origin_on_own_host(&request("100.101.102.103:47830", None), &own), "no Origin, no entry");
    }

    /// A daemon with remote access on, its "tailnet" on 127.0.0.1, and
    /// whoever connects being the user themselves.
    fn daemon() -> (u16, std::path::PathBuf) {
        use crate::mux::remote::{self, Access, Allowed, RemoteConfig};
        use crate::mux::tailscale::Identity;
        let dir = std::env::temp_dir().join(format!("termo-web-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let config = RemoteConfig {
            enabled: true,
            port: 0,
            allowed: vec![Allowed { login: "me@example.com".into(), node: None, access: Access::ReadOnly }],
        };
        remote::write_config(&dir, &config).unwrap();
        let whois = Box::new(|_| Some(Identity { login: "me@example.com".into(), node: "phone".into() }));
        let (_, remote_port) = crate::mux::server::start_remote_for_test("web-token", &dir, whois).unwrap();
        (remote_port, dir)
    }

    fn websocket(port: u16, origin: &str) -> tungstenite::Result<WebSocket<TcpStream>> {
        use tungstenite::client::IntoClientRequest;
        let mut request = format!("ws://127.0.0.1:{port}/ws").into_client_request()?;
        request.headers_mut().insert("Origin", origin.parse().unwrap());
        let stream = TcpStream::connect(("127.0.0.1", port)).map_err(tungstenite::Error::Io)?;
        tungstenite::client(request, stream).map(|(ws, _)| ws).map_err(|err| match err {
            tungstenite::HandshakeError::Failure(err) => err,
            tungstenite::HandshakeError::Interrupted(_) => tungstenite::Error::ConnectionClosed,
        })
    }

    #[test]
    fn the_page_and_its_websocket_are_served_to_whom_the_list_lets_in() {
        let (port, dir) = daemon();

        let mut page = TcpStream::connect(("127.0.0.1", port)).unwrap();
        page.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut answer = String::new();
        page.read_to_string(&mut answer).unwrap();
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.contains("<title>Termo</title>"));
        assert!(answer.contains("Content-Security-Policy"));

        // The page's own WebSocket speaks the daemon's protocol.
        let origin = format!("http://127.0.0.1:{port}");
        let mut ws = websocket(port, &origin).unwrap();
        let mut hello = Vec::new();
        let message = super::super::protocol::ClientMessage::Hello {
            protocol: super::super::protocol::PROTOCOL,
            token: String::new(),
            version: "web".into(),
        };
        super::super::protocol::write_frame(&mut hello, &super::super::protocol::Frame::Message(message)).unwrap();
        ws.send(Message::Binary(hello.into())).unwrap();
        let reply = loop {
            if let Message::Binary(data) = ws.read().unwrap() {
                break data;
            }
        };
        let mut reader: &[u8] = &reply;
        let frame = super::super::protocol::read_frame::<super::super::protocol::ServerMessage>(&mut reader).unwrap();
        assert!(
            matches!(
                frame,
                Some(super::super::protocol::Frame::Message(super::super::protocol::ServerMessage::Welcome {
                    access: Some(crate::mux::remote::Access::ReadOnly),
                    ..
                }))
            ),
            "{frame:?}"
        );

        // Another site's page, open in the same browser, doesn't get one.
        assert!(websocket(port, "https://evil.example").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_socket_pair_is_connected_both_ways() {
        let (mut a, mut b) = pair().unwrap();
        a.write_all(b"ping").unwrap();
        let mut got = [0u8; 4];
        b.read_exact(&mut got).unwrap();
        assert_eq!(&got, b"ping");
    }
}
