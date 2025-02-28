use bevy::{app::{App, Plugin, Update}, ecs::{event::{Event, EventReader, EventWriter}, schedule::{common_conditions::on_event, IntoSystemConfigs}, system::{Res, Resource}}};
use crossbeam::channel::{bounded, Receiver};
use js_sys::Function;
use messages::game::{server_message, PlayerMove, ServerMessage};
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
        
        let closure  = Closure::wrap(Box::new(move |x: js_sys::ArrayBuffer| {
            let uint8_array = js_sys::Uint8Array::new(&x);
            let x = uint8_array.to_vec();
            console_log!("socked plugin closure called {:?}", x);
            tx.send(InnerState { data: x }).ok();
        }) as Box<dyn FnMut(js_sys::ArrayBuffer)>);    
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
        let numbers: Vec<u8> = new_state.data;
        if let Ok(server_message) = ServerMessage::decode(&*numbers) {
            ev_message.send(SocketRecv(server_message));
        } else {
            println!("socket plugin error when decoding protobuf message");
        }
    }
}

fn send_system(mut ev_message: EventReader<SocketSend>) {
    console_log!("send system is called");
    for SocketSend(ev) in ev_message.read() {
        let bytes = match ev {
            ServerMessage{message: Some(server_message::Message::PlayerMove(x))} => {
                Some(ev.encode_to_vec())
            },
            _ => None // other types of messages are not supported
        };
        if let Some(bytes) = bytes {
            sendDataToSocket(bytes);
        }
    }
}

#[derive(Event)]
pub struct SocketRecv(pub ServerMessage);

#[derive(Event)]
pub struct SocketSend(pub ServerMessage);