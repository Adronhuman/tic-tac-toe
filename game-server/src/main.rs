use std::{net::SocketAddr, path::PathBuf};
use axum::body::Body;
use axum::{extract::{ws::{Message, WebSocket}, ConnectInfo, WebSocketUpgrade}, http::{header, HeaderValue}, response::{Html, IntoResponse, Response}, routing::{any, get}, Router};
use futures_util::{SinkExt, StreamExt};
use tokio_util::io::ReaderStream;
use tower_http::services::ServeDir;


async fn html_handler() -> Response {
    let file = tokio::fs::File::open("..\\game-client\\static_server\\index.html").await.unwrap();
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    
    ([
        (header::CONTENT_TYPE, HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref())),
        (header::SET_COOKIE, HeaderValue::from_static("something=for_your_mind"))
    ], body)
    .into_response()
}

#[tokio::main]
async fn main() {
    
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..\\game-client\\static_server");
    println!("assets dir {}", assets_dir.to_string_lossy());
    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/", get(html_handler))
        .route("/ws", any(ws_handler));
        // .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    axum::serve(
        listener, 
        app.into_make_service_with_connect_info::<SocketAddr>()
    )
    .await
    .unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    // State(state): State<Arc<AppState>>
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}


async fn handle_socket(mut socket: WebSocket, who: SocketAddr,
    // state: Arc<AppState>
) {
    let (mut sender, mut receiver) = socket.split();

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            println!("processing message, {:?}", msg);

            match msg {
                Message::Binary(bytes) => {
                    sender.send(Message::binary(bytes)).await.ok();
                },
                _ => {}
            }
        }
    });

    recv_task.await.ok();
}