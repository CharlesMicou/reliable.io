#![cfg_attr(feature="cargo-clippy", warn(clippy, clippy_correctness, clippy_style, clippy_pedantic, clippy_perf))]
#![feature(nll, stmt_expr_attributes)]
#![warn(rust_2018_idioms)]

// TODO: remove when done
#![allow(dead_code, unused_imports)]

#[macro_use] extern crate log;

pub mod capi;
pub mod binding_version;

/* TODO:
enum Counters {

}

#define RELIABLE_ENDPOINT_COUNTER_NUM_PACKETS_SENT                          0
#define RELIABLE_ENDPOINT_COUNTER_NUM_PACKETS_RECEIVED                      1
#define RELIABLE_ENDPOINT_COUNTER_NUM_PACKETS_ACKED                         2
#define RELIABLE_ENDPOINT_COUNTER_NUM_PACKETS_STALE                         3
#define RELIABLE_ENDPOINT_COUNTER_NUM_PACKETS_INVALID                       4
#define RELIABLE_ENDPOINT_COUNTER_NUM_PACKETS_TOO_LARGE_TO_SEND             5
#define RELIABLE_ENDPOINT_COUNTER_NUM_PACKETS_TOO_LARGE_TO_RECEIVE          6
#define RELIABLE_ENDPOINT_COUNTER_NUM_FRAGMENTS_SENT                        7
#define RELIABLE_ENDPOINT_COUNTER_NUM_FRAGMENTS_RECEIVED                    8
#define RELIABLE_ENDPOINT_COUNTER_NUM_FRAGMENTS_INVALID                     9
#define RELIABLE_ENDPOINT_NUM_COUNTERS                                      10

*/

const RELIABLE_MAX_PACKET_HEADER_BYTES: usize = 9;
const RELIABLE_FRAGMENT_HEADER_BYTES: usize = 5;

#[derive(Debug)]
pub enum ReliableError {
    Io(std::io::Error),
    ExceededMaxPacketSize
}

impl std::fmt::Display for ReliableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid first item to double")
    }
}

// This is important for other errors to wrap this one.
impl std::error::Error for ReliableError {
    fn description(&self) -> &str {
        "invalid first item to double"
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        None
    }
}


#[derive(Clone)]
pub struct Config {
    name: String,
    index: i32,
    max_packet_size: usize,
    fragment_above: u32,
    max_fragments: u32,
    fragment_size: usize,
    ack_buffer_size: usize,
    sent_packets_buffer_size: usize,
    received_packets_buffer_size: usize,
    fragment_reassembly_buffer_size: usize,
    rtt_smoothing_factor: f32,
    packet_loss_smoothing_factor: f32,
    bandwidth_smoothing_factor: f32,
    packet_header_size: u32,
}

impl Config {
    fn new(name: &str, ) -> Self {
        let mut r = Self::default();
        r.name = name.to_string();
        r
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "".to_string(),
            index: 1,
            max_packet_size: 16 * 1024,
            fragment_above: 1024,
            max_fragments: 16,
            fragment_size: 1024,
            ack_buffer_size: 256,
            sent_packets_buffer_size: 256,
            received_packets_buffer_size: 256,
            fragment_reassembly_buffer_size: 64,
            rtt_smoothing_factor: 0.0025,
            packet_loss_smoothing_factor: 0.1,
            bandwidth_smoothing_factor: 0.1,
            packet_header_size: 28,
        }
    }
}

/*
struct reliable_fragment_reassembly_data_t
{
    uint16_t sequence;
    uint16_t ack;
    uint32_t ack_bits;
    int num_fragments_received;
    int num_fragments_total;
    uint8_t * packet_data;
    int packet_bytes;
    int packet_header_bytes;
    uint8_t fragment_received[256];
};

*/

#[derive(Debug, Clone)]
struct SentData {
    time: f64,
    acked: bool,
    size: usize,
}

impl SentData {
    pub fn new(time: f64, size: usize) -> Self {
        Self {
            time,
            size,
            acked: false,
        }
    }
}

impl Default for SentData {
    fn default() -> Self {
        Self {
            time: 0.0,
            size: 0,
            acked: false,
        }
    }
}

#[derive(Debug, Clone)]
struct RecvData {
    time: f64,
    size: usize,
}
impl RecvData {
    pub fn new(time: f64, size: usize) -> Self {
        Self {
            time,
            size,
        }
    }
}

impl Default for RecvData {
    fn default() -> Self {
        Self {
            time: 0.0,
            size: 0,
        }
    }
}

