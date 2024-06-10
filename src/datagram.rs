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

    /// Get the topic
    pub fn topic(&self) -> &str {
        &self.topic
    }

    /// Get the value
    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serde() {}
}
