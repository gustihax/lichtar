//! Завантаження з докачуванням (HTTP Range). Не залежить від Tauri, тому тестується окремо.
//!
//! Файл пишеться в `<part>`. Якщо він уже є, просимо сервер продовжити з потрібного байта.
//! Сервер без підтримки Range (відповідь 200) змушує почати спочатку; 416 означає, що файл уже повний.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures_util::StreamExt;

#[derive(Debug)]
pub enum DlError {
    Cancelled,
    NoSpace { need: u64, free: u64 },
    Other(String),
}

impl std::fmt::Display for DlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "CANCELLED"),
            Self::NoSpace { need, free } => write!(f, "NOSPACE:{need}:{free}"),
            Self::Other(m) => write!(f, "{m}"),
        }
    }
}

fn other<E: std::fmt::Display>(e: E) -> DlError {
    DlError::Other(e.to_string())
}

pub async fn download(
    url: &str,
    part: &Path,
    cancel: &AtomicBool,
    progress: &(dyn Fn(f32) + Send + Sync),
) -> Result<(), DlError> {
    let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let client = reqwest::Client::builder().connect_timeout(Duration::from_secs(15)).build().map_err(other)?;
    let mut req = client.get(url);
    if have > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let resp = req.send().await.map_err(other)?;
    let status = resp.status();
    if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        return Ok(()); // файл уже завантажено повністю, перевірить хеш
    }
    let resp = resp.error_for_status().map_err(other)?;
    let partial = status == reqwest::StatusCode::PARTIAL_CONTENT;
    if partial {
        let ok = resp
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with(&format!("bytes {have}-")));
        if !ok {
            let _ = fs::remove_file(part);
            return Err(DlError::Other("Сервер повернув некоректний діапазон, спробуйте ще раз".into()));
        }
    } else {
        have = 0;
    }
    let len = resp.content_length().unwrap_or(0);
    if len > 0 {
        let dir = part.parent().unwrap_or(Path::new("."));
        let free = fs2::available_space(dir).unwrap_or(u64::MAX);
        if free < len + 8 * 1024 * 1024 {
            return Err(DlError::NoSpace { need: len, free });
        }
    }
    let total = (have + len) as f32;
    let mut opts = OpenOptions::new();
    if partial {
        opts.create(true).append(true);
    } else {
        opts.create(true).write(true).truncate(true);
    }
    let mut file = opts.open(part).map_err(other)?;
    let (mut got, mut last) = (0u64, u32::MAX);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return Err(DlError::Cancelled);
        }
        let chunk = chunk.map_err(other)?;
        file.write_all(&chunk).map_err(other)?;
        got += chunk.len() as u64;
        if total > 0.0 {
            let pct = (have + got) as f32 / total * 100.0;
            if pct as u32 != last {
                last = pct as u32;
                progress(pct);
            }
        }
    }
    file.flush().map_err(other)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Copy, PartialEq)]
    enum Mode { Range, IgnoreRange, BadRange }

    fn body() -> Vec<u8> { (0..3_000_000u32).map(|i| (i % 251) as u8).collect() }

    /// Мінімальний HTTP-сервер: віддає body(), записує отримані заголовки Range.
    fn serve(mode: Mode, conns: usize) -> (String, Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/p.zip", l.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(vec![]));
        let s2 = seen.clone();
        std::thread::spawn(move || {
            let data = body();
            for _ in 0..conns {
                let Ok((mut c, _)) = l.accept() else { return };
                let mut buf = [0u8; 4096];
                let n = c.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let range = req.lines().find(|l| l.to_lowercase().starts_with("range:")).map(|l| l.trim().to_string());
                s2.lock().unwrap().push(range.clone().unwrap_or_default());
                let start = range.as_ref().and_then(|r| r.split("bytes=").nth(1)).and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                let total = data.len();
                let head;
                let slice: &[u8];
                match (start, mode) {
                    (Some(s), _) if s >= total => { head = "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(); slice = &[]; }
                    (Some(s), Mode::Range) => { head = format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {s}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", total - 1, total - s); slice = &data[s..]; }
                    (Some(s), Mode::BadRange) => { head = format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", total - 1, total - s); slice = &data[s..]; }
                    _ => { head = format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"); slice = &data[..]; }
                }
                let _ = c.write_all(head.as_bytes());
                for chunk in slice.chunks(32 * 1024) { if c.write_all(chunk).is_err() { break; } }
            }
        });
        (url, seen)
    }

    fn none(_: f32) {}

    #[tokio::test]
    async fn fresh_download() {
        let (url, _) = serve(Mode::Range, 1);
        let d = tempfile::tempdir().unwrap(); let p = d.path().join("a.part");
        download(&url, &p, &AtomicBool::new(false), &none).await.unwrap();
        assert_eq!(fs::read(&p).unwrap(), body());
    }

    #[tokio::test]
    async fn resumes_with_range() {
        let (url, seen) = serve(Mode::Range, 1);
        let d = tempfile::tempdir().unwrap(); let p = d.path().join("a.part");
        fs::write(&p, &body()[..1_000_000]).unwrap();
        download(&url, &p, &AtomicBool::new(false), &none).await.unwrap();
        assert_eq!(fs::read(&p).unwrap(), body());
        assert!(seen.lock().unwrap()[0].contains("bytes=1000000-"), "клієнт мав попросити продовження");
    }

    #[tokio::test]
    async fn server_without_range_restarts() {
        let (url, _) = serve(Mode::IgnoreRange, 1);
        let d = tempfile::tempdir().unwrap(); let p = d.path().join("a.part");
        fs::write(&p, vec![7u8; 500_000]).unwrap();
        download(&url, &p, &AtomicBool::new(false), &none).await.unwrap();
        assert_eq!(fs::read(&p).unwrap(), body(), "без Range файл має почати спочатку, а не дописатись");
    }

    #[tokio::test]
    async fn complete_file_gives_416_and_stays() {
        let (url, _) = serve(Mode::Range, 1);
        let d = tempfile::tempdir().unwrap(); let p = d.path().join("a.part");
        fs::write(&p, body()).unwrap();
        download(&url, &p, &AtomicBool::new(false), &none).await.unwrap();
        assert_eq!(fs::read(&p).unwrap(), body());
    }

    #[tokio::test]
    async fn cancel_keeps_part_then_resume_finishes() {
        let (url, _) = serve(Mode::Range, 2);
        let d = tempfile::tempdir().unwrap(); let p = d.path().join("a.part");
        let cancel = AtomicBool::new(false);
        let r = download(&url, &p, &cancel, &|pct| { if pct > 30.0 { cancel.store(true, Ordering::Relaxed) } }).await;
        assert!(matches!(r, Err(DlError::Cancelled)));
        let size = fs::metadata(&p).unwrap().len();
        assert!(size > 0 && (size as usize) < body().len(), "частина лишилась на диску: {size}");
        download(&url, &p, &AtomicBool::new(false), &none).await.unwrap();
        assert_eq!(fs::read(&p).unwrap(), body());
    }

    #[tokio::test]
    async fn wrong_content_range_is_rejected() {
        let (url, _) = serve(Mode::BadRange, 1);
        let d = tempfile::tempdir().unwrap(); let p = d.path().join("a.part");
        fs::write(&p, &body()[..1_000_000]).unwrap();
        let r = download(&url, &p, &AtomicBool::new(false), &none).await;
        assert!(matches!(r, Err(DlError::Other(_))) && !p.exists(), "некоректний діапазон: помилка й видалення частини");
    }

    #[tokio::test]
    async fn http_error_is_reported() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x", l.local_addr().unwrap());
        std::thread::spawn(move || { if let Ok((mut c, _)) = l.accept() { let mut b = [0u8; 1024]; let _ = c.read(&mut b); let _ = c.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"); } });
        let d = tempfile::tempdir().unwrap();
        let r = download(&url, &d.path().join("a.part"), &AtomicBool::new(false), &none).await;
        assert!(matches!(r, Err(DlError::Other(m)) if m.contains("404")));
    }
}