#[derive(Debug, Clone)]
struct ReassemblyData {}

impl Default for ReassemblyData {
    fn default() -> Self {
        Self {}
    }
}

struct SequenceBuffer<T> where T: Default + std::clone::Clone + Send + Sync {
    entries: Vec<T>,
    entry_sequences: Vec<u16>,
    sequence: u16,
}

impl<T> SequenceBuffer<T> where T: Default + std::clone::Clone + Send + Sync {
    pub fn with_capacity(size: usize) -> Self {
        let mut entries = Vec::with_capacity(size);
        let mut entry_sequences = Vec::with_capacity(size);

        entries.resize(size, T::default());
        entry_sequences.resize(size, 0xFFFF);

        Self {
            sequence: 0,
            entries,
            entry_sequences,
        }
    }

    pub fn insert(&mut self, data: T, sequence: u16) -> Result<u16, ReliableError> {

        if Self::sequence_less_than(sequence, self.sequence - self.len() {

        }

        let index = (sequence % self.entries.len() as u16) as usize;

        self.entries[index] = data;
        self.entry_sequences[index] = sequence;

        self.sequence = sequence + 1;

        Ok(sequence)
    }


    pub fn clear(&mut self) {

    }

    pub fn sequence(&self) -> u16 {
        self.sequence
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn capacity(&self) -> usize {
        self.entries.capacity()
    }

    fn sequence_greater_than(s1: u16, s2: u16) -> bool {
        ( ( s1 > s2 ) && ( s1 - s2 <= 32768 ) ) || ( ( s1 < s2 ) && ( s2 - s1  > 32768 ) )
    }

    fn sequence_less_than(s1: u16, s2: u16) -> bool {
        Self::sequence_greater_than(s2, s1)
    }
}

pub struct Endpoint {
    time: f64,
    config: Config,
    acks: Vec<u16>,
    sequence: i32,
    num_acks: usize,
    sent_buffer: SequenceBuffer<SentData>,
    recv_buffer: SequenceBuffer<RecvData>,
    reassembly_buffer: SequenceBuffer<ReassemblyData>,

}

impl Endpoint {
    pub fn new(config: Config, time: f64, ) -> Self {
        trace!("Creating new endpoint named '{}'", config.name);
        let mut r = Self {
            time,
            config: config.clone(),
            acks: Vec::new(),
            num_acks: 0,
            sequence: 0,
            sent_buffer: SequenceBuffer::with_capacity(config.sent_packets_buffer_size),
            recv_buffer: SequenceBuffer::with_capacity(config.received_packets_buffer_size),
            reassembly_buffer: SequenceBuffer::with_capacity(config.fragment_reassembly_buffer_size),
        };

        r.acks.resize(config.ack_buffer_size, 0);
        r
    }

    pub fn reset(&mut self) {
        self.num_acks = 0;
        self.sequence = 0;

        self.sent_buffer.clear();
        self.recv_buffer.clear();
        self.reassembly_buffer.clear();
    }

    pub fn next_sequence(&self) -> i32 {
        self.sequence
    }

    pub fn send(&mut self, index: i32, packet: &[u8]) -> Result<usize, ReliableError> {

        if packet.len() > self.config.max_packet_size {
            error!("Packet too large: Attempting to send {}, max={}", packet.len(), self.config.max_packet_size);
            return Err(ReliableError::ExceededMaxPacketSize);
        }

        // Increment sequence
        self.sequence = self.sequence + 1;

        Ok(packet.len())
    }

    fn generate_ack_bits(&self, ack: &mut u16, ackbits: &mut u32) {

    }
}

#[cfg(test)]
mod tests {
    fn enable_logging() {
        use env_logger::Builder;
        use log::LevelFilter;

        Builder::new().filter(None, LevelFilter::Trace).init();
    }


    use super::*;

    #[test]
    fn sequence_test() {
        #[derive(Debug, Clone, Default)]
        struct TestData {
            sequence: u16,
        }
        let mut buffer = SequenceBuffer::<TestData>::with_capacity(255);

        assert_eq!(buffer.capacity(), 255);
        assert_eq!(buffer.sequence(), 0);

        for i in 0..255 {
            buffer.insert(TestData{ sequence: i }, i);
            assert_eq!(buffer.sequence(), i + 1);
        }

        for i in 0..255 {

        }


    }

    #[test]
    fn rust_impl_endpoint() {
        enable_logging();

        let endpoint = Endpoint::new(Config::new("balls"), 0.0);

    }
}