use std::{collections::HashMap, io::stdin};
use tokio::pin;
use tokio_stream::StreamExt;

use anyhow::Result;
use clap::Parser;
use obws::{
    Client,
    events::Event,
    requests::{inputs::Volume, scene_items::SetEnabled},
    responses::{inputs::InputId, scenes::SceneId},
};
use serde::Deserialize;
use thiserror::Error;

use midir::{Ignore, MidiInput};
use tokio::{fs::File, io::AsyncReadExt, spawn, sync::mpsc};

#[derive(Debug, Deserialize, Default)]
struct Mappings {
    pads: PadMappings,
    #[serde(default)]
    faders: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Default)]
struct PadMappings {
    #[serde(default)]
    pc: HashMap<String, String>,
    #[serde(default)]
    cc: HashMap<String, String>,
}

#[derive(Debug)]
struct CurrentScene {
    id: SceneId,
    inputs: HashMap<String, i64>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "lpd8-mappings.toml")]
    pub config_path: String,
    #[arg(short = 'H', long, default_value = "localhost")]
    pub host: String,
    #[arg(short, long, default_value_t = 4455)]
    pub port: u16,
    #[arg(short = 'P', long = "password", env)]
    pub obs_password: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut f = File::open(args.config_path).await?;
    let mut buffer = String::new();
    f.read_to_string(&mut buffer).await?;

    let mappings: Mappings = toml::from_str(buffer.as_str())?;

    let mut input = MidiInput::new("akai-lpd8-obs")?;
    input.ignore(Ignore::None);

    if let Some(port) = input.ports().into_iter().find(|p| {
        input
            .port_name(p)
            .map(|n| n.contains("LPD8"))
            .unwrap_or(false)
    }) {
        let client = Client::connect(args.host, args.port, args.obs_password).await?;
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

        let (sender, mut receiver) = mpsc::channel(100);

        let midi_sender = sender.clone();
        let _conn = input
            .connect(
                &port,
                "lpd8",
                move |_, msg, _| {
                    let status = msg[0];
                    if status & 0xC0 == 0xC0 {
                        // this is a program change (aka pad pressed is pressed in PC mode)
                        let pad = msg[1];
                        midi_sender.blocking_send(Message::PCPad(pad)).unwrap();
                    } else if status & 0xB0 == 0xB0 {
                        // this is a control change (aka fader is rotated or pad in CC mode)
                        let control = msg[1];
                        let value = msg[2];
                        if (12..=19).contains(&control) {
                            // this is a pad in control change mode
                            midi_sender
                                .blocking_send(Message::CCPad(control, value))
                                .unwrap();
                        } else {
                            // this is a fader
                            midi_sender
                                .blocking_send(Message::Fader(control, value))
                                .unwrap();
                        }
                    }
                },
                (),
            )
            .unwrap();

        let initial_scene = client.scenes().current_program_scene().await?;

        let initial_inputs: HashMap<_, _> =
            gather_scene_inputs(&client, initial_scene.id.clone()).await?;

        println!("Initial scene inputs: {initial_inputs:?}");

        let events = client.events()?;
        let _event_handler = spawn(async move {
            pin!(events);
            println!("Event Stream handler started");
            while let Some(event) = events.next().await {
                if let Event::CurrentProgramSceneChanged { id } = event {
                    println!("New scene: {id:?}");
                    if let Err(e) = sender.send(Message::NewScene(id)).await {
                        println!("Could not enqueue message: {e}");
                    }
                }
            }
            println!("Event Stream handler ends");
        });

        let _handler = spawn(async move {
            let mut current_scene: CurrentScene = CurrentScene {
                id: initial_scene.id,
                inputs: initial_inputs,
            };

            while let Some(msg) = receiver.recv().await {
                match msg {
                    Message::PCPad(num) => {
                        if let Some(scene) = mappings.pads.pc.get(&num.to_string())
                            && let Some(scene_id) = scenes.get(scene)
                            && let Err(e) =
                                client.scenes().set_current_program_scene(scene_id).await
                        {
                            println!("Could not change current program scene: {e}");
                        }
                    }
                    Message::CCPad(num, value) => {
                        // pad is pressed
                        if value > 0 {
                            // pad is pressed, show the scene
                            if let Some(input_name) = mappings.pads.cc.get(&num.to_string())
                                && let Some(input_id) = current_scene.inputs.get(input_name)
                            {
                                client
                                    .scene_items()
                                    .set_enabled(SetEnabled {
                                        scene: current_scene.id.clone().into(),
                                        item_id: *input_id,
                                        enabled: true,
                                    })
                                    .await
                                    .unwrap();
                            }
                        } else {
                            // pad is released, hide the scene
                            if let Some(input_name) = mappings.pads.cc.get(&num.to_string())
                                && let Some(input_id) = current_scene.inputs.get(input_name)
                            {
                                client
                                    .scene_items()
                                    .set_enabled(SetEnabled {
                                        scene: current_scene.id.clone().into(),
                                        item_id: *input_id,
                                        enabled: false,
                                    })
                                    .await
                                    .unwrap();
                            }
                        }
                    }
                    Message::Fader(num, value) => {
                        if let Some(fader) = mappings.faders.get(&num.to_string())
                            && let Some(input_id) = inputs.get(fader)
                        {
                            let req_id: obws::requests::inputs::InputId = input_id.into();
                            client
                                .inputs()
                                .set_volume(req_id, Volume::Mul(value as f32 / 127.))
                                .await
                                .unwrap();
                        }
                    }
                    Message::NewScene(id) => {
                        let scene_inputs = gather_scene_inputs(&client, id.clone()).await.unwrap();
                        current_scene.id = id;
                        current_scene.inputs = scene_inputs;
                    }
                }
            }
        });

        println!("OBS Controller is up and running, press [ENTER] to quit.");
        let mut input = String::new();
        stdin().read_line(&mut input)?;
        println!("Bye bye");
    } else {
        Err(MidiError::PortNotFound)?
    }

    Ok(())
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
enum Message {
    PCPad(u8),
    CCPad(u8, u8),
    Fader(u8, u8),
    NewScene(SceneId),
}

#[derive(Error, Debug)]
enum MidiError {
    #[error("No LPD8 found")]
    PortNotFound,
}
