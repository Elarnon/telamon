use explorer::choice::ActionEx;
use explorer::config::Config;
use rpds::List;
use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, SerializeSeq, Serializer};
use serde_cbor;
use std::fs::File;
use std::sync::mpsc;

#[derive(Serialize, Deserialize)]
pub enum Event {
    Evaluation {
        actions: Sequence<ActionEx>,
        score: f64,
    },
}

#[derive(Debug, Fail)]
pub enum LoggingError {
    #[fail(display = "{}", _0)]
    IOError(#[cause] ::std::io::Error),
    #[fail(display = "{}", _0)]
    SerializationError(#[cause] serde_cbor::error::Error),
}

impl From<::std::io::Error> for LoggingError {
    fn from(error: ::std::io::Error) -> LoggingError {
        LoggingError::IOError(error)
    }
}

impl From<serde_cbor::error::Error> for LoggingError {
    fn from(error: serde_cbor::error::Error) -> LoggingError {
        LoggingError::SerializationError(error)
    }
}

pub fn log(config: &Config, recv: mpsc::Receiver<Event>) -> Result<(), LoggingError> {
    // We manually create a sequence serializer in order to perform
    // streaming serialization.
    let file = File::create(&config.event_log)?;
    let mut ser = serde_cbor::Serializer::packed(file);
    let mut seq = ser.serialize_seq(None)?;
    loop {
        match recv.recv() {
            Ok(event) => seq.serialize_element(&event)?,
            Err(_) => {
                seq.end()?;
                return Ok(());
            }
        }
    }
}
