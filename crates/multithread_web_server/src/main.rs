use multithread_web_server::ThreadPool;
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    time::Duration,
};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const ADDRESS: &str = "127.0.0.1:8877";

fn main() -> io::Result<()> {
    let listener = TcpListener::bind(ADDRESS)?;
    let pool = ThreadPool::new(4);
    println!("生日页面已就绪：http://{ADDRESS}");
    println!("按 Ctrl+C 停止服务器。");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => pool.execute(move || {
                if let Err(error) = handle_connection(stream) {
                    eprintln!("连接结束：{error}");
                }
            }),
            Err(error) => eprintln!("无法接收连接：{error}"),
        }
    }
    Ok(())
}

struct Response {
    status: &'static str,
    content_type: &'static str,
    extra_headers: String,
    body: Vec<u8>,
}

impl Response {
    fn text(status: &'static str, text: &str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            extra_headers: String::new(),
            body: text.as_bytes().to_vec(),
        }
    }

    fn file(filename: &str, content_type: &'static str) -> Self {
        match fs::read(Path::new(ROOT).join(filename)) {
            Ok(body) => Self {
                status: "200 OK",
                content_type,
                extra_headers: String::new(),
                body,
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Self::text("404 Not Found", "没有找到这个文件。")
            }
            Err(_) => Self::text("500 Internal Server Error", "暂时无法读取文件。"),
        }
    }

    fn with_range(mut self, range: Option<&str>) -> Self {
        if self.status != "200 OK" {
            return self;
        }
        self.extra_headers = "Accept-Ranges: bytes\r\n".to_owned();
        let Some(range) = range else {
            return self;
        };
        let length = self.body.len();
        match byte_range(range, length) {
            Some((start, end)) => {
                self.status = "206 Partial Content";
                self.extra_headers
                    .push_str(&format!("Content-Range: bytes {start}-{end}/{length}\r\n"));
                self.body = self.body[start..=end].to_vec();
            }
            None => {
                self.status = "416 Range Not Satisfiable";
                self.extra_headers
                    .push_str(&format!("Content-Range: bytes */{length}\r\n"));
                self.body.clear();
            }
        }
        self
    }
}

// A browser requests byte ranges when loading metadata or seeking in a video.
fn byte_range(range: &str, length: usize) -> Option<(usize, usize)> {
    if length == 0 {
        return None;
    }
    let (start, end) = range.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix: usize = end.parse().ok()?;
        return (suffix > 0).then(|| (length.saturating_sub(suffix), length - 1));
    }
    let start: usize = start.parse().ok()?;
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<usize>().ok()?.min(length - 1)
    };
    (start <= end).then_some((start, end))
}

fn response_for(path: &str) -> Response {
    let path = path.split('?').next().unwrap_or(path);
    match path {
        "/" | "/birthday" | "/babyHappyBirthDay.html" => {
            Response::file("babyHappyBirthDay.html", "text/html; charset=utf-8")
        }
        "/images/1.jpg" => Response::file("images/1.jpg", "image/jpeg"),
        "/images/2.jpg" => Response::file("images/2.jpg", "image/jpeg"),
        "/images/3.MP4" | "/images/3.mp4" => Response::file("images/3.MP4", "video/mp4"),
        "/hello.html" => Response::file("hello.html", "text/html; charset=utf-8"),
        "/sleep" => {
            std::thread::sleep(Duration::from_secs(5));
            Response::file("hello.html", "text/html; charset=utf-8")
        }
        "/favicon.ico" => Response::text("204 No Content", ""),
        _ => {
            let mut response = Response::file("404.html", "text/html; charset=utf-8");
            response.status = "404 Not Found";
            response
        }
    }
}

fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Bound request headers so one client cannot consume unlimited memory.
    let mut reader = BufReader::new((&stream).take(16 * 1024));
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let mut header = String::new();
    let mut range = None;
    loop {
        header.clear();
        if reader.read_line(&mut header)? == 0 {
            return Ok(());
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_owned());
            }
        }
    }

    let parts: Vec<_> = request_line.split_whitespace().collect();
    let is_head = parts.first() == Some(&"HEAD");
    let response = match parts.as_slice() {
        ["GET" | "HEAD", path, "HTTP/1.1" | "HTTP/1.0"] => response_for(path),
        [_, _, "HTTP/1.1" | "HTTP/1.0"] => {
            Response::text("405 Method Not Allowed", "请使用 GET 或 HEAD。")
        }
        _ => Response::text("400 Bad Request", "无法识别这个请求。"),
    };
    let response = if response.content_type == "video/mp4" {
        response.with_range(if is_head { None } else { range.as_deref() })
    } else {
        response
    };
    write!(
        stream,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\n{}{}\r\n",
        response.status,
        response.content_type,
        response.body.len(),
        response.extra_headers,
        if response.status == "405 Method Not Allowed" {
            "Allow: GET, HEAD\r\n"
        } else {
            ""
        },
    )?;
    if !is_head {
        stream.write_all(&response.body)?;
    }
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn birthday_and_assets_are_served_from_any_working_directory() {
        for path in ["/", "/birthday", "/babyHappyBirthDay.html?from=test"] {
            let response = response_for(path);
            assert_eq!(response.status, "200 OK");
            assert_eq!(response.content_type, "text/html; charset=utf-8");
            assert!(
                String::from_utf8(response.body)
                    .unwrap()
                    .contains("生日快乐")
            );
        }
    }

    #[test]
    fn photos_are_sent_as_original_binary_data() {
        for name in ["1.jpg", "2.jpg"] {
            let response = response_for(&format!("/images/{name}"));
            assert_eq!(response.status, "200 OK");
            assert_eq!(response.content_type, "image/jpeg");
            assert_eq!(
                response.body,
                fs::read(Path::new(ROOT).join("images").join(name)).unwrap()
            );
        }
    }

    #[test]
    fn private_files_and_path_traversal_are_not_served() {
        for path in [
            "/Cargo.toml",
            "/src/main.rs",
            "/images/../Cargo.toml",
            "/images/%2e%2e/Cargo.toml",
            "/missing",
        ] {
            assert_eq!(response_for(path).status, "404 Not Found", "{path}");
        }
    }

    #[test]
    fn video_ranges_support_playback_and_seeking() {
        assert_eq!(byte_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(byte_range("bytes=500-", 1000), Some((500, 999)));
        assert_eq!(byte_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(byte_range("bytes=990-2000", 1000), Some((990, 999)));
        assert_eq!(byte_range("bytes=1000-", 1000), None);
        assert_eq!(byte_range("bytes=50-20", 1000), None);
        assert_eq!(byte_range("bytes=-0", 1000), None);
        assert_eq!(byte_range("bytes=0-1,5-6", 1000), None);

        let partial = Response::text("200 OK", "abcdefghij").with_range(Some("bytes=2-4"));
        assert_eq!(partial.status, "206 Partial Content");
        assert_eq!(partial.body, b"cde");
        assert!(
            partial
                .extra_headers
                .contains("Content-Range: bytes 2-4/10\r\n")
        );
        let invalid = Response::text("200 OK", "abc").with_range(Some("bytes=5-"));
        assert_eq!(invalid.status, "416 Range Not Satisfiable");
        assert!(invalid.body.is_empty());
        assert!(
            invalid
                .extra_headers
                .contains("Content-Range: bytes */3\r\n")
        );
    }

    #[test]
    fn head_has_the_same_length_as_get_without_a_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream).unwrap();
        });
        client
            .write_all(b"HEAD / HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut result = String::new();
        client.read_to_string(&mut result).unwrap();
        server.join().unwrap();
        assert!(result.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(result.contains(&format!(
            "Content-Length: {}\r\n",
            response_for("/").body.len()
        )));
        assert!(result.ends_with("\r\n\r\n"));
    }
}
