//! websocket communication

use crate::gamedata::GameState;
use crate::RootRenderingComponent;
use futures::Future;
use js_sys::Reflect;
use mem3_common::WsMessage;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{console, WebSocket};

///setup websocket connection
pub fn setup_ws_connection(location_href: &str) -> WebSocket {
    //web-sys has websocket for Rust exactly like javascript has¸
    console::log_1(&"location_href".into());
    console::log_1(&wasm_bindgen::JsValue::from_str(location_href));
    //location_href comes in this format  http://localhost:4000/
    let mut loc_href = location_href
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    //Only for debugging in the development environment
    //let mut loc_href = String::from("ws://192.168.1.57:80/");
    loc_href.push_str("mem3ws/");
    console::log_1(&wasm_bindgen::JsValue::from_str(&loc_href));
    //same server address and port as http server
    let ws = WebSocket::new(&loc_href).expect("WebSocket failed to connect.");

    //I don't know why is clone needed
    let ws_c = ws.clone();
    //It looks that the first send is in some way a handshake and is part of the connection
    //it will be execute onopen as a closure
    let open_handler = Box::new(move || {
        console::log_1(&"Connection opened, sending 'test' to server".into());
        ws_c.send_with_str(
            &serde_json::to_string(&WsMessage::RequestWsUid {
                test: String::from("test"),
            })
            .expect("error sending test"),
        )
        .expect("Failed to send 'test' to server");
    });

    let cb_oh: Closure<dyn Fn()> = Closure::wrap(open_handler);
    ws.set_onopen(Some(cb_oh.as_ref().unchecked_ref()));
    //don't drop the open_handler memory
    cb_oh.forget();
    ws
}

