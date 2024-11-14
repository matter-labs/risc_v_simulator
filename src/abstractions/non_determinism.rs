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

use super::memory::MemorySource;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct QuasiUARTSource {
    pub oracle: VecDeque<u32>,
    write_state: QuasiUARTSourceState,
}

impl Default for QuasiUARTSource {
    fn default() -> Self {
        Self {
            oracle: VecDeque::new(),
            write_state: QuasiUARTSourceState::Ready,
        }
    }
}

#[derive(Clone, Debug)]
pub enum QuasiUARTSourceState {
    Ready,
    Buffering {
        remaining_words: Option<usize>,
        remaining_len_in_bytes: Option<usize>,
        buffer: Vec<u8>,
    },
}

impl QuasiUARTSourceState {
    const HELLO_VALUE: u32 = u32::MAX;

    pub fn process_write(&mut self, value: u32) {
        match self {
            QuasiUARTSourceState::Ready => {
                if value == Self::HELLO_VALUE {
                    *self = QuasiUARTSourceState::Buffering {
                        remaining_words: None,
                        remaining_len_in_bytes: None,
                        buffer: Vec::new(),
                    };
                }
            }
            QuasiUARTSourceState::Buffering {
                remaining_words,
                remaining_len_in_bytes,
                buffer,
            } => {
                if remaining_words.is_none() {
                    *remaining_words = Some(value as usize);
                    buffer.clear();
                    return;
                }
                if remaining_len_in_bytes.is_none() {
                    assert!(remaining_words.is_some());
                    *remaining_words.as_mut().unwrap() -= 1;
                    *remaining_len_in_bytes = Some(value as usize);
                    buffer.reserve(value as usize);

                    return;
                }
                *remaining_words.as_mut().unwrap() -= 1;
                if remaining_len_in_bytes.unwrap() >= 4 {
                    buffer.extend(value.to_le_bytes());
                    *remaining_len_in_bytes.as_mut().unwrap() -= 4;
                } else {
                    let remaining_len = remaining_len_in_bytes.unwrap();
                    let bytes = value.to_le_bytes();
                    buffer.extend_from_slice(&bytes[..remaining_len]);
                    *remaining_len_in_bytes.as_mut().unwrap() = 0;
                }
                if remaining_words.unwrap() == 0 {
                    let buffer = std::mem::replace(buffer, Vec::new());
                    // let string = String::from_utf8(buffer).unwrap();
                    // println!("UART: `{}`", string);
                    println!("UART: `{}`", String::from_utf8_lossy(&buffer));
                    *self = QuasiUARTSourceState::Ready;
                }
                // buffer.extend(value.to_le_bytes());
                // if

                // let message_len_in_bytes = *message_len_in_bytes.unwrap();
                // let mut reset_and_output = false;
                // if let Some(remaining_len) = remaining_len.as_mut() {
                //     if *remaining_len >= 4 {
                //         buffer.extend(value.to_le_bytes());
                //         *remaining_len -= 4;
                //     } else {
                //         let remaining_len = *remaining_len;
                //         let bytes = value.to_le_bytes();
                //         buffer.extend_from_slice(&bytes[..remaining_len]);
                //         reset_and_output = true;
                //     }
                // } else {
                //     *remaining_len = Some(value as usize);
                //     buffer.reserve(value as usize);
                // }
                // if reset_and_output {
                //     let buffer = std::mem::replace(buffer, Vec::new());
                //     // let string = String::from_utf8(buffer).unwrap();
                //     // println!("UART: `{}`", string);
                //     println!("UART: `{}`", String::from_utf8_lossy(&buffer));
                //     self.write_state = QuasiUARTSourceState::Ready;
                // }
            }
        }
    }
}

impl<M: MemorySource> NonDeterminismCSRSource<M> for QuasiUARTSource {
    fn read(&mut self) -> u32 {
        self.oracle.pop_front().unwrap_or_default()
        // self.oracle.pop_front().unwrap()
    }

    fn write_with_memory_access(&mut self, _memory: &M, value: u32) {
        self.write_state.process_write(value);
    }
}
