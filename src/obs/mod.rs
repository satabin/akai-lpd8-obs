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

use crate::{
    Action, ConditionalAction, Mappings,
    lpd8::{Input, Lpd8Message},
};

pub struct Obs {
    client: Client,
    scenes: HashMap<String, SceneId>,
    inputs: HashMap<String, InputId>,
}

impl Obs {
    pub async fn connect(host: String, port: u16, password: Option<String>) -> Result<Obs> {
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

        Ok(Obs {
            client,
            scenes,
            inputs,
        })
    }

    pub async fn start(
        self,
        mappings: Mappings,
        mut lpd8_messages: Receiver<Lpd8Message>,
    ) -> Result<JoinHandle<()>> {
        let initial_scene = self.client.scenes().current_program_scene().await?;

        let initial_inputs: HashMap<_, _> =
            gather_scene_inputs(&self.client, initial_scene.id.clone()).await?;

        let pc_mappings = mappings.program_changes;
        let cc_mappings = build_cc_mappings(mappings.control_changes);

        let events = self.client.events()?;
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
                            Lpd8Message::ProgramChange(input) => {
                                if let Some(action) = pc_mappings.get(&input)
                                    && let Err(e) =
                                        self.execute_action(action, 0, &current_scene).await
                                {
                                    error!("Unable to execute action {action}: {e}");
                                }
                            }
                            Lpd8Message::ControlChange(input, value) => {
                                if let Some(action_with_default) = cc_mappings.get(&input)
                                    && let Some(action) = action_with_default.get(value)
                                    && let Err(e) = self.execute_action(action, value, &current_scene).await
                                {
                                    error!("Unable to execute action {action}: {e}");
                                }
                        }
                    }
                    },
                    Some(event) = events.next() => {
                        if let Event::CurrentProgramSceneChanged { id } = event {
                            match gather_scene_inputs(&self.client, id.clone()).await {
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

        Ok(event_handler)
    }

    async fn execute_action(
        &self,
        action: &Action,
        data: u8,
        current_scene: &CurrentScene,
    ) -> Result<()> {
        match action {
            Action::SetScene { name } => {
                if let Some(scene_id) = self.scenes.get(name) {
                    self.client
                        .scenes()
                        .set_current_program_scene(scene_id)
                        .await?
                }
            }
            Action::SetVolume { name, value } => {
                if let Some(input_id) = self.inputs.get(name) {
                    self.client
                        .inputs()
                        .set_volume(
                            input_id.into(),
                            Volume::Mul(match value {
                                crate::Volume::Pass => data as f32 / 127.,
                                crate::Volume::Value(v) => *v as f32 / 100.,
                            }),
                        )
                        .await?
                }
            }
            Action::ToggleInput { name } => {
                if let Some(input_id) = self.inputs.get(name) {
                    self.client.inputs().toggle_mute(input_id.into()).await?;
                }
            }
            Action::EnableSceneItem { name } => {
                if let Some(input_id) = current_scene.inputs.get(name) {
                    self.client
                        .scene_items()
                        .set_enabled(SetEnabled {
                            scene: current_scene.id.clone().into(),
                            item_id: *input_id,
                            enabled: true,
                        })
                        .await?
                }
            }
            Action::DisableSceneItem { name } => {
                if let Some(input_id) = current_scene.inputs.get(name) {
                    self.client
                        .scene_items()
                        .set_enabled(SetEnabled {
                            scene: current_scene.id.clone().into(),
                            item_id: *input_id,
                            enabled: false,
                        })
                        .await?
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MappingWithDefault {
    by_value: HashMap<u8, Action>,
    default: Option<Action>,
}

impl MappingWithDefault {
    fn get(&self, value: u8) -> Option<&Action> {
        self.by_value.get(&value).or(self.default.as_ref())
    }
}

fn build_cc_mappings(
    cc_mappings: Vec<HashMap<Input, ConditionalAction>>,
) -> HashMap<Input, MappingWithDefault> {
    let mut grouped: HashMap<Input, MappingWithDefault> = HashMap::new();
    for kv in cc_mappings {
        for (input, action) in kv {
            let mapping = grouped.entry(input).or_default();
            match action.on {
                Some(value) => {
                    mapping.by_value.insert(value, action.action);
                }
                None => {
                    mapping.default = Some(action.action);
                }
            }
        }
    }

    grouped
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
