use bevy::{app::{App, Plugin, Update}, ecs::{event::{Event, EventReader, EventWriter}, schedule::{common_conditions::on_event, IntoSystemConfigs}, system::{Res, Resource}}};
use crossbeam::channel::{bounded, Receiver};
use js_sys::Function;
use messages::game::{update::UpdateMessage, Chat, PlayerMove};
use prost::Message;
use wasm_bindgen::{prelude::Closure, JsCast};
use crate::javascript::bindings::log;

use crate::{console_log, javascript::bindings::{listenToSocketData, sendDataToSocket}};

#[derive(Resource)]
struct UpdateReceiver {
    reciever: Receiver<InnerState>
}

struct InnerState {
    data: Vec<u8>,
}

pub struct SocketPlugin;

impl Plugin for SocketPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = bounded::<InnerState>(1);

        let state = UpdateReceiver {
            reciever: rx
        };
        
        let closure: Closure<dyn FnMut(Vec<u8>)> = Closure::wrap(Box::new(move |x: Vec<u8>| {
            tx.send(InnerState { data: x }).ok();
        }) as Box<dyn FnMut(Vec<u8>)>);    
        let function: Function = closure.into_js_value().dyn_into().unwrap();
    
        listenToSocketData(&function);
    
        app
            .insert_resource(state)
            .add_systems(Update, (receive_system, send_system.run_if(on_event::<SocketSend>)))
            .add_event::<SocketRecv>()
            .add_event::<SocketSend>();
    }
}

fn receive_system(
    state: Res<UpdateReceiver>,
    mut ev_message: EventWriter<SocketRecv> 
) {
    if let Ok(new_state) = state.reciever.try_recv() {
        let numbers = new_state.data;
        ev_message.send(SocketRecv(UpdateMessage::MoveUpdate(PlayerMove { r#move: numbers[0] as i32})));
    }
}

fn send_system(mut ev_message: EventReader<SocketSend>) {
    console_log!("send system is called");
    for SocketSend(ev) in ev_message.read() {
        let bytes = match ev {
            UpdateMessage::ChatUpdate(chat) => chat.encode_to_vec(),
            UpdateMessage::MetaUpdate(meta) => meta.encode_to_vec(),
            UpdateMessage::MoveUpdate(mv) => mv.encode_to_vec()
        };
        sendDataToSocket(bytes);
    }
}

#[derive(Event)]
pub struct SocketRecv(pub UpdateMessage);

#[derive(Event)]
pub struct SocketSend(pub UpdateMessage);