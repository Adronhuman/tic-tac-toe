mod network;
mod javascript;

use bevy::{app::{App, Startup, Update}, ecs::{event::{Event, EventReader, EventWriter}, schedule::{common_conditions::on_event, IntoSystemConfigs}, system::{Res, ResMut, Resource}}, input::{keyboard::KeyCode, ButtonInput}, DefaultPlugins};
use messages::game::{server_message::Message, PlayerMove, PlayerType, ServerMessage};
use network::socket_plugin::{SocketPlugin, SocketRecv, SocketSend};
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsCast};
use javascript::bindings::log;

#[derive(Resource, Debug)]
struct GameState {
    board: [u8; 9],
    game_finished: Option<bool>,
    is_your_turn: bool
}

#[derive(Event)]
struct PlayersMove {
    cell: usize
}

#[wasm_bindgen]
pub fn start_bevy() {
    let game_state = GameState {
        board: [228;9],
        game_finished: None,
        is_your_turn: false
    };

    App::new()
        .insert_resource(game_state)
        .add_plugins((DefaultPlugins, SocketPlugin))
        .add_systems(Update, (handle_update_from_network.run_if(on_event::<SocketRecv>)))
        .add_systems(Update, process_players_move.run_if(on_event::<PlayersMove>))
        .add_systems(Update, keyboard_input)
        .add_event::<PlayersMove>()
        .run();
}

fn handle_update_from_network(
    mut game_state: ResMut<GameState>,
    mut ev_message: EventReader<SocketRecv>
) {
    for SocketRecv(ev) in ev_message.read() {
        console_log!("receive network update event");
        if let Some(message) = ev.message {
            match message {
                Message::InitGame(g) => {                    
                    if let Ok(PlayerType::X) = PlayerType::try_from(g.your_player) {
                        game_state.is_your_turn = true;
                    }
                    console_log!("got init game: {:?}; {:?}", game_state, g.your_player);
                }
                Message::GameFinished(f) => {
                    game_state.game_finished = Some(f.winner);
                }
                Message::PlayerMove(mv) => {
                    let cell = mv.cell as usize;
                    game_state.board[cell] = 96;
                    game_state.is_your_turn = true;
                }
            }
        }
    }
}

fn process_players_move(
    mut game_state: ResMut<GameState>,
    mut ev_move: EventReader<PlayersMove>,
    mut ev_message: EventWriter<SocketSend>
) {
    if !game_state.is_your_turn {
        return;
    }

    for player_move in ev_move.read() {
        if game_state.board[player_move.cell] != 228 {
            return;
        }

        game_state.board[player_move.cell] = 69;
        console_log!("game board state is now {:?}", game_state);
        ev_message.send(SocketSend(ServerMessage{message: Some(Message::PlayerMove(PlayerMove {cell: player_move.cell as u32}))}));
        game_state.is_your_turn = false;
    }

}

fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut ev_message: EventWriter<PlayersMove>
) {
    if keys.pressed(KeyCode::Digit0) {
        ev_message.send(PlayersMove { cell: 0 });
    }
    if keys.pressed(KeyCode::Digit1) {
        ev_message.send(PlayersMove { cell: 1 });
    }
    if keys.pressed(KeyCode::Digit2) {
        ev_message.send(PlayersMove { cell: 2 });
    }
}