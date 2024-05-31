use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
struct Datagram<'a> {
    dtype: MessageType,
    bytes: &'a [u8]
}

#[derive(Serialize, Deserialize, Debug)]
enum MessageType {
    Int64,
    Uint64,
    Float64,
    Boolean,
}


#[cfg(test)]
mod tests {
    use super::*;

    fn test_datagram() {
        let datagram = Datagram {
            dtype: MessageType::Boolean,
            bytes: &[0]
        };
    }
}