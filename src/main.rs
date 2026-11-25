#![allow(unused_imports)]

use std::{collections::HashMap, sync::LazyLock, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedReadHalf},
    sync::RwLock,
    task,
    time::sleep,
};

use crate::EncodingType::{ARRAY, BULK, ERROR, INTEGER, SIMPLE};

enum EncodingType {
    BULK(String),
    SIMPLE(String),
    INTEGER(i32),
    ERROR(String),
    ARRAY(Vec<String>),
}

static STORE: LazyLock<RwLock<HashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

static RPUSH_LIST: LazyLock<RwLock<HashMap<String, Vec<String>>>> =
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
            "RPUSH" => handle_rpush(&cmd_args[1..]).await,
            "LRANGE" => handle_lrange(&cmd_args[1..]).await,
            _ => "".to_string(),
        };
        write_half.write_all(output.as_bytes()).await?
    }
}

async fn handle_rpush(args: &[String]) -> String {
    if args.len() < 2 {
        return format_output(ERROR(
            "wrong number of arguments for 'rpush' command".to_string(),
        ));
    }
    let count;
    {
        let mut writer = RPUSH_LIST.write().await;
        let mut list = writer.entry(args[0].clone()).or_default();
        list.extend(args[1..].iter().cloned());
        count = list.len();
    }

    format_output(EncodingType::INTEGER(count as i32))
}

async fn handle_lrange(args: &[String]) -> String {
    if args.len() != 3 {
        return format_output(ERROR(
            "wrong number of arguments for 'lrange' command".to_string(),
        ));
    }
    let start: i32 = args[1].clone().parse().unwrap();
    let end: i32 = args[2].clone().parse().unwrap();
    let reader = RPUSH_LIST.read().await;
    match reader.get(&args[0]) {
        Some(list) => {
            let start = get_lrange_index(list.len(), start);
            let end = get_lrange_index(list.len(), end);

            if end < start || start >= list.len() {
                return format_output(ARRAY(Vec::new()));
            }
            let end = end.min(list.len() - 1);
            eprint!("start{} end{}", start, end);
            dbg!(start);
            dbg!(end);
            let temp: Vec<String> = list[start..=end].iter().cloned().collect();
            format_output(ARRAY(temp))
        }
        None => format_output(ARRAY(Vec::new())),
    }
}
fn get_lrange_index(len: usize, index: i32) -> usize {
    if index < 0 && index.abs() > len as i32 {
        return 0;
    }
    return if index < 0 {
        (len as i32 + index) as usize
    } else {
        index as usize
    };
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
    format_output(EncodingType::SIMPLE("OK".to_string()))
}

async fn handle_get(args: &[String]) -> String {
    let val = read_db(&args[0]).await;
    let res = match val {
        Some(x) => format_output(EncodingType::BULK(x)),
        None => format_output(EncodingType::BULK("".to_string())),
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

fn format_output(encoding_type: EncodingType) -> String {
    match encoding_type {
        BULK(s) => {
            if s.is_empty() {
                return format!("${}\r\n", -1);
            }
            return format!("${}\r\n{}\r\n", s.len(), s);
        }
        SIMPLE(s) => format!("+{}\r\n", s),
        INTEGER(i) => format!(":{}\r\n", i),
        ERROR(s) => format!("-ERR {}\r\n", s),
        ARRAY(arr) => {
            let mut res = format!("*{}\r\n", arr.len());
            for ele in arr {
                res.push_str(format!("${}\r\n{}\r\n", ele.len(), ele).as_str());
            }
            res
        }
    }
}

async fn handle_echo(args: &[String]) -> String {
    format_output(EncodingType::BULK(args[0].clone()))
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
