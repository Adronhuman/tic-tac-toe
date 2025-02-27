use std::collections::{vec_deque, HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::u64;
use std::{net::SocketAddr, path::PathBuf};
use axum::body::Body;
use axum::extract::ws::Utf8Bytes;
use axum::extract::State;
use axum::{extract::{ws::{Message, WebSocket}, ConnectInfo, WebSocketUpgrade}, http::{header, HeaderValue}, response::{Html, IntoResponse, Response}, routing::{any, get}, Router};
use axum_extra::headers::Cookie;
use axum_extra::TypedHeader;
use futures_util::{lock, SinkExt, StreamExt};
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio_util::io::ReaderStream;
use tower_http::services::ServeDir;

static PLAYER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

async fn html_handler(TypedHeader(cookie): TypedHeader<Cookie>) -> Response {
    let player_id = match cookie.get("PLAYER_ID") {
        Some(player_id) => player_id.to_string(),
        None => {
            let id = PLAYER_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            id.to_string()
        }
    };

    let file = tokio::fs::File::open("..\\game-client\\static_server\\index.html").await.unwrap();
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    
        ([
            (header::CONTENT_TYPE, HeaderValue::from_static(mime::TEXT_HTML_UTF_8.as_ref())),
            (header::SET_COOKIE, HeaderValue::from_str(format!("PLAYER_ID={player_id}").as_str()).unwrap())
        ], body)
        .into_response()
}

#[derive(PartialEq, Copy, Clone)]
#[repr(u8)]
enum PlayerType {
    x_player = 1,
    o_player = 2
}
impl PlayerType {
    fn next(self) -> PlayerType {
        match self {
            PlayerType::x_player => PlayerType::o_player,
            PlayerType::o_player => PlayerType::x_player
        }
    }
}

struct GameState {
    board: [u8; 9],
    turn: PlayerType,
    you: PlayerType
}
impl GameState {
    fn validate_move(&self, mv: usize, player: PlayerType) -> bool {
        if self.board[mv] != 0 {
            return false;
        }
        let xs = self.board.iter().filter(|x| **x == 1).count();
        let os = self.board.iter().filter(|x| **x == 2).count();

        if xs == os {
            player == PlayerType::x_player
        } else {
            player == PlayerType::o_player
        }
    }

    fn check_win(&self) -> Option<PlayerType> {
        let winning_combinations: [[usize; 3]; 8] = [
            [0, 1, 2], // Row 1
            [3, 4, 5], // Row 2
            [6, 7, 8], // Row 3
            [0, 3, 6], // Column 1
            [1, 4, 7], // Column 2
            [2, 5, 8], // Column 3
            [0, 4, 8], // Diagonal 1
            [2, 4, 6], // Diagonal 2
        ];

        for combo in &winning_combinations {
            if self.check_combination(combo, PlayerType::x_player) {
                return Some(PlayerType::x_player);
            }
            if self.check_combination(combo, PlayerType::o_player) {
                return Some(PlayerType::o_player);
            }
        }
        None
    }

    fn check_combination(&self, combo: &[usize; 3], player: PlayerType) -> bool {
        let player = player as u8;
        let (i1, i2, i3) = (combo[0], combo[1], combo[1]);
        self.board[i1] == player && self.board[i2] == player && self.board[i3] == player
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            board: [0; 9],      // Empty board
            turn: PlayerType::x_player,      // Assuming X starts first
            you: PlayerType::o_player
        }
    }
}

struct AppState {
    players_connections: Mutex<HashMap<usize, Sender<String>>>,
    players_queue: Mutex<VecDeque<GameRequest>>,
    players_writer: Sender<usize>
}

struct GameRequest {
    messenger: Sender<String>,
    call_me_back: oneshot::Sender<GameRequest>
}

