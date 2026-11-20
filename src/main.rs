#![allow(unused_imports)]

use std::{collections::HashMap, sync::LazyLock, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedReadHalf},
    sync::RwLock,
    task,
    time::sleep,
};

static STORE: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
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
            "SET" => handle_set(&cmd_args[1..]).await,
            "GET" => handle_get(&cmd_args[1..]).await,
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
async fn handle_set(args: &[String]) -> String {
    {
        let mut db = STORE.write().await;
        db.insert(args[0].clone(), args[1].clone());
    }
    if args.len() > 2 {
        let key = args[0].clone();
        match args[2].as_str() {
            "PX" => {
                let duration = get_duration(args[3].parse().unwrap(), "PX");
                tokio::spawn(async move {
                    set_expiry(duration).await;
                    remove_key(key).await;
                });
            }
            "EX" => {
                let duration = get_duration(args[3].parse().unwrap(), "EX");
                tokio::spawn(async move {
                    set_expiry(duration).await;
                    remove_key(key).await;
                });
            }
            _ => {}
        }
    }
    format_simple_output("OK")
}

async fn handle_get(args: &[String]) -> String {
    let val = read_db(&args[0]).await;
    let res = match val {
        Some(x) => format_bulk_output(x.as_str()),
        None => format_bulk_output(""),
    };

    res
}
async fn remove_key(key: String) {
    let mut db = STORE.write().await;
    db.remove(&key);
}
async fn read_db(key: &str) -> Option<String> {
    let db = STORE.read().await;
    db.get(key).cloned()
}

fn format_simple_output(o: &str) -> String {
    format!("+{}\r\n", o)
}
fn format_bulk_output(o: &str) -> String {
    if o.is_empty() {
        return format!("${}\r\n", -1);
    }
    format!("${}\r\n{}\r\n", o.len(), o)
}

async fn handle_echo(args: &[String]) -> String {
    format_bulk_output(&args[0])
}

async fn set_expiry(duration: Duration) {
    sleep(duration).await;
}

fn get_duration(value: i64, flag: &str) -> Duration {
    match flag {
        "PX" => Duration::from_millis(value as u64),
        "EX" => Duration::from_secs(value as u64),
        _ => Duration::from_hours(2),
    }
}
