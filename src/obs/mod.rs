use std::collections::HashMap;

use anyhow::Result;
use log::error;
use obws::{
    Client,
    events::Event,
    requests::{inputs::Volume, scene_items::SetEnabled},
    responses::{inputs::InputId, scenes::SceneId},
};
use tokio::{pin, select, spawn, sync::mpsc::Receiver, task::JoinHandle};
use tokio_stream::StreamExt;

use crate::{Mappings, midi::Lpd8Message};

pub struct Obs {
    _event_handler: JoinHandle<()>,
}

impl Obs {
    pub async fn connect(
        host: String,
        port: u16,
        password: Option<String>,
        mappings: Mappings,
        mut lpd8_messages: Receiver<Lpd8Message>,
    ) -> Result<Obs> {
        let client = Client::connect(host, port, password).await?;

        let scenes: HashMap<_, _> = client
            .scenes()
            .list()
            .await?
            .scenes
            .into_iter()
            .map(|s| (s.id.name.clone(), s.id.clone()))
            .collect();

        let inputs: HashMap<String, InputId> = client
            .inputs()
            .list(None)
            .await?
            .into_iter()
            .map(|i| (i.id.name.clone(), i.id.clone()))
            .collect();

        let initial_scene = client.scenes().current_program_scene().await?;

        let initial_inputs: HashMap<_, _> =
            gather_scene_inputs(&client, initial_scene.id.clone()).await?;

        let events = client.events()?;
        let event_handler = spawn(async move {
            pin!(events);
            let mut current_scene: CurrentScene = CurrentScene {
                id: initial_scene.id,
                inputs: initial_inputs,
            };

            loop {
                select! {
                    Some(msg) = lpd8_messages.recv() => {
                        match msg {
                            Lpd8Message::PCPad(num) => {
                                if let Some(scene) = mappings.pads.pc.get(&num.to_string())
                                    && let Some(scene_id) = scenes.get(scene)
                                    && let Err(e) =
                                        client.scenes().set_current_program_scene(scene_id).await
                                {
                                    error!("Unable to set current program scene to {scene}: {e}");
                                }
                            }
                            Lpd8Message::CCPad(num, value) => {
                                // pad is pressed
                                // pad is pressed, show the scene
                                if let Some(input_name) = mappings.pads.cc.get(&num.to_string())
                                    && let Some(input_id) = current_scene.inputs.get(input_name)
                                    && let Err(err) = client
                                        .scene_items()
                                        .set_enabled(SetEnabled {
                                            scene: current_scene.id.clone().into(),
                                            item_id: *input_id,
                                            // if value is > 0, pad is pressed and we show the input,
                                            // otherwise it is released and we hide it
                                            enabled: value > 0,
                                        })
                                        .await
                                {
                                    error!(
                                        "Unable to change current program scene to {}: {}",
                                        current_scene.id.name, err
                                    );
                                }
                            }
                            Lpd8Message::Fader(num, value) => {
                                if let Some(fader) = mappings.faders.get(&num.to_string())
                                    && let Some(input_id) = inputs.get(fader)
                                {
                                    let req_id: obws::requests::inputs::InputId = input_id.into();
                                    if let Err(err) = client
                                        .inputs()
                                        .set_volume(req_id, Volume::Mul(value as f32 / 127.))
                                        .await
                                    {
                                        error!(
                                            "Unable to change volume of input {}: {}",
                                            input_id.name, err
                                        );
                                    }
                                }
                            }
                        }
                    },
                    Some(event) = events.next() => {
                        if let Event::CurrentProgramSceneChanged { id } = event {
                            match gather_scene_inputs(&client, id.clone()).await {
                                Ok(scene_inputs) => {
                                    current_scene.id = id;
                                    current_scene.inputs = scene_inputs;
                                }
                                Err(err) => error!(
                                    "Error while gathering inputs for scene {}: {}",
                                    id.name, err
                                ),
                            }
                        }
                    },
                }
            }
        });
        Ok(Obs {
            _event_handler: event_handler,
        })
    }
}

async fn gather_scene_inputs(client: &Client, id: SceneId) -> Result<HashMap<String, i64>> {
    Ok(client
        .scene_items()
        .list(id.into())
        .await?
        .into_iter()
        .map(|i| (i.source_name, i.id))
        .collect())
}

#[derive(Debug)]
struct CurrentScene {
    id: SceneId,
    inputs: HashMap<String, i64>,
}
