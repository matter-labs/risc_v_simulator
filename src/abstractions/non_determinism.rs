// there is no interpretation of methods here, it's just read/write and that's all
pub trait NonDeterminismCSRSource {
    fn read(&mut self) -> u32;
    fn write(&mut self, value: u32);
}

pub struct ZeroedSource;

impl NonDeterminismCSRSource for ZeroedSource {
    fn read(&mut self) -> u32 {
        0u32
    }
    fn write(&mut self, _value: u32) {}
}

use std::{collections::VecDeque, ffi::CString};

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
        remaining_len: Option<usize>,
        buffer: Vec<u8>,
    },
}

impl NonDeterminismCSRSource for QuasiUARTSource {
    fn read(&mut self) -> u32 {
        self.oracle.pop_front().unwrap()
    }

    fn write(&mut self, value: u32) {
        match &mut self.write_state {
            QuasiUARTSourceState::Ready => {
                self.write_state = QuasiUARTSourceState::Buffering {
                    remaining_len: None,
                    buffer: Vec::new(),
                };
            }
            QuasiUARTSourceState::Buffering {
                remaining_len,
                buffer,
            } => {
                let mut reset = false;
                if let Some(remaining_len) = remaining_len.as_mut() {
                    if *remaining_len >= 4 {
                        buffer.extend(value.to_le_bytes());
                        *remaining_len -= 4;
                    } else {
                        let remaining_len = *remaining_len;
                        let bytes = value.to_le_bytes();
                        buffer.extend_from_slice(&bytes[..remaining_len]);
                        reset = true;
                    }
                } else {
                    *remaining_len = Some(value as usize);
                    buffer.reserve(value as usize);
                }
                if reset {
                    let buffer = std::mem::replace(buffer, Vec::new());
                    let string = String::from_utf8(buffer).unwrap();
                    println!("UART: `{}`", string);
                }
                self.write_state = QuasiUARTSourceState::Ready;
            }
        }
    }
}
