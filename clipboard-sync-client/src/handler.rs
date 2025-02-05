use futures_util::{SinkExt, StreamExt};
use rsa::Pkcs1v15Encrypt;
use tokio::spawn;
use tokio_tungstenite::tungstenite::Message;

use crate::{config::Config, ClipboardState, BUFFER_SIZE, CONFIG};

async fn has_state_updated(state: &ClipboardState, new_content: &[u8]) -> bool {
    let mut state_content = state.lock().await;
    if new_content != state_content.as_slice() {
        *state_content = new_content.to_vec();
        true
    } else {
        false
    }
}

async fn send_clipboard(
    mut sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    state: ClipboardState,
) {
    let Config { peers_keys, .. } = CONFIG.get().unwrap();

    let mut clipboard = arboard::Clipboard::new().unwrap();

    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));

        let clipboard_content = clipboard.get_text();
        if clipboard_content.is_err() {
            eprintln!("Failed to get clipboard content");
            continue;
        }
        let clipboard_content = clipboard_content.unwrap();
        let clipboard_content = clipboard_content.as_bytes();

        if !has_state_updated(&state, clipboard_content).await {
            continue;
        }

        println!(
            "Clipboard: {}",
            std::str::from_utf8(clipboard_content).unwrap()
        );
        let messages = peers_keys.iter().map(|(id, key)| {
            (
                id,
                key.encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, clipboard_content),
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

    let mut clipboard = arboard::Clipboard::new().unwrap();

    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Binary(data) = msg {
            if let Ok(data) = client_key.decrypt(Pkcs1v15Encrypt, &data) {
                if !has_state_updated(&state, &data).await {
                    continue;
                }

                let text = std::str::from_utf8(&data).unwrap();
                if clipboard.set_text(text).is_err() {
                    println!("Failed to set recieved clipboard content");
                }
                println!("Clipboard: {}", text);
            } else {
                println!("Failed to decrypt message");
            }
        }
    }
}

pub async fn connect(id: Box<str>) -> tokio_tungstenite::tungstenite::Result<()> {
    let Config { relay, .. } = CONFIG.get().unwrap();
    let connection_url = &format!("ws://{}", relay);

    let (mut ws_stream, _) = tokio_tungstenite::connect_async(connection_url).await?;
    println!("Connected to the server");
    ws_stream.send(Message::Text(id.as_ref().into())).await?;

    let (sink, stream) = ws_stream.split();

    let current_clipboard = std::sync::Arc::from(futures_util::lock::Mutex::from(
        Vec::with_capacity(BUFFER_SIZE),
    ));

    let send = spawn(send_clipboard(sink, current_clipboard.clone()));
    let recieve = spawn(recieve_clipboard(stream, current_clipboard));

    tokio::select!(
        send = send => send.expect("send failed"),
        recieve = recieve => recieve.expect("recieve failed")
    );

    Ok(())
}
