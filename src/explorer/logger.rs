use explorer::config::Config;
use explorer::monitor;
use serde::ser::{Serialize, SerializeSeq, Serializer};
use serde_cbor;
use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Serialize, Deserialize)]
pub enum LogMessage<E> {
    Event(E),
    NewBest {
        score: f64,
        cpt: usize,
        timestamp: Duration,
    },
    Finished(monitor::TerminationReason),
}

#[derive(Debug, Fail)]
pub enum LogError {
    #[fail(display = "{}", _0)]
    IOError(#[cause] ::std::io::Error),
    #[fail(display = "event serialization failed")]
    SerializationError(#[cause] serde_cbor::error::Error),
}

impl From<::std::io::Error> for LogError {
    fn from(error: ::std::io::Error) -> LogError {
        LogError::IOError(error)
    }
}

impl From<serde_cbor::error::Error> for LogError {
    fn from(error: serde_cbor::error::Error) -> LogError {
        LogError::SerializationError(error)
    }
}

pub fn log<E: Send + Serialize>(
    config: &Config,
    recv: mpsc::Receiver<LogMessage<E>>,
) -> Result<(), LogError> {
    let mut ser = init_eventlog(config)?;
    let mut seq = ser.serialize_seq(None)?;
    let mut write_buffer = init_log(config)?;
    loop {
        match recv.recv() {
            Ok(message) => match message {
                LogMessage::Event(event) => seq.serialize_element(&event)?,
                LogMessage::NewBest {
                    score,
                    cpt,
                    timestamp,
                } => {
                    log_monitor(score, cpt, timestamp, &mut write_buffer);
                }
                LogMessage::Finished(reason) => {
                    unwrap!(writeln!(write_buffer, "search stopped because {}", reason));
                }
            },
            Err(_) => {
                seq.end()?;
                write_buffer.flush()?;
                return Ok(());
            }
        }
    }
}

fn init_eventlog(config: &Config) -> io::Result<serde_cbor::Serializer<File>> {
    let f = File::create(&config.event_log)?;
    Ok(serde_cbor::Serializer::packed(f))
}

fn init_log(config: &Config) -> io::Result<BufWriter<File>> {
    let mut output_file = File::create(&config.log_file)?;
    write!(output_file, "LOGGER\n{}\n", config)?;
    Ok(BufWriter::new(output_file))
}

fn log_monitor(
    score: f64,
    cpt: usize,
    timestamp: Duration,
    write_buffer: &mut BufWriter<File>,
) {
    let t_s = timestamp.as_secs();
    let n_seconds = t_s % 60;
    let n_minutes = (t_s / 60) % 60;
    let n_hours = t_s / 3600;
    let message = format!(
        "New best candidate, score: {:.4e}ns, timestamp: {}h {}m {}s, \
         {} candidates evaluated\n",
        score, n_hours, n_minutes, n_seconds, cpt
    );
    unwrap!(write_buffer.write_all(message.as_bytes()));
}
