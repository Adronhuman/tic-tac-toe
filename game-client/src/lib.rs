mod network;
mod javascript;

use bevy::{app::{App, Startup, Update}, ecs::{event::{EventReader, EventWriter}, schedule::{common_conditions::on_event, IntoSystemConfigs}, system::{Res, ResMut, Resource}}, input::{keyboard::KeyCode, ButtonInput}, DefaultPlugins};
use messages::game::{update::UpdateMessage, PlayerMove};
use network::socket_plugin::{SocketPlugin, SocketRecv, SocketSend};
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsCast};
use javascript::bindings::log;

#[wasm_bindgen]
pub fn start_bevy() {
    App::new()
        .add_plugins((DefaultPlugins, SocketPlugin))
        .add_systems(Update, hello_system)
        .add_systems(Update, (handle_update_from_network.run_if(on_event::<SocketRecv>)))
        .add_systems(Update, keyboard_input)
        .run();
}

fn hello_system() {
    console_log!("gamess startup");
}

fn handle_update_from_network(
    mut ev_message: EventReader<SocketRecv>
) {
    for SocketRecv(ev) in ev_message.read() {
        match ev {
            UpdateMessage::MoveUpdate(PlayerMove { r#move: cell }) => console_log!("move [{cell}] was made"),
            _ => console_log!("I don't know how to handle this")
        }
    }
}

fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut ev_message: EventWriter<SocketSend>
) {
    if keys.pressed(KeyCode::Space) {
        ev_message.send(SocketSend(UpdateMessage::MoveUpdate(PlayerMove { r#move: 69 })));
    }
}