use anyhow::Result;
use log::error;
use midir::{Ignore, MidiInput, MidiInputConnection};
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver};

#[derive(Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Input {
    Pad1,
    Pad2,
    Pad3,
    Pad4,
    Pad5,
    Pad6,
    Pad7,
    Pad8,
    Fader1,
    Fader2,
    Fader3,
    Fader4,
    Fader5,
    Fader6,
    Fader7,
    Fader8,
}

pub enum Lpd8Message {
    ProgramChange(Input),
    ControlChange(Input, u8),
}

pub struct Lpd8 {
    pub messages: Receiver<Lpd8Message>,
    _connection: MidiInputConnection<()>,
}

impl Lpd8 {
    pub fn connect() -> Result<Lpd8> {
        let mut input = MidiInput::new("akai-lpd8-obs")?;
        input.ignore(Ignore::None);
        let mut lpd8_port = None;
        for p in input.ports() {
            let name = input.port_name(&p)?;
            if name.contains("LPD8") {
                lpd8_port = Some(p);
                break;
            }
        }

        if let Some(lpd8_port) = lpd8_port {
            let (sender, receiver) = mpsc::channel(100);

            let connection = input
                .connect(
                    &lpd8_port,
                    "lpd8",
                    move |_, msg, _| {
                        if let Some(msg) = process_input(msg)
                            && let Err(err) = sender.blocking_send(msg)
                        {
                            error!("Cannot send message to channel: {err}");
                        }
                    },
                    (),
                )
                .or(Err(LPD8Error::MidiError))?;

            return Ok(Lpd8 {
                messages: receiver,
                _connection: connection,
            });
        }

        Err(LPD8Error::NotFound.into())
    }
}

fn process_input(msg: &[u8]) -> Option<Lpd8Message> {
    if msg.is_empty() {
        return None;
    }

    let status = msg[0];
    if status & 0xC0 == 0xC0 {
        // this is a program change (aka pad pressed is pressed in PC mode)
        let pad = msg[1];
        program_change(pad)
    } else if status & 0xB0 == 0xB0 {
        // this is a control change (aka fader is rotated or pad in CC mode)
        let num = msg[1];
        let value = msg[2];
        control_change(num, value)
    } else {
        None
    }
}

fn get_input(num: u8) -> Option<Input> {
    if num == 0 || num == 12 {
        Some(Input::Pad1)
    } else if num == 1 || num == 13 {
        Some(Input::Pad2)
    } else if num == 2 || num == 14 {
        Some(Input::Pad3)
    } else if num == 3 || num == 15 {
        Some(Input::Pad4)
    } else if num == 4 || num == 16 {
        Some(Input::Pad5)
    } else if num == 5 || num == 17 {
        Some(Input::Pad6)
    } else if num == 6 || num == 18 {
        Some(Input::Pad7)
    } else if num == 7 || num == 19 {
        Some(Input::Pad8)
    } else if num == 70 {
        Some(Input::Fader1)
    } else if num == 71 {
        Some(Input::Fader2)
    } else if num == 72 {
        Some(Input::Fader3)
    } else if num == 73 {
        Some(Input::Fader4)
    } else if num == 74 {
        Some(Input::Fader5)
    } else if num == 75 {
        Some(Input::Fader6)
    } else if num == 76 {
        Some(Input::Fader7)
    } else if num == 77 {
        Some(Input::Fader8)
    } else {
        None
    }
}

fn program_change(num: u8) -> Option<Lpd8Message> {
    get_input(num).map(Lpd8Message::ProgramChange)
}

fn control_change(num: u8, value: u8) -> Option<Lpd8Message> {
    get_input(num).map(|i| Lpd8Message::ControlChange(i, value))
}

#[derive(Debug, Error)]
enum LPD8Error {
    #[error("No LPD8 Found")]
    NotFound,
    #[error("An error occured when connecting to LPD8")]
    MidiError,
}
