use anyhow::Result;
use log::error;
use log_error::LogError;
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
    Knob1,
    Knob2,
    Knob3,
    Knob4,
    Knob5,
    Knob6,
    Knob7,
    Knob8,
}

impl TryFrom<u8> for Input {
    type Error = LPD8Error;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        if value == 0 || value == 12 {
            Ok(Input::Pad1)
        } else if value == 1 || value == 13 {
            Ok(Input::Pad2)
        } else if value == 2 || value == 14 {
            Ok(Input::Pad3)
        } else if value == 3 || value == 15 {
            Ok(Input::Pad4)
        } else if value == 4 || value == 16 {
            Ok(Input::Pad5)
        } else if value == 5 || value == 17 {
            Ok(Input::Pad6)
        } else if value == 6 || value == 18 {
            Ok(Input::Pad7)
        } else if value == 7 || value == 19 {
            Ok(Input::Pad8)
        } else if value == 70 {
            Ok(Input::Knob1)
        } else if value == 71 {
            Ok(Input::Knob2)
        } else if value == 72 {
            Ok(Input::Knob3)
        } else if value == 73 {
            Ok(Input::Knob4)
        } else if value == 74 {
            Ok(Input::Knob5)
        } else if value == 75 {
            Ok(Input::Knob6)
        } else if value == 76 {
            Ok(Input::Knob7)
        } else if value == 77 {
            Ok(Input::Knob8)
        } else {
            Err(LPD8Error::UnknownInput(value))
        }
    }
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
                        if let Some(msg) = process_input(msg) {
                            sender
                                .blocking_send(msg)
                                .log_error("Cannot send message to channel");
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
    if status & 0xC0 == 0xC0 && msg.len() == 2 {
        // this is a program change (aka pad pressed is pressed in PC mode)
        let pad = msg[1];
        program_change(pad)
    } else if status & 0xB0 == 0xB0 && msg.len() == 3 {
        // this is a control change (aka knob is rotated or pad in CC mode)
        let num = msg[1];
        let value = msg[2];
        control_change(num, value)
    } else {
        None
    }
}

fn program_change(num: u8) -> Option<Lpd8Message> {
    num.try_into()
        .log_error("Unable to detect program change input")
        .map(Lpd8Message::ProgramChange)
}

fn control_change(num: u8, value: u8) -> Option<Lpd8Message> {
    num.try_into()
        .log_error("Unable to detect control change input")
        .map(|i| Lpd8Message::ControlChange(i, value))
}

#[derive(Debug, Error)]
pub enum LPD8Error {
    #[error("No LPD8 Found")]
    NotFound,
    #[error("An error occured when connecting to LPD8")]
    MidiError,
    #[error("Unknown LPD8 input with id {0}")]
    UnknownInput(u8),
}
