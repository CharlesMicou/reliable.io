#![cfg_attr(feature="cargo-clippy", warn(clippy, clippy_correctness, clippy_style, clippy_pedantic, clippy_perf))]
#![cfg_attr(feature="cargo-clippy", allow(similar_names))]
#![feature(nll, stmt_expr_attributes)]
#![warn(rust_2018_idioms)]

// TODO: remove when done
#![allow(dead_code, unused_imports)]

#[macro_use] extern crate log;

use std::num::Wrapping;

pub mod capi;
pub mod binding_version;

mod sequence_buffer;
pub use crate::sequence_buffer::SequenceBuffer as SequenceBuffer;

mod error;
pub use crate::error::ReliableError as ReliableError;

mod headers;
pub use crate::headers::PacketHeader as PacketHeader;
pub use crate::headers::FragmentHeader as FragmentHeader;
pub use crate::headers::HeaderParser as Header;

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

pub const RELIABLE_MAX_PACKET_HEADER_BYTES: usize = 9;
pub const RELIABLE_FRAGMENT_HEADER_BYTES: usize = 5;


#[derive(Clone)]
pub struct EndpointConfig {
    pub name: String,
    pub index: i32,
    pub max_packet_size: usize,
    pub fragment_above: usize,
    pub max_fragments: u32,
    pub fragment_size: usize,
    pub ack_buffer_size: usize,
    pub sent_packets_buffer_size: usize,
    pub received_packets_buffer_size: usize,
    pub fragment_reassembly_buffer_size: usize,
    pub rtt_smoothing_factor: f32,
    pub packet_loss_smoothing_factor: f32,
    pub bandwidth_smoothing_factor: f32,
    pub packet_header_size: usize,
}

impl EndpointConfig {
    pub fn new(name: &str, ) -> Self {
        let mut r = Self::default();
        r.name = name.to_string();
        r
    }
}

impl Default for EndpointConfig {
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

#[derive(Clone)]
struct ReassemblyData {
    sequence: u16,
    ack: u16,
    ack_bits: u32,
    num_fragments_received: usize,
    num_fragments_total: usize,
    buffer: Vec<u8>,
    fragments_received: [bool; 256],
    header_size: usize,
}

impl ReassemblyData {
    pub fn new(sequence: u16, ack: u16, ack_bits: u32, num_fragments_total: usize, header_size: usize, prealloc: usize,) -> Self {
        Self {
            sequence,
            ack,
            ack_bits,
            num_fragments_received: 0,
            num_fragments_total,
            buffer: Vec::with_capacity(prealloc),
            fragments_received: [false; 256],
            header_size,
        }
    }
}
impl Default for ReassemblyData {
    fn default() -> Self {
        Self {
            sequence: 0,
            ack: 0,
            ack_bits: 0,
            num_fragments_received: 0,
            num_fragments_total: 0,
            buffer: Vec::with_capacity(1024),
            fragments_received: [false; 256],
            header_size: 0,
        }
    }
}


#[derive(Debug, Clone)]
struct SentData {
    time:  f64,
    acked: bool,
    size:  usize,
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

pub struct Endpoint {
    time: f64,
    rtt: f32,
    config: EndpointConfig,
    acks: Vec<u16>,
    sequence: i32,
    sent_buffer: SequenceBuffer<SentData>,
    recv_buffer: SequenceBuffer<RecvData>,
    reassembly_buffer: SequenceBuffer<ReassemblyData>,
    temp_packet_buffer: Vec<u8>,
    send_function: &'static dyn Fn(i32, u16, &[u8]),
    recv_function: &'static dyn Fn(i32, u16, &[u8]) -> bool,

}

impl Endpoint {
    #[cfg_attr(feature="cargo-clippy", allow(needless_pass_by_value))]
    pub fn new(config: EndpointConfig, time: f64, send_function: &'static dyn Fn(i32, u16, &[u8]), recv_function: &'static dyn Fn(i32, u16, &[u8]) -> bool) -> Self {
        trace!("Creating new endpoint named '{}'", config.name);
        Self {
            time,
            rtt: 0.0,
            config: config.clone(),
            acks: Vec::with_capacity(config.ack_buffer_size),
            sequence: 0,
            sent_buffer: SequenceBuffer::with_capacity(config.sent_packets_buffer_size),
            recv_buffer: SequenceBuffer::with_capacity(config.received_packets_buffer_size),
            reassembly_buffer: SequenceBuffer::with_capacity(config.fragment_reassembly_buffer_size),
            temp_packet_buffer: Vec::with_capacity(config.max_packet_size),
            send_function,
            recv_function,
        }
    }

