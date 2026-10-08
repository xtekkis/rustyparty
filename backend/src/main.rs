use axum::Router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let app = Router::new();

    // Listen on all network interfaces so phones on the same Wi-Fi can connect
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
