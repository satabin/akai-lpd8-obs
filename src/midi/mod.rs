use anyhow::Result;
use log::error;
use midir::{Ignore, MidiInput, MidiInputConnection};
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver};

pub enum Lpd8Message {
    PCPad(u8),
    CCPad(u8, u8),
    Fader(u8, u8),
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
        Some(Lpd8Message::PCPad(pad))
    } else if status & 0xB0 == 0xB0 {
        // this is a control change (aka fader is rotated or pad in CC mode)
        let control = msg[1];
        let value = msg[2];
        if (12..=19).contains(&control) {
            // this is a pad in control change mode
            Some(Lpd8Message::CCPad(control, value))
        } else {
            // this is a fader
            Some(Lpd8Message::Fader(control, value))
        }
    } else {
        None
    }
}

#[derive(Debug, Error)]
enum LPD8Error {
    #[error("No LPD8 Found")]
    NotFound,
    #[error("An error occured when connecting to LPD8")]
    MidiError,
}
