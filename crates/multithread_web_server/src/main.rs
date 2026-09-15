use std::{
    arch::x86_64, fs, io::{BufReader, prelude::*}, net::{TcpListener, TcpStream}, os::unix::fs,
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8877").unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        // println!("Connection established!");
        //
        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);

    let Some(Ok(request_line)) = buf_reader.lines().next() else {
        return;
    };
    let html_path = concat!(env!("CARGO_MANIFEST_DIR"), "/hello.html");
    let html_path2 = concat!(env!("CARGO_MANIFEST_DIR"), "/404.html");

    let (status_line, filename) =
        if request_line = "GET / HTTP/1.1"{
            ("HTTP/1.1 200 OK ",html_path)
        }else{
            ("HTTP/1.1 404 NOT FOUND ",html_path2)
            }

    let contents = fs::read_to_string(filename).unwrap();
    let length = contents.len();
    let response = format!(
        "{status_line}\r\n\
        Content-Length: {length}\r\n\r\n\
        {contents}"
    );
        stream.write_all(response.as_bytes()).unwrap();
    }
}
