extern crate bincode;
extern crate telamon;
extern crate telamon_utils as utils;

use telamon::explorer::TreeEvent;
use utils::tfrecord::{ReadError, RecordReader};

fn main() -> Result<(), ReadError> {
    let mut f = std::fs::File::open("eventlog.tfrecord")?;
    while let Some(record) = f.try_read_record()? {
        match bincode::deserialize(&record).unwrap() {
            TreeEvent::Evaluation {
                actions,
                score: _score,
            } => println!("{:?}", actions.to_vec()),
        }
    }
    Ok(())
}
