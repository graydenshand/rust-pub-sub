// Size of all channel buffers
pub const CHANNEL_BUFFER_SIZE: usize = 1_000_000;

// Character used as wildcard in subscription patterns
pub const WILDCARD: char = '*';

// System published topics will use this prefix
pub const SYSTEM_TOPIC_PREFIX: &'static str = "!system";

// Metric reporting intervals
pub const M_COMMANDS_INTERVAL_S: u64 = 10;
pub const M_CONNECTIONS_INTERVAL_S: u64 = 10;
pub const M_MESSAGES_SENT_INTERVAL_S: u64 = 10;
