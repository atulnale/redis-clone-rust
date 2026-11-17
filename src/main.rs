#![allow(unused_imports)]

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    task,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    // Uncomment the code below to pass the first stage
    //
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();

    loop {
        let (stream, address) = listener.accept().await?;
        task::spawn(async move {
            if let Err(e) = handle_stream(stream).await {
                eprintln!("Client error: {e}");
            }
        });
    }
}

async fn handle_stream(mut stream: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let (read_half, mut write_half) = stream.into_split();
    let mut line = String::new();
    let mut buf_reader = BufReader::new(read_half);
    loop {
        line.clear();
        let n = buf_reader.read_line(&mut line).await?;
        if n == 0 {
            println!("Client disconnected");
            return Ok(());
        }
        match line.trim() {
            "PING" => {
                write_half.write_all(b"+PONG\r\n").await?;
            }
            _ => {}
        }
    }
}
