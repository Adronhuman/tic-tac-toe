use std::collections::{vec_deque, HashMap, HashSet, VecDeque};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::u64;
use std::{net::SocketAddr, path::PathBuf};
use axum::body::{Body, Bytes};
use axum::extract::ws::Utf8Bytes;
use axum::extract::State;
use axum::{extract::{ws::{Message, WebSocket}, ConnectInfo, WebSocketUpgrade}, http::{header, HeaderValue}, response::{Html, IntoResponse, Response}, routing::{any, get}, Router};
use axum_extra::headers::Cookie;
use axum_extra::TypedHeader;
use futures_util::{lock, SinkExt, StreamExt};
use messages::game::server_message::Message as Com_Message;
use messages::game::{self, GameFinished, InitGame, PlayerMove, PlayerType, ServerMessage};
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

trait IPlayerType {
    fn next(self) -> PlayerType;
}

impl IPlayerType for PlayerType {
    fn next(self) -> PlayerType {
        match self {
            PlayerType::X => PlayerType::O,
            PlayerType::O => PlayerType::X
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
        if self.board[mv] != 228 {
            return false;
        }
        let xs = self.board.iter().filter(|x| **x == PlayerType::X as u8).count();
        let os = self.board.iter().filter(|x| **x == PlayerType::O as u8).count();

        if xs == os {
            player == PlayerType::X
        } else {
            player == PlayerType::O
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
            if self.check_combination(combo, PlayerType::X) {
                return Some(PlayerType::X);
            }
            if self.check_combination(combo, PlayerType::O) {
                return Some(PlayerType::O);
            }
        }
        None
    }

    fn check_combination(&self, combo: &[usize; 3], player: PlayerType) -> bool {
        let player = player as u8;
        let (i1, i2, i3) = (combo[0], combo[1], combo[2]);
        self.board[i1] == player && self.board[i2] == player && self.board[i3] == player
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            board: [228; 9],      // Empty board
            turn: PlayerType::X,      // Assuming X starts first
            you: PlayerType::O
        }
    }
}

struct AppState {
    players_connections: Mutex<HashMap<usize, Sender<String>>>,
    players_queue: Mutex<VecDeque<GameRequest>>,
    players_writer: Sender<usize>
}

struct GameRequest {
    messenger: Sender<Bytes>,
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

    let listener = tokio::net::TcpListener::bind("0.0.0.0:80").await.unwrap();
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
        game_state_locked.you = PlayerType::X;
        drop(game_state_locked);

        let opponent = rx_matchmaking.await.unwrap();
        opponent.messenger
    };

    println!("matched players!!");
    // Client communication
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(Mutex::new(sender));
    
    // Send initial state to clients
    {
        let game_locked = game_state.lock().await;
        let game_init = InitGame { your_player: game_locked.you as i32 };
        let game_init_message = ServerMessage { message: Some(Com_Message::InitGame(game_init)) };
        let sender_copy = Arc::clone(&sender);
        let mut sender = sender_copy.lock().await;
        let game_init_message = <ServerMessage as prost::Message>::encode_to_vec(&game_init_message);
        sender.send(Message::Binary(Bytes::from(game_init_message))).await.unwrap();
        println!("sent init game {:?}", game_init);
    }

    // Game Starteds
    let borrowed_state = Arc::clone(&state);
    let game_state_for_this_player = Arc::clone(&game_state);
    let sender_for_this_player = Arc::clone(&sender);
    let recv_client_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Binary(bytes) => {
                    if let Ok(message) = <ServerMessage as prost::Message>::decode(&*bytes) {
                        let player_move = if let Some(Com_Message::PlayerMove(pm)) = message.message {
                            Some(pm)
                        } else {None};
                        if player_move == None {
                            continue;
                        }
                        let player_move = player_move.unwrap();
                        let mv = player_move.cell as usize;

                        let mut game_state = game_state_for_this_player.lock().await;
                        if !game_state.validate_move(mv, game_state.you) {
                            println!("received invalid move");
                            continue;
                        }

                        game_state.board[mv] = game_state.you as u8;
                        game_state.turn = game_state.turn.next();
                        println!("game board state: {:?}", game_state.board);
                        if let Some(won) = game_state.check_win() {
                            println!("my win assumed");
                            let mut sender = sender_for_this_player.lock().await;
                            let game_outcome = GameFinished { winner: true };
                            let final_message = ServerMessage { message: Some(Com_Message::GameFinished(game_outcome))};
                            let bytes = <ServerMessage as prost::Message>::encode_to_vec(&final_message);
                            sender.send(Message::Binary(Bytes::from(bytes))).await.unwrap();
                        } 
                        if let Err(err) = opponent_messenger.send(bytes) {
                            println!("wtf error is {err}");
                        }
                    } else {
                        println!("failed to decode protobuf message");
                    }
                },
                Message::Text(str) => {
                    println!("text is not supported anymore");
                },
                _ => {}
            }
        }
    });

    let (tx, mut rx) = channel(1);

    let game_state_ref = Arc::clone(&game_state);
    let recv_others_task = tokio::spawn(async move {
        while let Ok(msg) = my_messenger.recv().await {
            if let Ok(message) = <ServerMessage as prost::Message>::decode(&*msg) {
                let mut game_state = game_state_ref.lock().await;
                let player_move = if let Some(Com_Message::PlayerMove(pm)) = message.message {
                    Some(pm)
                } else {None};
                if player_move == None {
                    continue;
                }
                let player_move = player_move.unwrap();
                let mv = player_move.cell as usize;
                if !game_state.validate_move(mv, game_state.you.next()) {
                    println!("received invalid move");
                    continue;
                }

                game_state.board[mv] = game_state.you.next() as u8;
                game_state.turn = game_state.turn.next();

                let mut sender = sender.lock().await;
                if let Some(won) = game_state.check_win() {
                    println!("their win assumed");
                    let game_outcome = GameFinished { winner: false };
                    let final_message = ServerMessage { message: Some(Com_Message::GameFinished(game_outcome))};
                    let bytes = <ServerMessage as prost::Message>::encode_to_vec(&final_message);
                    sender.send(Message::Binary(Bytes::from(bytes))).await.unwrap();
                } else {
                    let player_move = PlayerMove { cell: mv as u32 };
                    let move_message = ServerMessage { message: Some(Com_Message::PlayerMove(player_move))};
                    let bytes = <ServerMessage as prost::Message>::encode_to_vec(&move_message);
                    sender.send(Message::Binary(Bytes::from(bytes))).await.unwrap();
                } 
            } else {

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