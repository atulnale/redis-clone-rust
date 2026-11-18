#![allow(unused_imports)]

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedReadHalf},
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

    let mut buf_reader = BufReader::new(read_half);
    loop {
        let cmd_args: Vec<String> = parse_command(&mut buf_reader).await?;
        let output = match cmd_args[0].as_str() {
            "PING" => "+PONG\r\n".to_string(),
            "ECHO" => handle_echo(&cmd_args[1..]).await,
            _ => "".to_string(),
        };
        write_half.write_all(output.as_bytes()).await?
    }
}

async fn parse_command(
    buf_reader: &mut BufReader<OwnedReadHalf>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut line = String::new();
    let n = buf_reader.read_line(&mut line).await?;
    if n == 0 {
        println!("Client disconnected");
        return Ok(Vec::new());
    }
    let mut res: Vec<String> = Vec::new();
    let trimmed = line.trim();
    let count: i32 = (&trimmed[1..]).parse().unwrap();
    for i in 0..count {
        let mut len_str = String::new();
        let n = buf_reader.read_line(&mut len_str).await?;
        let len: i32 = ((&len_str.trim())[1..]).parse().unwrap();
        let mut arg = String::new();
        let n = buf_reader.read_line(&mut arg).await?;
        res.push((arg.trim()[..len as usize]).to_string());
    }
    Ok(res)
}

async fn format_output(o: &str) -> String {
    format!("${}\r\n{}\r\n", o.len(), o)
}

async fn handle_echo(args: &[String]) -> String {
    format_output(&args[0]).await
}
