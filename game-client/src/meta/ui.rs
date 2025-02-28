use bevy::{app::{Plugin, Startup, Update}, color::Color, ecs::{component::Component, entity::Entity, event::{Event, EventReader}, query::With, schedule::{common_conditions::on_event, IntoSystemConfigs}, system::{Commands, Query, Single}}, hierarchy::{BuildChildren, ChildBuild, DespawnRecursiveExt}, text::{JustifyText, TextColor, TextLayout}, ui::{widget::Text, AlignItems, BackgroundColor, JustifyContent, Node, PositionType, UiRect, Val}, utils::default};

use crate::console_log;
use crate::log;

pub struct GameUI;

impl Plugin for GameUI {
    fn build(&self, app: &mut bevy::app::App) {
        app
            .add_event::<MetaEvent>()
            .add_systems(Startup, (draw_searching_modal))
            .add_systems(Update, (searching_processor, finish_processor).run_if(on_event::<MetaEvent>))
        ;
    }
}

fn searching_processor(
    mut commands: Commands,
    searching_modal: Query<Entity, With<SearchingOpponentModal>>,
    mut event_queue: EventReader<MetaEvent> 
) {
    let searching_modal = searching_modal.iter().nth(0);

    if let Some(searching_modal) = searching_modal {
        for event in event_queue.read() {
            match event {
                MetaEvent::OpponentFound => {
                    commands.entity(searching_modal).try_despawn_recursive();    
                },
                _ => {}
            }
        }
    }
}

fn finish_processor(
    commands: Commands,
    mut event_queue: EventReader<MetaEvent> 
) {
    for event in event_queue.read() {
        match event {
            MetaEvent::GameFinished(is_win) => {
                let txt = match is_win {
                    true => "won",
                    false => "lost"
                };
                console_log!("finish processor got event is_win: {is_win}");
                draw_final_modal(commands, String::from(txt));
                break;
            },
            _ => {}
        }
    }
}

#[derive(Component)]
struct SearchingOpponentModal;

fn draw_searching_modal(
    mut commands: Commands
) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .insert(SearchingOpponentModal)
    .with_children(|parent| {
            parent.spawn((Node {
                width: Val::Px(320.0),
                height: Val::Px(150.0),
                position_type: PositionType::Absolute,
                margin: UiRect {
                    top: Val::Px(50.0),
                    ..Default::default()
                },
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
                },
                BackgroundColor(Color::srgb(0.376, 0.376, 0.820))
        ))
        .with_children(|parent: &mut bevy::hierarchy::ChildBuilder<'_>| {
            parent.spawn(
               (Text::new("Searching opponent..."),
                TextColor(Color::srgb(0.941, 0.941, 0.286)),
                TextLayout {justify: JustifyText::Right, ..default()}
            ));
        });
    });
}

fn draw_final_modal(
    mut commands: Commands,
    txt: String
) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .insert(SearchingOpponentModal)
    .with_children(|parent| {
            parent.spawn((Node {
                width: Val::Px(320.0),
                height: Val::Px(150.0),
                position_type: PositionType::Absolute,
                margin: UiRect {
                    top: Val::Px(50.0),
                    ..Default::default()
                },
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..Default::default()
                },
                BackgroundColor(Color::srgb(0.92, 0.92, 0.247))
        ))
        .with_children(|parent: &mut bevy::hierarchy::ChildBuilder<'_>| {
            parent.spawn(
               (Text::new(format!("You {txt}!!!")),
                TextColor(Color::srgb(0.157, 0.094, 0.647)),
                TextLayout {justify: JustifyText::Right, ..default()}
            ));
        });
    });
}

#[derive(Event)]
pub enum MetaEvent {
    OpponentFound,
    GameFinished(bool) // whether win or not
}