    #[cfg_attr(feature="cargo-clippy", allow(cast_possible_truncation, cast_sign_loss))]
    pub fn send(&mut self, packet: &[u8]) -> Result<usize, ReliableError> {
        if packet.len() > self.config.max_packet_size {
            error!("Packet too large: Attempting to send {}, max={}", packet.len(), self.config.max_packet_size);
            return Err(ReliableError::ExceededMaxPacketSize);
        }

        // Increment sequence
        let sequence = self.sequence;
        self.sequence += 1;

        let (ack, ack_bits) = self.recv_buffer.ack_bits();

        let send_size = packet.len() + self.config.packet_header_size;
        let sent = SentData::new(self.time, send_size);
        self.sent_buffer.insert(sent, sequence as u16)?;

        let header = PacketHeader::new(sequence as u16, ack, ack_bits);

        if packet.len() <= self.config.fragment_above {
            // no fragments
            // TODO: reimplement this as a cursor
            trace!("Sending packet {} without fragmentation", sequence);

            self.temp_packet_buffer.resize(header.size(), 0);
            let mut cursor = std::io::Cursor::new(self.temp_packet_buffer.as_mut_slice());
            header.write(&mut cursor)?;
            self.temp_packet_buffer.extend_from_slice(packet);

            (self.send_function)(self.config.index, sequence as u16, &self.temp_packet_buffer);
        } else {
            let remainder = if packet.len() % self.config.fragment_size  > 0 { 1 } else { 0 };
            let num_fragments = (packet.len() / self.config.fragment_size ) + remainder;

            trace!("Sending packet {} with fragmentation, size={}, fragments={}", sequence, packet.len(), num_fragments);

            for fragment_id in 0..num_fragments {
                let fragment = FragmentHeader::new(fragment_id as u8, num_fragments as u8, header.clone());
                self.temp_packet_buffer.resize(fragment.size(), 0);

                let mut cursor = std::io::Cursor::new(self.temp_packet_buffer.as_mut_slice());
                fragment.write(&mut cursor)?;

                let cur_start = fragment_id * self.config.fragment_size;
                let mut cur_end = (fragment_id + 1) * self.config.fragment_size;
                if cur_end > packet.len() {
                    cur_end = packet.len();
                }

                self.temp_packet_buffer.extend_from_slice(&packet[cur_start..cur_end]);

                (self.send_function)(self.config.index, sequence as u16, &self.temp_packet_buffer);
                self.temp_packet_buffer.clear();
            }
        }



        Ok(packet.len())
    }

