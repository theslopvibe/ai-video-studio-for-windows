//! Serves the bundled OpenCut editor (static files) on 127.0.0.1 so it runs offline in an iframe.
//! The port is fixed on purpose: the editor keeps projects in the browser's IndexedDB, which belongs to the origin
//! (including the port), so a random port would make projects disappear on every start.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PORT: u16 = 47616;
static URL: OnceLock<String> = OnceLock::new();

pub fn url() -> Option<String> {
    URL.get().cloned()
}

fn mime(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" | "webmanifest" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// Maps a request path to a file under `root`; `..` and absolute parts never escape it.
fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("");
    let mut out = root.to_path_buf();
    for part in path.split('/').filter(|p| !p.is_empty()) {
        let part = percent_decode(part);
        if part.is_empty() || part == ".." || part.contains(['/', '\\', ':', '\0']) {
            return None;
        }
        if part != "." {
            out.push(part);
        }
    }
    if out.is_dir() {
        out.push("index.html");
    }
    // last line of defence: the real path (symlinks resolved) must still be inside the folder
    let (real, base) = (out.canonicalize().ok()?, root.canonicalize().ok()?);
    (real.is_file() && real.starts_with(&base)).then_some(real)
}

/// Only requests addressed to this machine are served; a web page on another name (DNS rebinding) is refused.
fn host_ok(head: &str) -> bool {
    head.lines().any(|l| {
        l.split_once(':').is_some_and(|(k, v)| {
            let h = v.trim().to_ascii_lowercase();
            k.trim().eq_ignore_ascii_case("host") && (h.starts_with("127.0.0.1:") || h.starts_with("localhost:") || h == "127.0.0.1" || h == "localhost")
        })
    })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

async fn handle(mut sock: TcpStream, root: PathBuf) {
    let mut buf = vec![0u8; 8192];
    let Ok(n) = sock.read(&mut buf).await else { return };
    let head = String::from_utf8_lossy(&buf[..n]).into_owned();
    let mut first = head.lines().next().unwrap_or("").split_whitespace();
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or("/"));
    let (status, body, ctype) = match (method, if host_ok(&head) { resolve(&root, target) } else { None }) {
        ("GET" | "HEAD", Some(p)) => match tokio::fs::read(&p).await {
            Ok(b) => ("200 OK", b, mime(&p)),
            Err(_) => ("500 Internal Server Error", b"read error".to_vec(), "text/plain"),
        },
        ("GET" | "HEAD", None) => ("404 Not Found", b"Not found".to_vec(), "text/plain"),
        _ => ("405 Method Not Allowed", b"Method not allowed".to_vec(), "text/plain"),
    };
    let cache = if target.contains("/_next/static/") { "public, max-age=31536000, immutable" } else { "no-cache" };
    let hdr = format!("HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: {cache}\r\nConnection: close\r\n\r\n", body.len());
    let _ = sock.write_all(hdr.as_bytes()).await;
    if method != "HEAD" {
        let _ = sock.write_all(&body).await;
    }
    let _ = sock.shutdown().await;
}

pub async fn serve(root: PathBuf) {
    if !root.join("projects").join("index.html").is_file() {
        crate::log::error(&format!("editor: files not found in {}", root.display()));
        return;
    }
    let listener = match TcpListener::bind(("127.0.0.1", PORT)).await {
        Ok(l) => l,
        Err(_) => match TcpListener::bind(("127.0.0.1", 0)).await {
            Ok(l) => {
                crate::log::warn("editor: port 47616 is busy, using another one (editor projects will not carry over)");
                l
            }
            Err(x) => {
                crate::log::error(&format!("editor: bind failed: {x}"));
                return;
            }
        },
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(PORT);
    let _ = URL.set(format!("http://127.0.0.1:{port}"));
    crate::log::info(&format!("editor: serving {} on 127.0.0.1:{port}", root.display()));
    loop {
        let Ok((sock, _)) = listener.accept().await else { continue };
        tauri::async_runtime::spawn(handle(sock, root.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_cannot_escape_the_folder() {
        let d = std::env::temp_dir().join(format!("avs-editor-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(d.join("projects")).unwrap();
        std::fs::write(d.join("projects").join("index.html"), "x").unwrap();
        std::fs::write(d.join("secret.txt"), "s").unwrap();
        let inner = d.join("projects");
        assert!(resolve(&d, "/").is_none());
        assert!(resolve(&inner, "/").is_some());
        assert!(resolve(&d, "/projects/").is_some());
        assert!(resolve(&inner, "/../secret.txt").is_none());
        assert!(resolve(&inner, "/%2e%2e/secret.txt").is_none());
        assert!(resolve(&inner, "/C:/Windows/win.ini").is_none());
        assert!(resolve(&d, "/projects/index.html?x=1").is_some());
        assert!(resolve(&inner, "/..%2fsecret.txt").is_none());
        assert!(resolve(&inner, "/%2e%2e%2fsecret.txt").is_none());
        assert!(host_ok("GET / HTTP/1.1\r\nHost: 127.0.0.1:47616\r\n\r\n"));
        assert!(host_ok("GET / HTTP/1.1\r\nHost: localhost:47616\r\n\r\n"));
        assert!(!host_ok("GET / HTTP/1.1\r\nHost: evil.example:47616\r\n\r\n"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
