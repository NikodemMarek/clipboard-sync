use core::panic;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Bytes, Error, Message, Result,
    },
};

type InternalMessage = (tokio_tungstenite::tungstenite::Utf8Bytes, Bytes);

async fn accept_connection(
    peer: SocketAddr,
    stream: TcpStream,
    sender: tokio::sync::broadcast::Sender<InternalMessage>,
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
    sender: tokio::sync::broadcast::Sender<InternalMessage>,
) {
    while let Some(Ok(message)) = stream.next().await {
        if let Message::Text(id) = message {
            if let Some(Ok(Message::Binary(data))) = stream.next().await {
                let _ = sender.send((id, data));
            } else {
                println!("Failed to recieve clipboard data");
            }
        } else {
            eprintln!("Did not recieve destination identifier");
        }
    }
}

async fn send_handler(
    mut sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        Message,
    >,
    mut reciever: tokio::sync::broadcast::Receiver<InternalMessage>,
    id: tokio_tungstenite::tungstenite::Utf8Bytes,
) {
    while let Ok((recv_id, message)) = reciever.recv().await {
        if id == recv_id {
            let _ = sink.send(Message::Binary(message)).await;
        }
    }
}

async fn handle_connection(
    peer: SocketAddr,
    stream: TcpStream,
    sender: tokio::sync::broadcast::Sender<InternalMessage>,
) -> Result<()> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    println!("New incoming connection: {}", peer);

    let id = if let Some(Ok(Message::Text(id))) = ws_stream.next().await {
        id
    } else {
        eprintln!("Did not recieve identifier");
        ws_stream
            .close(Some(CloseFrame {
                code: CloseCode::Invalid,
                reason: "Did not recieve identifier".into(),
            }))
            .await?;
        return Err(Error::ConnectionClosed);
    };

    println!("New client connected: {}, with id: {}", peer, id);

    let (sink, stream) = ws_stream.split();

    let recieve = tokio::spawn(recieve_handler(stream, sender.clone()));
    let send = tokio::spawn(send_handler(sink, sender.subscribe(), id.clone()));

    tokio::select!(
        send = send => send.unwrap_or_else(|_| panic!("send failed for {}", id)),
        recieve = recieve => recieve.unwrap_or_else(|_| panic!("recieve failed for {}", id))
    );

    Ok(())
}

#[tokio::main]
async fn main() {
    let addr = "0.0.0.0:5200";
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
