use rmp_serde;
use rmpv::Value;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    topic: String,
    value: Value,
}
impl Message {
    pub fn new(topic: &str, value: Value) -> Message {
        Message {
            topic: String::from(topic),
            value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {}
}
