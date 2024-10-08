// there is no interpretation of methods here, it's just read/write and that's all
pub trait NonDeterminismCSRSource<M: MemorySource> {
    const SHOULD_MOCK_READS_BEFORE_WRITES: bool = true;
    const SHOULD_IGNORE_WRITES_AFTER_READS: bool = true;

    fn read(&mut self) -> u32;

    // we in general can allow CSR source to peek into memory (readonly)
    // to perform adhoc computations to prepare result. This will allow to save on
    // passing large structures
    fn write_with_memory_access(&mut self, memory: &M, value: u32);
}

pub struct ZeroedSource;

impl<M: MemorySource> NonDeterminismCSRSource<M> for ZeroedSource {
    fn read(&mut self) -> u32 {
        0u32
    }
    fn write_with_memory_access(&mut self, _memory: &M, _value: u32) {}
}

use std::collections::VecDeque;

use ringbuffer::RingBuffer;

use super::memory::MemorySource;

#[derive(Clone, Debug)]
pub struct QuasiUARTSource {
    pub oracle: VecDeque<u32>,
    write_state: QuasiUARTSourceState,
    last_values_buffer: ringbuffer::ConstGenericRingBuffer<u32, 8>,
}

impl Default for QuasiUARTSource {
    fn default() -> Self {
        Self {
            oracle: VecDeque::new(),
            write_state: QuasiUARTSourceState::Ready,
            last_values_buffer: ringbuffer::ConstGenericRingBuffer::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum QuasiUARTSourceState {
    Ready,
    Buffering {
        remaining_len: Option<usize>,
        buffer: Vec<u8>,
    },
}

impl QuasiUARTSource {
    const HELLO_VALUE: u32 = u32::MAX;
}

impl<M: MemorySource> NonDeterminismCSRSource<M> for QuasiUARTSource {
    fn read(&mut self) -> u32 {
        self.oracle.pop_front().unwrap()
    }

    fn write_with_memory_access(&mut self, _memory: &M, value: u32) {
        self.last_values_buffer.push(value);
        match &mut self.write_state {
            QuasiUARTSourceState::Ready => {
                if value == Self::HELLO_VALUE {
                    self.write_state = QuasiUARTSourceState::Buffering {
                        remaining_len: None,
                        buffer: Vec::new(),
                    };
                }
            }
            QuasiUARTSourceState::Buffering {
                remaining_len,
                buffer,
            } => {
                let mut reset_and_output = false;
                if let Some(remaining_len) = remaining_len.as_mut() {
                    if *remaining_len >= 4 {
                        buffer.extend(value.to_le_bytes());
                        *remaining_len -= 4;
                    } else {
                        let remaining_len = *remaining_len;
                        let bytes = value.to_le_bytes();
                        buffer.extend_from_slice(&bytes[..remaining_len]);
                        reset_and_output = true;
                    }
                } else {
                    *remaining_len = Some(value as usize);
                    buffer.reserve(value as usize);
                }
                if reset_and_output {
                    let buffer = std::mem::replace(buffer, Vec::new());
                    // let string = String::from_utf8(buffer).unwrap();
                    // println!("UART: `{}`", string);
                    println!("UART: `{}`", String::from_utf8_lossy(&buffer));
                    self.write_state = QuasiUARTSourceState::Ready;
                }
            }
        }
    }
}

impl Drop for QuasiUARTSource {
    fn drop(&mut self) {
        println!("Total program value output:");
        for el in self.last_values_buffer.iter() {
            println!("0x{:08x}", el);
        }
    }
}
