use std::collections::HashMap;

use crate::cycle::status_registers::TrapReason;

pub trait MemorySource {
    fn set(&mut self, phys_address: u64, value: u32, access_type: AccessType, trap: &mut u32);
    fn get(&self, phys_address: u64, access_type: AccessType, trap: &mut u32) -> u32;
}

pub struct VectorMemoryImpl {
    pub(crate) inner: Vec<u32>,
}

impl VectorMemoryImpl {
    pub fn new_for_byte_size(bytes: usize) -> Self {
        assert_eq!(bytes % 4, 0);
        let word_size = bytes / 4;
        Self {
            inner: vec![0u32; word_size],
        }
    }
}

impl MemorySource for VectorMemoryImpl {
    #[must_use]
    #[inline(always)]
    fn get(&self, phys_address: u64, access_type: AccessType, trap: &mut u32) -> u32 {
        if ((phys_address / 4) as usize) < self.inner.len() {
            self.inner[(phys_address / 4) as usize]
        } else {
            match access_type {
                AccessType::Instruction => {
                    *trap = TrapReason::InstructionAccessFault.as_register_value()
                }
                AccessType::Load => *trap = TrapReason::LoadAccessFault.as_register_value(),
                AccessType::Store => *trap = TrapReason::StoreOrAMOAccessFault.as_register_value(),
            }

            0
        }
    }

    #[inline(always)]
    fn set(&mut self, phys_address: u64, value: u32, access_type: AccessType, trap: &mut u32) {
        if ((phys_address / 4) as usize) < self.inner.len() {
            self.inner[(phys_address / 4) as usize] = value;
        } else {
            match access_type {
                AccessType::Instruction => {
                    *trap = TrapReason::InstructionAccessFault.as_register_value()
                }
                AccessType::Load => *trap = TrapReason::LoadAccessFault.as_register_value(),
                AccessType::Store => *trap = TrapReason::StoreOrAMOAccessFault.as_register_value(),
            }
        }
    }
}

pub trait Timestamp: 'static + Clone + Copy + std::fmt::Debug {
    fn new_cycle_timestamp(self) -> Self;
    fn update_after_subaccess(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum AccessType {
    Instruction = 0,
    Load = 1,
    Store = 2,
}

// we deliberately assume that no more 16 accesses can happen per cycle
impl Timestamp for u32 {
    fn new_cycle_timestamp(self) -> Self {
        let new = self & !(15u32); // cleanup low
        new.wrapping_add(16u32) // increment
    }

    fn update_after_subaccess(&mut self) {
        *self = self.wrapping_add(1u32)
    }
}

pub trait MemoryAccessTracer {
    type Timestamp: Timestamp;

    fn add_query(
        &mut self,
        phys_address: u64,
        value: u32,
        rw_flag: bool,
        access_type: AccessType,
        ts: Self::Timestamp,
        trap: u32,
    );
}

impl MemoryAccessTracer for () {
    type Timestamp = u32;

    #[inline(always)]
    fn add_query(
        &mut self,
        _phys_address: u64,
        _value: u32,
        _rw_flag: bool,
        _access_type: AccessType,
        _ts: Self::Timestamp,
        _trap: u32,
    ) {
    }
}

pub struct MemoryAccessTracerImpl {
    pub instruction_trace: HashMap<usize, u32>,
    pub memory_trace: HashMap<usize, (u64, u32, bool, u32)>,
}

impl MemoryAccessTracerImpl {
    pub fn new() -> Self {
        Self {
            instruction_trace: HashMap::new(),
            memory_trace: HashMap::new(),
        }
    }

    pub fn get(&self, ts: usize) -> (Option<u32>, Option<(u64, u32, bool, u32)>) {
        (
            self.instruction_trace.get(&ts).copied(),
            self.memory_trace.get(&ts).copied(),
        )
    }
}

impl MemoryAccessTracer for MemoryAccessTracerImpl {
    type Timestamp = u32;

    #[inline(always)]
    fn add_query(
        &mut self,
        phys_address: u64,
        value: u32,
        rw_flag: bool,
        access_type: AccessType,
        ts: Self::Timestamp,
        trap: u32,
    ) {
        match access_type {
            AccessType::Instruction => {
                self.instruction_trace.insert(ts as usize, value);
            }
            _ => {
                self.memory_trace.insert(ts as usize, (phys_address, value, rw_flag, trap));
            }
        }
    }
}