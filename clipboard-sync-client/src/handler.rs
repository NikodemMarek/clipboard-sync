use std::io::Read;

use futures_util::{SinkExt, StreamExt};
use rsa::Pkcs1v15Encrypt;
use tokio::spawn;
use tokio_tungstenite::tungstenite::Message;

use crate::{config::Config, ClipboardState, BUFFER_SIZE, CONFIG};

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
    let Config { peers_keys, .. } = CONFIG.get().unwrap();

    let mut new_buffer = [0u8; BUFFER_SIZE];

    while let Ok(bytes_read @ 1..) = fifo.read(&mut new_buffer) {
        let mut state_value = state.lock().await;

        if new_buffer == *state_value {
            continue;
        }

        *state_value = new_buffer;
        new_buffer = [0u8; BUFFER_SIZE];

        let data = &state_value[..bytes_read];
        let messages = peers_keys.iter().map(|(id, key)| {
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
    state: ClipboardState,
) {
    let Config { client_key, .. } = CONFIG.get().unwrap();

    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Binary(data) = msg {
            if let Ok(data) = client_key.decrypt(Pkcs1v15Encrypt, &data) {
                let text = std::str::from_utf8(&data).unwrap();
                println!("Clipboard: {}", text);
            } else {
                println!("Failed to decrypt message");
            }
        }
    }
}

pub async fn connect(id: Box<str>) -> tokio_tungstenite::tungstenite::Result<()> {
    let Config {
        clipboard_fifo,
        relay,
        ..
    } = CONFIG.get().unwrap();
    let connection_url = &format!("ws://{}", relay);

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(connection_url).await?;
    println!("Connected to the server");
    ws_stream.send(Message::Text(id.as_ref().into())).await?;

    let (sink, stream) = ws_stream.split();

    let fifo = std::fs::OpenOptions::new()
        .read(true)
        .open(clipboard_fifo)
        .expect("Failed to open clipboard fifo");

    let current_clipboard =
        std::sync::Arc::from(futures_util::lock::Mutex::from([0u8; BUFFER_SIZE]));

    let send = spawn(send_clipboard(sink, current_clipboard.clone(), fifo));
    let recieve = spawn(recieve_clipboard(stream, current_clipboard));

    tokio::select!(
        send = send => send.expect("send failed"),
        recieve = recieve => recieve.expect("recieve failed")
    );

    Ok(())
}
