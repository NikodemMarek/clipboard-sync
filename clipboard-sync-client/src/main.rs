mod config;
mod handler;

use rsa::pkcs8::{EncodePublicKey, LineEnding};
use tokio_tungstenite::tungstenite::Error;

static CONFIG: once_cell::sync::OnceCell<config::Config> = once_cell::sync::OnceCell::new();

const BUFFER_SIZE: usize = 1024 * 1024;

type ClipboardState = std::sync::Arc<futures_util::lock::Mutex<[u8; BUFFER_SIZE]>>;

#[tokio::main]
async fn main() {
    let config = config::get();

    println!("Client ID: {}", config.client_id);
    println!(
        "Client encryption key:\n{}",
        config
            .client_pub_key
            .to_public_key_pem(LineEnding::CRLF)
            .unwrap()
    );

    dbg!(&config);

    let id = config.client_id.clone();
    CONFIG.set(config).unwrap();

    if let Err(e) = handler::connect(id).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => eprintln!("Failed to connect: {}", err),
        }
    }
}
