extern crate serde;
extern crate serde_cbor;
extern crate telamon;

use serde::de::{Deserializer, SeqAccess, Visitor};
use std::fmt;
use telamon::explorer::choice::ActionEx;
use telamon::ir::dim;
use telamon::ir::mem::InternalId;
use telamon::ir::*;
use telamon::search_space::*;

struct ActionsPrinter {}

impl<'de> Visitor<'de> for ActionsPrinter {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "multiple action lists")
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        while let Some(actions) = seq.next_element()? as Option<Vec<ActionEx>> {
            println!("{:?}", actions);
        }

        Ok(())
    }
}

fn main() {
    let f = std::fs::File::open("actions.cbor").unwrap();
    let mut de = serde_cbor::Deserializer::from_reader(f);
    de.deserialize_seq(ActionsPrinter {}).unwrap();
}
