mod network;
mod javascript;

use bevy::{app::{App, Startup, Update}, ecs::{event::EventReader, schedule::{common_conditions::on_event, IntoSystemConfigs}, system::{Res, ResMut, Resource}}, DefaultPlugins};
use messages::game::{update::UpdateMessage, PlayerMove};
use network::socket_plugin::{SocketMessage, SocketPlugin};
use wasm_bindgen::{prelude::{wasm_bindgen, Closure}, JsCast};
use javascript::bindings::log;

#[wasm_bindgen]
pub fn start_bevy() {
    App::new()
        .add_plugins((DefaultPlugins, SocketPlugin))
        .add_systems(Startup, hello_system)
        .add_systems(Update, handle_update_from_network.run_if(on_event::<SocketMessage>))
        .run();
}

fn hello_system() {
    console_log!("game startup");
}

fn handle_update_from_network(
    mut ev_message: EventReader<SocketMessage>
) {
    for SocketMessage(ev) in ev_message.read() {
        match ev {
            UpdateMessage::MoveUpdate(PlayerMove { r#move: cell }) => console_log!("move [{cell}] was made"),
            _ => console_log!("I don't know how to handle this")
        }
    }
}