    #[cfg_attr(feature="cargo-clippy", allow(cast_possible_truncation, cast_sign_loss, if_not_else))]
    pub fn recv(&mut self, packet: &[u8]) -> Result<(), ReliableError> {
        if packet.len() > self.config.max_packet_size {
            error!("Packet too large: Attempting to recv {}, max={}", packet.len(), self.config.max_packet_size);
            return Err(ReliableError::ExceededMaxPacketSize);
        }

        let mut ret = Ok(());
        let mut packet_reader = std::io::Cursor::new(packet);
        let prefix_byte = packet[0];

        if prefix_byte & 1 == 0 {
            match PacketHeader::parse(&mut packet_reader) {
                Ok(header) => {
                    if !self.recv_buffer.check_sequence(header.sequence()) {
                        error!("Ignoring stale packet: {}", header.sequence());
                        return Err(ReliableError::StalePacket);
                    }

                    trace!("Processing packet...");
                    if (self.recv_function)(self.config.index, header.sequence(), &packet[packet_reader.position() as usize..packet.len()]) {
                        trace!("process packet successful");

                        self.recv_buffer.insert(RecvData::new(self.time, self.config.packet_header_size + packet.len()), header.sequence())?;

                        let mut ack_bits = header.ack_bits();
                        for i in 0..32 {
                            if ack_bits & 1 != 0 {
                                let ack_sequence: u16 = (Wrapping(header.ack()) - Wrapping(i)).0;

                                if let Some(sent_data) = self.sent_buffer.get_mut(ack_sequence) {
                                    if !sent_data.acked && self.acks.len() < self.config.ack_buffer_size {
                                        trace!("mark acked packet: {}", ack_sequence);
                                        self.acks.push(ack_sequence);

                                        sent_data.acked = true;
                                        let rtt: f32 = (self.time as f32 - sent_data.time as f32) * 1000.0;
                                        if (self.rtt == 0.0 && rtt > 0.0) || (self.rtt - rtt).abs() < 0.00001 {
                                            self.rtt = rtt;
                                        } else {
                                            self.rtt = self.rtt + ((rtt - self.rtt) * self.config.rtt_smoothing_factor);
                                        }
                                    }
                                }
                            }
                            ack_bits >>= 1;
                        }
                    } else {
                        error!("Process received packet failed");
                    }


                    return Ok(());
                },
                Err(e) => { return Err(e); },
            }
        } else {
            match FragmentHeader::parse(&mut packet_reader) {
                Ok(header) => {
                    trace!("parsed fragment header correctly, processing reassembly..: id={}, s={}", header.sequence(), header.id());

                    {
                        let reassembly_data = match self.reassembly_buffer.get_mut(header.sequence()) {
                            Some(reassembly_data) => {
                                reassembly_data
                            },
                            None => {
                                if header.id() == 0 {
                                    if header.packet_header().is_none() {
                                        return Err(ReliableError::InvalidFragment);
                                    }

                                    let ack = header.packet_header().unwrap().ack();
                                    let ack_bits = header.packet_header().unwrap().ack_bits();
                                    let reassembly_data = ReassemblyData::new(header.sequence(),
                                                                              ack,
                                                                              ack_bits,
                                                                              usize::from(header.count()),
                                                                              header.size(),
                                                                              RELIABLE_MAX_PACKET_HEADER_BYTES + self.config.fragment_size);

                                    self.reassembly_buffer.insert(reassembly_data.clone(), header.sequence())?
                                } else {
                                    panic!("Error!");
                                }
                            },
                        };

                        // Got the data
                        if reassembly_data.num_fragments_total != usize::from(header.count()) {
                            return Err(ReliableError::InvalidFragment);
                        }

                        if reassembly_data.fragments_received[usize::from(header.id())] {
                            return Err(ReliableError::InvalidFragment);
                        }

                        reassembly_data.num_fragments_received += 1;
                        reassembly_data.fragments_received[usize::from(header.id())] = true;

                        trace!("{}: recieved fragment #{}/{}, wtf={}", self.config.name, header.id()+1, header.count(), reassembly_data.num_fragments_received );

                        let start_position: usize = if header.id() == 0 {
                            5
                        } else {
                            header.size()
                        };

                        reassembly_data.buffer.extend_from_slice(&packet[start_position..packet.len()]);

                        if reassembly_data.num_fragments_received == reassembly_data.num_fragments_total {
                            let sequence = reassembly_data.sequence as u16;
                            let buffer = reassembly_data.buffer.clone(); // TODO: WHY DO I HAVE TO DO THIS CLONE!?!?!

                            ret = self.recv(buffer.as_slice());

                            self.reassembly_buffer.remove(sequence);
                        }
                    }
                },
                Err(e) => { return Err(e); },
            }
        }

        ret
    }

    pub fn update(&mut self, time: f64) {
        self.time = time;


    }

    pub fn reset(&mut self) {
        self.sequence = 0;

        self.acks.clear();
        self.sent_buffer.reset();
        self.recv_buffer.reset();
        self.reassembly_buffer.reset();
    }

    pub fn next_sequence(&self) -> i32 {
        self.sequence
    }
    pub fn acks(&self ) -> &[u16] { self.acks.as_slice() }

}

#[cfg(test)]
mod tests {
    const TEST_BUFFER_SIZE: usize = 256;
    
    use super::*;

    use std::sync::{Once, ONCE_INIT};

    static LOGGER_INIT: Once = ONCE_INIT;

    fn enable_logging() {
        LOGGER_INIT.call_once(||{
            use env_logger::Builder;
            use log::LevelFilter;

            Builder::new().filter(None, LevelFilter::Trace).init();
        });

    }

    fn test_compare<T>(one: &[T], two: &[T]) -> bool
        where T: PartialEq
    {
        if one.len() != two.len() {
            return false;
        }
        for i in 0..one.len() {
            if one[i] != two[i] {
                return false;
            }
        }
        true
    }


