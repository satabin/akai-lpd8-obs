mod midi;
mod obs;

use log::{LevelFilter, info};
use log4rs::{
    Config,
    append::console::ConsoleAppender,
    config::{Appender, Root},
};
use std::{collections::HashMap, io::stdin};

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;

use tokio::{fs::File, io::AsyncReadExt};

use crate::{midi::Lpd8, obs::Obs};

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
    let _obs = Obs::connect(
        args.host,
        args.port,
        args.password,
        mappings,
        lpd8.messages,
    )
    .await?;

    info!("OBS Controller is up and running, press [ENTER] to quit.");
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    info!("Bye bye");

    Ok(())
}
