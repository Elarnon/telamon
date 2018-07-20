extern crate rpds;

use self::rpds::List;
use serde::de::{Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};

/// A type representing a sequence of values.
///
/// Can be implemented as either a persistent list or as a native
/// vector. This is handy for serialization: we usually work with
/// persistent lists but those share part of their structure and make
/// them bad candidates for "direct" serialization. Instead, we
/// serialize the persistent list as a sequence, and always
/// deserialize it as a vector. By duplicating the objects when
/// serializing we sidestep the problem of having pointers in the
/// serialized stream.
pub enum Sequence<T> {
    List(List<T>),
    Vec(Vec<T>),
}

impl<T: Serialize> Serialize for Sequence<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Sequence::List(list) => serializer.collect_seq(list.iter()),
            Sequence::Vec(vec) => vec.serialize(serializer),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Sequence<T> {
    fn deserialize<D>(deserializer: D) -> Result<Sequence<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Sequence::Vec(<Vec<T> as Deserialize<'de>>::deserialize(
            deserializer,
        )?))
    }
}