    const TEST_FRAGMENTS_NUM_ITERATIONS: usize = 200;
    #[test]
    fn fragments() {
        enable_logging();
        use std::sync::mpsc::{Sender, Receiver};
        let (one_send, one_recv): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = std::sync::mpsc::channel();
        let (two_send, two_recv): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = std::sync::mpsc::channel();

        let def = EndpointConfig::default();

        let mut time = 100.0;
        let test_data_remainder = [0x41; 4092];
        let test_data_align = [0x41; 2048];

        let mut test_data = &test_data_align;

        let mut one = Endpoint::new(EndpointConfig::new("one"), time,
                                    |_, sequence, buffer| {
                                        two_send.send(buffer.to_vec());
                                    },
                                    |_, _, data| {
                                        assert!(test_compare(data, test_data));

                                        true
                                    });

        let mut two = Endpoint::new(EndpointConfig::new("two"), time,
                                    |_, sequence, buffer| {
                                        one_send.send(buffer.to_vec());
                                    },
                                    |_, _, data| {
                                        assert!(test_compare(data, test_data));

                                        true
                                    });

        let delta_time = 0.01;
        for i in 0..TEST_FRAGMENTS_NUM_ITERATIONS {
            // forward packets to their endpoints
            match one_recv.try_recv() {
                Ok(v) => { one.recv(v.as_slice()); },
                Err(_) => {}
            }
            match two_recv.try_recv() {
                Ok(v) => { two.recv(v.as_slice()); },
                Err(_) => {}
            }

            // Send test packets
            one.send(test_data);
            two.send(test_data);

            time += delta_time;
            one.update(time);
            two.update(time);
        }

        let mut test_data = &test_data_remainder;

        let delta_time = 0.01;
        for i in 0..TEST_FRAGMENTS_NUM_ITERATIONS {
            // forward packets to their endpoints
            match one_recv.try_recv() {
                Ok(v) => { one.recv(v.as_slice()).unwrap(); },
                Err(_) => {}
            }
            match two_recv.try_recv() {
                Ok(v) => { two.recv(v.as_slice()).unwrap(); },
                Err(_) => {}
            }

            // Send test packets
            one.send(test_data).unwrap();
            two.send(test_data).unwrap();

            time += delta_time;
            one.update(time);
            two.update(time);
        }
    }

    const TEST_ACKS_NUM_ITERATIONS: usize = 200;
    #[test]
    fn acks() {
        enable_logging();
        use std::sync::mpsc::{Sender, Receiver};
        let (one_send, one_recv): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = std::sync::mpsc::channel();
        let (two_send, two_recv): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = std::sync::mpsc::channel();

        let mut time = 100.0;
        let test_data = [0x41; 24];

        let mut one = Endpoint::new(EndpointConfig::new("one"), time,
        |_, sequence, buffer| {
            trace!("ONE: Sending packet: len={}", buffer.len());
            two_send.send(buffer.to_vec()).unwrap();
        },
        |_, _, data| {

            assert_eq!(&data, &test_data);

            true
        });

        let mut two = Endpoint::new(EndpointConfig::new("two"), time,
        |_, sequence, buffer| {
            trace!("TWO: Sending packet: len={}", buffer.len());
            one_send.send(buffer.to_vec()).unwrap();
        },
        |_, _, data| {

            assert_eq!(&data, &test_data);

            true
        });

        let delta_time = 0.01;
        for i in 0..TEST_ACKS_NUM_ITERATIONS {
            // forward packets to their endpoints
            match one_recv.try_recv() {
                Ok(v) => { one.recv(v.as_slice()).unwrap(); },
                Err(_) => {}
            }
            match two_recv.try_recv() {
                Ok(v) => { two.recv(v.as_slice()).unwrap(); },
                Err(_) => {}
            }

            // Send test packets
            one.send(&test_data).unwrap();
            two.send(&test_data).unwrap();

            time += delta_time;
            one.update(time);
            two.update(time);
        }

        /* TODO: I dont understand what he was checking here?
        let mut one_acked: [u8; TEST_ACKS_NUM_ITERATIONS] = [0; TEST_ACKS_NUM_ITERATIONS];
        let mut i = 0;
        for ack in one.acks() {
            if *ack < TEST_ACKS_NUM_ITERATIONS as u16 {
                one_acked[*ack as usize] = 1;
                trace!("Acked: {}", i);
            }
            i += 1;
        }
        for i in 0..TEST_ACKS_NUM_ITERATIONS / 2 {
            assert_eq!(one_acked[i], ((i+1) % 2) as u8);
        }*/

    }

