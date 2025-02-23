use bevy::{app::{App, Plugin, Update}, ecs::{event::{Event, EventWriter}, system::{Res, Resource}}};
use crossbeam::channel::{bounded, Receiver};
use js_sys::Function;
use messages::game::{update::UpdateMessage, PlayerMove};
use wasm_bindgen::{prelude::Closure, JsCast};

use crate::javascript::bindings::listenToSocketData;

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
            .add_systems(Update, update_system)
            .add_event::<SocketMessage>();
    }
}

fn update_system(
    state: Res<UpdateReceiver>,
    mut ev_message: EventWriter<SocketMessage> 
) {
    if let Ok(new_state) = state.reciever.try_recv() {
        let numbers = new_state.data;
        ev_message.send(SocketMessage(UpdateMessage::MoveUpdate(PlayerMove { r#move: numbers[0] as i32})));
    }
}

#[derive(Event)]
pub struct SocketMessage(pub UpdateMessage);