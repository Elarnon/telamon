#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_cbor;
extern crate telamon;

use serde::de::{Deserializer, SeqAccess, Visitor};
use std::fmt;
use telamon::explorer::choice::ActionEx;
use std::time::Duration;
use telamon::ir::dim;
use telamon::ir::mem::InternalId;
use telamon::ir::*;
use telamon::search_space::*;

#[derive(Serialize, Deserialize)]
#[serde(remote = "Duration")]
struct CustomDuration {
    #[serde(getter = "Duration::as_secs")]
    secs: u64,
    #[serde(getter = "Duration::subsec_nanos")]
    nanos: u32, // Always 0 <= nanos < NANOS_PER_SEC
}

impl From<CustomDuration> for Duration {
    fn from(custom: CustomDuration) -> Duration {
        Duration::new(custom.secs, custom.nanos)
    }
}

#[derive(Deserialize)]
enum Message {
    Evaluation {
        actions: Vec<ActionEx>,
        score: f64,
        #[serde(with = "CustomDuration")]
        wall: Duration,
    },
}

struct EventPrinter;

impl<'de> Visitor<'de> for EventPrinter {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an event log")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        while let Some(event) = seq.next_element()? {
            match event {
                Message::Evaluation {
                    actions,
                    score,
                    wall,
                } => {
                    println!("{:?}", actions);
                }
                _ => {}
            }
        }

        Ok(())
    }
}

fn main() {
    let f = std::fs::File::open("actions.cbor").unwrap();
    let mut de = serde_cbor::Deserializer::from_reader(f);
    de.deserialize_seq(EventPrinter).unwrap();
}
