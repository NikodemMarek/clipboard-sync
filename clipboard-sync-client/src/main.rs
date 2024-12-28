use core::str;
use std::{collections::HashMap, fs::OpenOptions, io::Read, sync::Arc};

use futures_util::{lock::Mutex, SinkExt, StreamExt};
use rsa::{
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePublicKey, LineEnding},
    Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey,
};
use tokio::spawn;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error, Message, Result},
};

#[derive(Debug)]
struct Config {
    fifo_path: Box<str>,
    relay_addr: Box<str>,
    priv_key: RsaPrivateKey,
    pub_keys: HashMap<Box<str>, RsaPublicKey>,
}

static CONFIG: once_cell::sync::OnceCell<Config> = once_cell::sync::OnceCell::new();

const BUFFER_SIZE: usize = 1024 * 1024;

type ClipboardState = Arc<Mutex<[u8; BUFFER_SIZE]>>;

async fn send_clipboard(
    mut sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    state: ClipboardState,
    mut fifo: std::fs::File,
) {
    let Config { pub_keys, .. } = CONFIG.get().unwrap();

    let mut new_buffer = [0u8; BUFFER_SIZE];

    while let Ok(bytes_read @ 1..) = fifo.read(&mut new_buffer) {
        let mut state_value = state.lock().await;

        if new_buffer == *state_value {
            continue;
        }

        *state_value = new_buffer;
        new_buffer = [0u8; BUFFER_SIZE];

        let data = &state_value[..bytes_read];
        let messages = pub_keys.iter().map(|(id, key)| {
            (
                id,
                key.encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, data),
            )
        });

        for (id, message) in messages {
            if let Ok(message) = message {
                let _ = sink.send(Message::Text(id.as_ref().into())).await;
                let _ = sink.send(Message::Binary(message.into())).await;
            } else {
                eprintln!("Failed to encrypt clipboard for {}", id);
            }
        }
    }
}
async fn recieve_clipboard(
    mut stream: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    state: Arc<Mutex<[u8; BUFFER_SIZE]>>,
) {
    let Config { priv_key, .. } = CONFIG.get().unwrap();

    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Binary(data) = msg {
            if let Ok(data) = priv_key.decrypt(Pkcs1v15Encrypt, &data) {
                let text = std::str::from_utf8(&data).unwrap();
                println!("Clipboard: {}", text);
            } else {
                println!("Failed to decrypt message");
            }
        }
    }
}

async fn connect(id: Box<str>) -> Result<()> {
    let Config {
        fifo_path,
        relay_addr,
        ..
    } = CONFIG.get().unwrap();
    let connection_url = &format!("ws://{}", relay_addr);

    let (mut ws_stream, _) = connect_async(connection_url).await?;
    println!("Connected to the server");
    ws_stream.send(Message::Text(id.as_ref().into())).await?;

    let (sink, stream) = ws_stream.split();

    let fifo = OpenOptions::new()
        .read(true)
        .open(fifo_path.to_string())
        .unwrap();

    let current_clipboard = Arc::from(Mutex::from([0u8; BUFFER_SIZE]));

    let send = spawn(send_clipboard(sink, current_clipboard.clone(), fifo));
    let recieve = spawn(recieve_clipboard(stream, current_clipboard));

    tokio::select!(
        send = send => send.expect("send failed"),
        recieve = recieve => recieve.expect("recieve failed")
    );

    Ok(())
}

fn compute_id(key: &RsaPublicKey) -> Box<str> {
    key.to_public_key_der()
        .map(|d| sha256::digest(d.as_bytes()).into())
        .expect("failed to compute id")
}

fn get_id_key_from_file(path: &str) -> (Box<str>, RsaPublicKey) {
    let key = RsaPublicKey::read_public_key_pem_file(path).expect("failed to get a key from file");
    (compute_id(&key), key)
}

#[tokio::main]
async fn main() {
    let priv_key =
        RsaPrivateKey::read_pkcs8_pem_file("../secret.key").expect("failed to get a key from file");
    let pub_key = RsaPublicKey::from(&priv_key);
    let id = compute_id(&pub_key);
    println!(
        "Public key:\n{}",
        pub_key.to_public_key_pem(LineEnding::LF).unwrap()
    );
    println!("ID: {}", id);

    let client_1 = get_id_key_from_file("../client_1.pub");
    let pub_keys = HashMap::from([client_1]);

    let config = Config {
        fifo_path: "/tmp/clipboard.pipe".into(),
        // relay_addr: "130.61.88.218:5200".into(),
        relay_addr: "127.0.0.1:5200".into(),
        priv_key,
        pub_keys,
    };
    CONFIG.set(config).unwrap();

    if let Err(e) = connect(id).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => eprintln!("Failed to connect: {}", err),
        }
    }
}
