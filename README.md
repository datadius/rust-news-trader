# rust-news-trader

Example of websocket in rust.

Thanks tesioai.

https://github.com/snapview/tokio-tungstenite/issues/137#issuecomment-1806628568

```rust
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::connect_async;
use futures::{ SinkExt, StreamExt };

#[tokio::main]
async fn main() {
    let url = "ws://localhost:3000/socket";

    // connect to socket
    if let Ok((mut socket, _)) = connect_async(url).await {
        println!("Successfully connected to the WebSocket");
        
        // create message
        let message = Message::from("message");

        // send message
        if let Err(e) = socket.send(message).await {
            eprintln!("Error sending message: {:?}", e);
        }

        // recieve response
        if let Some(Ok(response)) = socket.next().await {
            println!("{response}");
        }
    } else {
        eprintln!("Failed to connect to the WebSocket");
    }
}
```
