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

#[derive(Clone, Debug, Default)]
pub struct QuasiUARTSource {
    pub oracle: VecDeque<u32>,
    pub buffer: Vec<u8>,
}

pub const QUASI_UART_ADDRESS: u32 = 0x0000_0004;

impl NonDeterminismCSRSource for QuasiUARTSource {
    fn read(&mut self) -> u32 {
        self.oracle.pop_front().unwrap_or(0u32)
    }

    fn write(&mut self, value: u32) {
        self.buffer.extend(value.to_le_bytes());
        let len = self.buffer.len();
        for idx in (len - 4)..len {
            if self.buffer[idx] == 0 {
                // c-style string can be made
                let mut buffer = std::mem::replace(&mut self.buffer, vec![]);
                buffer.truncate(idx + 1);
                let c_string = CString::from_vec_with_nul(buffer).unwrap();
                println!("UART: `{}`", c_string.to_string_lossy());
                break;
            }
        }
    }
}
