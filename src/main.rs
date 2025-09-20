mod lpd8;
mod obs;

use log::{LevelFilter, info};
use log4rs::{
    Config,
    append::console::ConsoleAppender,
    config::{Appender, Root},
};
use std::{collections::HashMap, fmt::Display, io::stdin};

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;

use tokio::{fs::File, io::AsyncReadExt};

use crate::{
    lpd8::{Input, Lpd8},
    obs::Obs,
};

#[derive(Debug, Deserialize, Default)]
struct Mappings {
    #[serde(default)]
    program_changes: HashMap<Input, Action>,
    #[serde(default)]
    control_changes: Vec<HashMap<Input, ConditionalAction>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
enum Action {
    SetScene { name: String },
    SetVolume { name: String, value: Volume },
    ToggleInput { name: String },
    EnableSceneItem { name: String },
    DisableSceneItem { name: String },
    ToggleSceneItem { name: String },
}

impl Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::SetScene { name } => f.write_fmt(format_args!("set current scene to {name}")),
            Action::SetVolume { name, value } => {
                f.write_fmt(format_args!("set volume of {name} to {value}"))
            }
            Action::ToggleInput { name } => f.write_fmt(format_args!("toggle input {name}")),
            Action::EnableSceneItem { name } => {
                f.write_fmt(format_args!("enable scene item {name}"))
            }
            Action::DisableSceneItem { name } => {
                f.write_fmt(format_args!("disable scene item {name}"))
            }
            Action::ToggleSceneItem { name } => {
                f.write_fmt(format_args!("toggle scene item {name}"))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct ConditionalAction {
    on: Option<u8>,
    #[serde(flatten)]
    action: Action,
}

#[derive(Debug, Deserialize)]
enum Volume {
    #[serde(rename = "pass")]
    Pass,
    Value(u8),
}

impl Display for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Volume::Pass => f.write_str("input value"),
            Volume::Value(v) => f.write_fmt(format_args!("{v}")),
        }
    }
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
    #[arg(short = 'P', long, env)]
    pub password: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let stdout = ConsoleAppender::builder().build();
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))?;
    let _handle = log4rs::init_config(config)?;

    let args = Args::parse();

    let mut f = File::open(args.config_path).await?;
    let mut buffer = String::new();
    f.read_to_string(&mut buffer).await?;

    let mappings: Mappings = toml::from_str(buffer.as_str())?;

    let lpd8 = Lpd8::connect()?;
    let obs = Obs::connect(args.host, args.port, args.password).await?;
    let _handle = obs.start(mappings, lpd8.messages).await?;

    info!("OBS Controller is up and running, press [ENTER] to quit.");
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    info!("Bye bye");

    Ok(())
}