    #[test]
    fn ack_bits() {
        enable_logging();

        #[derive(Debug, Clone, Default)]
        struct TestData {
            sequence: u16,
        }

        let mut buffer = SequenceBuffer::<TestData>::with_capacity(TEST_BUFFER_SIZE);

        for i in 0..TEST_BUFFER_SIZE+1 {
            buffer.insert(TestData{ sequence: i as u16 }, i as u16).unwrap();
        }

        let (ack, ack_bits) = buffer.ack_bits();

        assert_eq!(ack, TEST_BUFFER_SIZE as u16);
        assert_eq!(ack_bits, 0xFFFFFFFF);

        ////

        buffer.reset();

        for ack in [1, 5, 9, 11].iter() {
            buffer.insert(TestData{ sequence: *ack as u16 }, *ack as u16).unwrap();
        }

        let (ack, ack_bits) = buffer.ack_bits();

        assert_eq!(ack, 11);
        assert_eq!(ack_bits, ( 1 | (1<<(11-9)) | (1<<(11-5)) | (1<<(11-1)) ) );
    }

    #[test]
    fn sequence_test() {
        enable_logging();

        #[derive(Debug, Clone, Default)]
        struct TestData {
            sequence: u16,
        }

        let mut buffer = SequenceBuffer::<TestData>::with_capacity(TEST_BUFFER_SIZE);

        assert_eq!(buffer.capacity(), TEST_BUFFER_SIZE);
        assert_eq!(buffer.sequence(), 0);

        for i in 0..TEST_BUFFER_SIZE {
            let r = buffer.get(i as u16);
            assert!(r.is_none());
        }

        for i in 0..TEST_BUFFER_SIZE*4 {
            buffer.insert(TestData{ sequence: i as u16 }, i as u16).unwrap();
            assert_eq!(buffer.sequence(), i as u16 + 1);

            let r = buffer.get(i as u16);
            assert_eq!(r.unwrap().sequence, i as u16);
        }

        for i in 0..TEST_BUFFER_SIZE-1 {
            let r = buffer.insert(TestData{ sequence: i as u16 }, i as u16);
            assert!(r.is_err());
        }

        let mut index = TEST_BUFFER_SIZE * 4-1;
        for _ in 0..TEST_BUFFER_SIZE-1  {
            let entry = buffer.get(index as u16);
            assert!(entry.is_some());
            let e = entry.unwrap();
            assert_eq!(e.sequence, index as u16);
            index = index - 1;
        }

    }

    #[test]
    fn fragment_header() {
        let write_id: u8 = 111;
        let write_num : u8 = 123;
        let write_sequence : u16 = 999;

        let write_fragment = FragmentHeader::new_fragment(write_id, write_num, write_sequence);

        let mut buffer = vec![];
        buffer.resize(RELIABLE_MAX_PACKET_HEADER_BYTES, 0);
        let mut cursor = std::io::Cursor::new(buffer.as_mut_slice());

        write_fragment.write(&mut cursor).unwrap();

        let mut cursor = std::io::Cursor::new(buffer.as_slice());
        let read_fragment = FragmentHeader::parse(&mut cursor).unwrap();

        assert_eq!(write_fragment.sequence(), read_fragment.sequence());
        assert_eq!(write_fragment.id(), read_fragment.id());
        assert_eq!(write_fragment.count(), read_fragment.count());

    }

    #[test]
    fn packet_header() {
        enable_logging();

        let write_sequence = 10000;
        let write_ack = 100;
        let write_ack_bits = 0;

        let mut buffer = vec![];
        buffer.resize(RELIABLE_MAX_PACKET_HEADER_BYTES, 0);
        let mut cursor = std::io::Cursor::new(buffer.as_mut_slice());

        let write_packet = PacketHeader::new(write_sequence, write_ack, write_ack_bits);
        write_packet.write(&mut cursor).unwrap();

        let mut cursor = std::io::Cursor::new(buffer.as_slice());
        let read_packet = PacketHeader::parse(&mut cursor).unwrap();

        assert_eq!(write_packet.sequence(), read_packet.sequence());
        assert_eq!(write_packet.ack(), read_packet.ack());
        assert_eq!(write_packet.ack_bits(), read_packet.ack_bits());
    }

    #[test]
    fn rust_impl_endpoint() {
        enable_logging();

        let _endpoint = Endpoint::new(EndpointConfig::new("balls"), 0.0,
                                     |_, _, _| trace!("send"),
                                     |_, _, _| { trace!("recv"); true }
        );

    }
}