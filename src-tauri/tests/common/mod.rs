//! A tiny HTTP/1.1 server for tests: one request per connection, optional Range support,
//! and the ability to drop the connection part-way through a body.

#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct Req {
    pub method: String,
    /// Path and query, e.g. `/bucket/?list-type=2`.
    pub target: String,
    pub headers: HashMap<String, String>,
}

pub struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Close the connection after this many body bytes (Content-Length still announces all).
    pub cut_after: Option<usize>,
}

impl Resp {
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Resp { status: 200, headers: vec![], body: body.into(), cut_after: None }
    }
    pub fn status(code: u16) -> Self {
        Resp { status: code, headers: vec![], body: vec![], cut_after: None }
    }
    pub fn header(mut self, k: &str, v: impl Into<String>) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
    pub fn cut(mut self, n: usize) -> Self {
        self.cut_after = Some(n);
        self
    }
}

/// `bytes=START-` -> START
pub fn range_start(req: &Req) -> Option<usize> {
    req.headers.get("range")?.strip_prefix("bytes=")?.trim_end_matches('-').split('-').next()?.parse().ok()
}

pub type Handler = Arc<dyn Fn(&Req) -> Resp + Send + Sync>;

pub async fn serve(handler: Handler) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { return };
            let handler = handler.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let text = String::from_utf8_lossy(&buf).into_owned();
                let mut lines = text.split("\r\n");
                let mut first = lines.next().unwrap_or("").split(' ');
                let method = first.next().unwrap_or("").to_string();
                let target = first.next().unwrap_or("").to_string();
                let headers = lines
                    .take_while(|l| !l.is_empty())
                    .filter_map(|l| l.split_once(':'))
                    .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
                    .collect();
                let resp = handler(&Req { method, target, headers });

                let reason = match resp.status {
                    200 => "OK",
                    206 => "Partial Content",
                    404 => "Not Found",
                    416 => "Range Not Satisfiable",
                    500 => "Internal Server Error",
                    _ => "Status",
                };
                let mut head = format!("HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n", resp.status, reason, resp.body.len());
                for (k, v) in &resp.headers {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                let _ = sock.write_all(head.as_bytes()).await;
                let n = resp.cut_after.map_or(resp.body.len(), |c| c.min(resp.body.len()));
                let _ = sock.write_all(&resp.body[..n]).await;
                let _ = sock.flush().await;
                // dropping the socket closes it (abruptly if the body was cut short)
            });
        }
    });
    addr
}
