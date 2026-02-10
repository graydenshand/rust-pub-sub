//! Configuration for write buffering and performance tuning

/// Strategy for flushing the write buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushStrategy {
    /// Flush immediately after each write (lowest latency, most syscalls)
    Immediate,
    /// Flush periodically based on time interval (balanced)
    Periodic { interval_ms: u64 },
    /// Let buffer auto-flush when full (highest throughput, highest latency)
    Auto,
}

/// Performance mode presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceMode {
    LowLatency,
    Balanced,
    HighThroughput,
}

impl PerformanceMode {
    pub fn to_config(self) -> BufferConfig {
        match self {
            PerformanceMode::LowLatency => BufferConfig {
                size: 4 * 1024,         // 4 KB
                flush_strategy: FlushStrategy::Immediate,
            },
            PerformanceMode::Balanced => BufferConfig {
                size: 32 * 1024,        // 32 KB
                flush_strategy: FlushStrategy::Periodic { interval_ms: 10 },
            },
            PerformanceMode::HighThroughput => BufferConfig {
                size: 128 * 1024,       // 128 KB
                flush_strategy: FlushStrategy::Auto,
            },
        }
    }
}

/// Configuration for write buffering
#[derive(Debug, Clone, Copy)]
pub struct BufferConfig {
    /// Size of the write buffer in bytes
    pub size: usize,
    /// Strategy for flushing the buffer
    pub flush_strategy: FlushStrategy,
}

impl BufferConfig {
    /// Low latency configuration (4KB buffer, immediate flush)
    pub fn low_latency() -> Self {
        PerformanceMode::LowLatency.to_config()
    }

    /// Balanced configuration (32KB buffer, 10ms periodic flush)
    pub fn balanced() -> Self {
        PerformanceMode::Balanced.to_config()
    }

    /// High throughput configuration (128KB buffer, auto flush)
    pub fn high_throughput() -> Self {
        PerformanceMode::HighThroughput.to_config()
    }

    /// Custom configuration
    pub fn custom(size: usize, flush_strategy: FlushStrategy) -> Self {
        BufferConfig {
            size,
            flush_strategy,
        }
    }
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self::balanced()
    }
}