/// receive websocket msg callback. I don't understand this much. Too much future and promises.
pub fn setup_ws_msg_recv(ws: &WebSocket, vdom: &dodrio::Vdom) {
    //Player1 on machine1 have a button Ask player to play! before he starts to play.
    //Click and it sends the WsMessage want_to_play. Player1 waits for the reply and cannot play.
    //Player2 on machine2 see the WsMessage and Accepts it.
    //It sends a WsMessage with the vector of cards. Both will need the same vector.
    //The vector of cards is copied.
    //Player1 click a card. It opens locally and sends WsMessage with index of the card.
    //Machine2 receives the WsMessage and runs the same code as the player would click. The RootRenderingComponent is blocked.
    //The method with_component() needs a future (promise) It will be executed on the next vdom tick.
    //This is the only way I found to write to RootRenderingComponent fields.
    let weak = vdom.weak();
    let msg_recv_handler = Box::new(move |msg: JsValue| {
        let data: JsValue =
            Reflect::get(&msg, &"data".into()).expect("No 'data' field in websocket message!");

        //serde_json can find out the variant of WsMessage
        //parse json and put data in the enum
        let msg: WsMessage =
            serde_json::from_str(&data.as_string().expect("Field 'data' is not string"))
                .unwrap_or_else(|_x| WsMessage::Dummy {
                    dummy: String::from("error"),
                });

        //match enum by variant and prepares the future that will be executed on the next tick
        //in this big enum I put only boilerplate code that don't change any data.
        //for changing data I put code in separate functions for easy reading.
        match msg {
            //I don't know why I need a dummy, but is entertaining to have one.
            WsMessage::Dummy { dummy } => console::log_1(&dummy.into()),
            //this RequestWsUid is only for the WebSocket server
            WsMessage::RequestWsUid { test } => console::log_1(&test.into()),
            WsMessage::ResponseWsUid { your_ws_uid } => {
                wasm_bindgen_futures::spawn_local(
                    weak.with_component({
                        move |root| {
                            console::log_1(&"ResponseWsUid".into());
                            let root_rendering_component =
                                root.unwrap_mut::<RootRenderingComponent>();
                            root_rendering_component.on_response_ws_uid(your_ws_uid);
                        }
                    })
                    .map_err(|_| ()),
                );
            }

            WsMessage::WantToPlay {
                my_ws_uid,
                content_folder_name,
            } => {
                wasm_bindgen_futures::spawn_local(
                    weak.with_component({
                        let v2 = weak.clone();
                        move |root| {
                            let root_rendering_component =
                                root.unwrap_mut::<RootRenderingComponent>();

                            if let GameState::EndGame | GameState::Start | GameState::Asked =
                                root_rendering_component.game_data.game_state
                            {
                                root_rendering_component
                                    .on_want_to_play(my_ws_uid, content_folder_name);
                                v2.schedule_render();
                            }
                        }
                    })
                    .map_err(|_| ()),
                );
            }
            WsMessage::AcceptPlay {
                my_ws_uid,
                card_grid_data,
                ..
            } => {
                wasm_bindgen_futures::spawn_local(
                    weak.with_component({
                        let v2 = weak.clone();
                        move |root| {
                            console::log_1(&"rcv AcceptPlay".into());
                            let root_rendering_component =
                                root.unwrap_mut::<RootRenderingComponent>();
                            root_rendering_component.on_accept_play(my_ws_uid, &card_grid_data);
                            v2.schedule_render();
                        }
                    })
                    .map_err(|_| ()),
                );
            }
            WsMessage::PlayerClick {
                my_ws_uid,
                card_index,
                count_click_inside_one_turn,
                ..
            } => {
                wasm_bindgen_futures::spawn_local(
                    weak.with_component({
                        let v2 = weak.clone();
                        console::log_1(&"player_click".into());
                        move |root| {
                            let root_rendering_component =
                                root.unwrap_mut::<RootRenderingComponent>();
                            console::log_1(&"other_ws_uid".into());
                            if my_ws_uid == root_rendering_component.game_data.other_ws_uid {
                                root_rendering_component
                                    .on_player_click(count_click_inside_one_turn, card_index);
                                v2.schedule_render();
                            }
                        }
                    })
                    .map_err(|_| ()),
                );
            }
            WsMessage::PlayerChange { my_ws_uid, .. } => {
                wasm_bindgen_futures::spawn_local(
                    weak.with_component({
                        let v2 = weak.clone();
                        move |root| {
                            let root_rendering_component =
                                root.unwrap_mut::<RootRenderingComponent>();
                            console::log_1(&"PlayerChange".into());
                            if my_ws_uid == root_rendering_component.game_data.other_ws_uid {
                                root_rendering_component.on_player_change();
                                v2.schedule_render();
                            }
                        }
                    })
                    .map_err(|_| ()),
                );
            }
            //this message is for the WebSocket server
            WsMessage::RequestSpelling { filename } => console::log_1(&filename.into()),
            WsMessage::ResponseSpellingJson { json } => {
                wasm_bindgen_futures::spawn_local(
                    weak.with_component({
                        move |root| {
                            console::log_1(&"ResponseSpellingJson".into());
                            let root_rendering_component =
                                root.unwrap_mut::<RootRenderingComponent>();
                            root_rendering_component.on_response_spelling_json(&json)
                        }
                    })
                    .map_err(|_| ()),
                );
            }
            WsMessage::EndGame { .. } => {
                wasm_bindgen_futures::spawn_local(
                    weak.with_component({
                        move |root| {
                            console::log_1(&"EndGame".into());
                            let root_rendering_component =
                                root.unwrap_mut::<RootRenderingComponent>();
                            root_rendering_component.on_end_game();
                        }
                    })
                    .map_err(|_| ()),
                );
            }
        }
    });

    //magic ??
    let cb_mrh: Closure<dyn Fn(JsValue)> = Closure::wrap(msg_recv_handler);
    ws.set_onmessage(Some(cb_mrh.as_ref().unchecked_ref()));

    //don't drop the eventlistener from memory
    cb_mrh.forget();
}
