mod network;
mod javascript;

use bevy::{app::{App, Startup, Update}, asset::AssetServer, core_pipeline::core_2d::Camera2d, ecs::{event::{Event, EventReader, EventWriter}, schedule::{common_conditions::on_event, IntoSystemConfigs}, system::{Commands, Res, ResMut, Resource}}, input::{keyboard::KeyCode, ButtonInput}, math::Vec3, sprite::Sprite, transform::components::Transform, utils::default, DefaultPlugins};
use messages::game::{server_message::Message, PlayerMove, PlayerType, ServerMessage};
use network::socket_plugin::{SocketPlugin, SocketRecv, SocketSend};
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsCast};
use javascript::bindings::log;

#[derive(Resource, Debug)]
struct GameState {
    board: [u8; 9],
    game_finished: Option<bool>,
    is_your_turn: bool,
    me: PlayerType
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
        is_your_turn: false,
        me: PlayerType::O
    };

    App::new()
        .insert_resource(game_state)
        .add_plugins((DefaultPlugins, SocketPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, draw.run_if(on_event::<DrawRequest>))
        .add_systems(Update, (handle_update_from_network.run_if(on_event::<SocketRecv>)))
        .add_systems(Update, process_players_move.run_if(on_event::<PlayersMove>))
        .add_systems(Update, keyboard_input)
        .add_event::<PlayersMove>()
        .add_event::<DrawRequest>()
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2d);

    commands.spawn((
        Sprite::from_image(asset_server.load("desk.png")),
        Transform {
            translation: Vec3::new(-100., 100., 0.),
            ..default()
        }
    ));
}

#[derive(Event, Debug)]
struct DrawRequest {
    who: PlayerType,
    where_: usize
}

fn draw(
    mut commands: Commands,
    mut queue: EventReader<DrawRequest>,
    asset_server: Res<AssetServer>,
) {
    for dr in queue.read() {
        console_log!("draw called {:?}", dr);
        let cell_index = dr.where_ as i32;
        let player: &PlayerType = &dr.who;

        let local_origin = (-200, 200);
        let cell_coordinates = (local_origin.0 + 100 * (cell_index % 3), local_origin.1 - 100*(cell_index / 3));

        let image_name = match player {
            PlayerType::X => "tic.png",
            PlayerType::O => "tac.png"
        };

        commands.spawn((
            Sprite::from_image(asset_server.load(image_name)),
            Transform {
                translation: Vec3::new(cell_coordinates.0 as f32, cell_coordinates.1 as f32, 1.),
                ..default()
            }
        ));
    }
}

fn handle_update_from_network(
    mut game_state: ResMut<GameState>,
    mut ev_message: EventReader<SocketRecv>,
    mut draw_queue: EventWriter<DrawRequest>
) {
    for SocketRecv(ev) in ev_message.read() {
        console_log!("receive network update event");
        if let Some(message) = ev.message {
            match message {
                Message::InitGame(g) => {                    
                    if let Ok(PlayerType::X) = PlayerType::try_from(g.your_player) {
                        game_state.is_your_turn = true;
                        game_state.me = PlayerType::X;
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
                    draw_queue.send(DrawRequest { who: game_state.me.opposite(), where_: cell });
                }
            }
        }
    }
}

fn process_players_move(
    mut game_state: ResMut<GameState>,
    mut ev_move: EventReader<PlayersMove>,
    mut ev_message: EventWriter<SocketSend>,
    mut draw_queue: EventWriter<DrawRequest>
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
        draw_queue.send(DrawRequest { who: game_state.me, where_: player_move.cell });
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


trait OppositeExt {
    fn opposite(self) -> Self;
}

impl OppositeExt for PlayerType {
    fn opposite(self) -> Self {
        match self {
            PlayerType::X => PlayerType::O,
            PlayerType::O => PlayerType::X
        }
    }
} 