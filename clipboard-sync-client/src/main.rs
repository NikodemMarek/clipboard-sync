use std::{fs::OpenOptions, io::Read, sync::Arc};

use futures_util::{lock::Mutex, SinkExt, StreamExt};
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
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
    pub_keys: Box<[RsaPublicKey]>,
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
        let messages = pub_keys
            .iter()
            .map(|key| key.encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, data));

        for message in messages {
            let message = message.unwrap();
            let _ = sink.send(Message::Binary(message.into())).await;
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
                let data = priv_key.decrypt(Pkcs1v15Encrypt, &data).unwrap();
                let text = std::str::from_utf8(&data).unwrap();
                println!("Clipboard: {}", text);
            } else {
                println!("Failed to decrypt message");
            }
        }
    }
}

async fn connect() -> Result<()> {
    let Config {
        fifo_path,
        relay_addr,
        ..
    } = CONFIG.get().unwrap();
    let connection_url = &format!("ws://{}", relay_addr);

    let (ws_stream, _) = connect_async(connection_url).await?;
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

#[tokio::main]
async fn main() {
    let bits = 2048;
    let mut rng = rand::thread_rng();

    let priv_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
    let pub_key = RsaPublicKey::from(&priv_key);

    let pub_keys = Box::new([pub_key]);

    let config = Config {
        fifo_path: "/tmp/clipboard.pipe".into(),
        relay_addr: "127.0.0.1:5200".into(),
        priv_key,
        pub_keys,
    };
    CONFIG.set(config).unwrap();

    if let Err(e) = connect().await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => eprintln!("Failed to connect: {}", err),
        }
    }
}