#[tokio::main]
async fn main() {
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..\\game-client\\static_server");
    
    let players_connections = Mutex::new(HashMap::<usize, Sender<String>>::new());
    let (players_writer, players_reader) = channel::<usize>(10);
    
    // let players_games = Mutex::new(HashMap::<u64, Game>::new());
    let app_state = Arc::new(AppState {
        players_connections,
        players_queue: Mutex::new(VecDeque::new()),
        players_writer,
    });
    
    let app = Router::new()
        .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
        .route("/", get(html_handler))
        .route("/ws", any(ws_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    axum::serve(
        listener, 
        app.into_make_service_with_connect_info::<SocketAddr>()
    )
    .await
    .unwrap();
}

async fn ws_handler(
    TypedHeader(cookie): TypedHeader<Cookie>,
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>
) -> impl IntoResponse {
    println!("ws handler player id cookie {}", cookie.get("PLAYER_ID").or(Some("no cookie")).unwrap());
    let player_id = cookie.get("PLAYER_ID").or(Some("no cookie")).unwrap();
    let player_id = player_id.parse::<u64>().unwrap();

    ws.on_upgrade(move |socket| handle_socket(socket, addr, state, player_id))
}


async fn handle_socket(mut socket: WebSocket, who: SocketAddr,
    state: Arc<AppState>, this_player: u64
) {
    // Matchmaking
    let borrowed_state = Arc::clone(&state);
    
    let mut player_queue = borrowed_state.players_queue.lock().await;

    let (tx_messenger, mut my_messenger) = channel(10);
    let (tx_machmaking, rx_matchmaking) = oneshot::channel();
    let mut game_state = Arc::new(Mutex::new(GameState::default()));
    
    let my_handle = GameRequest {
        messenger: tx_messenger,
        call_me_back: tx_machmaking
    };

    let opponent_messenger = if let Some(opponent) = player_queue.pop_front() {
        opponent.call_me_back.send(my_handle).ok();
        drop(player_queue);

        opponent.messenger
    } else {
        player_queue.push_back(my_handle);
        drop(player_queue);
        
        let mut game_state_locked = game_state.lock().await;
        game_state_locked.you = PlayerType::x_player;
        drop(game_state_locked);

        let opponent = rx_matchmaking.await.unwrap();
        opponent.messenger
    };

    // Game started

    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));

    let borrowed_state = Arc::clone(&state);
    let game_state_for_this_player = Arc::clone(&game_state);
    let sender_for_this_player = Arc::clone(&sender);
    let recv_client_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Binary(bytes) => {
                    // sender.send(Message::binary(bytes)).await.ok();
                    let players_connections = borrowed_state.players_connections.lock().await;
                    let x = players_connections.get(&1);
                    if let Some(x) = x {
                        if let Ok(_) = x.send(format!("Player {this_player} says hello with binary data")) {

                        } else {
                            println!("couldn't send");
                        }
                    } else {
                       println!("number 1 was not found");
                    }
                },
                Message::Text(str) => {
                    println!("received text message from client {str}");
                    let mut game_state = game_state_for_this_player.lock().await;
                    let mv = str.to_string().parse::<usize>().unwrap();
                    if !game_state.validate_move(mv, game_state.you) {
                        println!("received invalid move");
                        continue;
                    }

                    game_state.board[mv] = game_state.you as u8;
                    game_state.turn = game_state.turn.next();
                    println!("game board state: {:?}", game_state.board);
                    if let Some(won) = game_state.check_win() {
                        let mut sender = sender_for_this_player.lock().await;
                        sender.send(Message::Text(Utf8Bytes::from("you won!!!"))).await.unwrap();
                    } 
                    if let Err(err) = opponent_messenger.send(str.to_string()) {
                        println!("wtf error is {err}");
                    }
                },
                _ => {}
            }
        }
    });

    let (tx, mut rx) = channel(1);

    let game_state_ref = Arc::clone(&game_state);
    let recv_others_task = tokio::spawn(async move {
        while let Ok(msg) = my_messenger.recv().await {
            let mut game_state = game_state_ref.lock().await;
            let mv = msg.parse::<usize>().unwrap();
            if !game_state.validate_move(mv, game_state.you.next()) {
                println!("received invalid move");
                continue;
            }

            game_state.board[mv] = game_state.you.next() as u8;
            game_state.turn = game_state.turn.next();

            let mut sender = sender.lock().await;
            if let Some(won) = game_state.check_win() {
                sender.send(Message::Text(Utf8Bytes::from("you lose!!!"))).await.unwrap();    
            } else {
                sender.send(Message::Text(Utf8Bytes::from(msg))).await.unwrap();
            }
        }
    });

    let mut lck = state.players_connections.lock().await;
    lck.insert(1, tx);
    drop(lck);

    tokio::select! {
        _ = recv_client_task => {

        },
        _ = recv_others_task => {

        } 
    }

    println!("Socket destroyed");
}