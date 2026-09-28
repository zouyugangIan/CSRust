use multithread_web_server::ThreadPool;
use std::{
    fs,
    io::{BufReader, prelude::*},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8877").unwrap();

    let pool = ThreadPool::new(4);
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        // println!("Connection established!");
        // thread::spawn(|| {
        //     handle_connection(stream);
        // });
        pool.excute(|| {
            handle_connection(stream);
        })
    }
}

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);

    let Some(Ok(request_line)) = buf_reader.lines().next() else {
        return;
    };
    let html_path = concat!(env!("CARGO_MANIFEST_DIR"), "/hello.html");
    let html_path2 = concat!(env!("CARGO_MANIFEST_DIR"), "/404.html");

    let (status_line, filename) = match &request_line[..] {
        "GET / HTTP/1.1" => ("HTTP/1.1 200 OK ", html_path),
        "GET /sleep HTTP/1.1" => {
            std::thread::sleep(Duration::from_secs(5));
            ("HTTP/1.1 200 OK ", html_path)
        }
        _ => ("HTTP/1.1 404 NOT FOUND ", html_path2),
    };

    let contents = fs::read_to_string(filename).unwrap();
    let length = contents.len();
    let response = format!(
        "{status_line}\r\n\
        Content-Length: {length}\r\n\r\n\
        {contents}"
    );
    stream.write_all(response.as_bytes()).unwrap();
}
