use std::{
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};

const ADDRESS: &str = "0.0.0.0:7878";
const HOME_PAGE: &str = include_str!("../hello.html");

fn main() -> io::Result<()> {
    let listener = TcpListener::bind(ADDRESS).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("无法监听 {ADDRESS}：{error}。请检查 7878 端口是否已被其他进程占用"),
        )
    })?;

    println!("服务器已启动，正在监听 {ADDRESS}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream) {
                    eprintln!("处理连接失败：{error}");
                }
            }
            Err(error) => eprintln!("接收连接失败：{error}"),
        }
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> io::Result<()> {
    let buf_reader = BufReader::new(&stream);
    for line in buf_reader.lines() {
        if line?.is_empty() {
            break;
        }
    }

    let status_line = "HTTP/1.1 200 OK";
    let length = HOME_PAGE.len();
    let response = format!(
        "{status_line}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {length}\r\n\
         Connection: close\r\n\r\n\
         {HOME_PAGE}"
    );
    stream.write_all(response.as_bytes())
}
