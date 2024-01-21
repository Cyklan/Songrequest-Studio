use std::time::Duration;

use poem::{
    get, handler,
    web::sse::{Event, SSE},
    Route,
};
use stream::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::{self as stream, wrappers::ReceiverStream};

#[handler]
fn index() -> SSE {
    let (tx, rx) = mpsc::channel::<String>(32);
    tokio::spawn(async move { check_spotify(tx).await });
    let stream = ReceiverStream::new(rx).map(|msg| Event::message(msg));

    return SSE::new(stream);
}

async fn check_spotify(tx: mpsc::Sender<String>) {
    let mut counter = 0;

    loop {
        println!("still running");
        counter += 1;
        if let Err(_) = tx.send(format!("Hello {}", counter)).await {
            break;
        };
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub fn api() -> Route {
    return Route::new().at("/", get(index));
}
