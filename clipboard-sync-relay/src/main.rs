use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Bytes, Error, Message, Result},
};

async fn accept_connection(
    peer: SocketAddr,
    stream: TcpStream,
    sender: tokio::sync::broadcast::Sender<Bytes>,
) {
    if let Err(e) = handle_connection(peer, stream, sender).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => eprintln!("Error processing connection: {}", err),
        }
    }
}

async fn recieve_handler(
    mut stream: futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<TcpStream>>,
    sender: tokio::sync::broadcast::Sender<Bytes>,
) {
    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Binary(data) = msg {
            let _ = sender.send(data);
        }
    }
}

async fn send_handler(
    mut sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        Message,
    >,
    mut reciever: tokio::sync::broadcast::Receiver<Bytes>,
) {
    while let Ok(v) = reciever.recv().await {
        let _ = sink.send(Message::Binary(v)).await;
    }
}

async fn handle_connection(
    peer: SocketAddr,
    stream: TcpStream,
    sender: tokio::sync::broadcast::Sender<Bytes>,
) -> Result<()> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");
    let (sink, stream) = ws_stream.split();

    println!("New WebSocket connection: {}", peer);

    let recieve = tokio::spawn(recieve_handler(stream, sender.clone()));
    let send = tokio::spawn(send_handler(sink, sender.subscribe()));

    tokio::select!(
        send = send => send.expect("send failed"),
        recieve = recieve => recieve.expect("recieve failed")
    );

    Ok(())
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:5200";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    println!("Listening on: {}", addr);

    let clipboard_broadcast = tokio::sync::broadcast::channel(10);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        println!("Peer address: {}", peer);

        let rx = clipboard_broadcast.0.clone();
        tokio::spawn(accept_connection(peer, stream, rx));
    }
}
