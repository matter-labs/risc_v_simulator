use std::collections::HashMap;
use crate::cycle::status_registers::TrapReason;


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum AccessType {
    Instruction = 0,
    Load = 1,
    Store = 2,
}


pub trait MemorySource {
    fn set(&mut self, phys_address: u64, value: u32, access_type: AccessType, trap: &mut TrapReason);
    fn get(&self, phys_address: u64, access_type: AccessType, trap: &mut TrapReason) -> u32;
}

pub struct VectorMemoryImpl {
    pub inner: Vec<u32>,
}

impl VectorMemoryImpl {
    pub fn new_for_byte_size(bytes: usize) -> Self {
        assert_eq!(bytes % 4, 0);
        let word_size = bytes / 4;
        Self {
            inner: vec![0u32; word_size],
        }
    }

    pub fn populate(&mut self, address: u32, value: u32) {
        assert!(address % 4 == 0);
        self.inner[(address / 4) as usize] = value;
    } 
}

impl MemorySource for VectorMemoryImpl {
    #[must_use]
    #[inline(always)]
    fn get(&self, phys_address: u64, access_type: AccessType, trap: &mut TrapReason) -> u32 {
        if ((phys_address / 4) as usize) < self.inner.len() {
            self.inner[(phys_address / 4) as usize]
        } else {
            match access_type {
                AccessType::Instruction => {
                    *trap = TrapReason::InstructionAccessFault
                }
                AccessType::Load => *trap = TrapReason::LoadAccessFault,
                AccessType::Store => *trap = TrapReason::StoreOrAMOAccessFault,
            }

            0
        }
    }

    #[inline(always)]
    fn set(&mut self, phys_address: u64, value: u32, access_type: AccessType, trap: &mut TrapReason) {
        if ((phys_address / 4) as usize) < self.inner.len() {
            self.inner[(phys_address / 4) as usize] = value;
        } else {
            match access_type {
                AccessType::Instruction => {
                    *trap = TrapReason::InstructionAccessFault
                }
                AccessType::Load => *trap = TrapReason::LoadAccessFault,
                AccessType::Store => *trap = TrapReason::StoreOrAMOAccessFault,
            }
        }
    }
}

pub trait Timestamp: 'static + Clone + Copy + std::fmt::Debug {
    fn new_cycle_timestamp(self, queries_per_cycle: u32) -> Self;
    fn get_and_update(&mut self) -> Self;
}

// we deliberately assume that no more 16 accesses can happen per cycle
impl Timestamp for u32 {
    fn get_and_update(&mut self) -> u32 {
        let res = *self;
        *self = self.wrapping_add(1u32);
        res
    }

    fn new_cycle_timestamp(self, queries_per_cycle: u32) -> Self {
        queries_per_cycle
    }
}

pub trait MemoryAccessTracer {
    type Timestamp: Timestamp;

    fn add_query(
        &mut self,
        ts: Self::Timestamp,
        access_type: AccessType,
        phys_address: u64,
        value: u32
    );
}

impl MemoryAccessTracer for () {
    type Timestamp = u32;

    #[inline(always)]
    fn add_query(
        &mut self,
        _ts: Self::Timestamp,
        _access_type: AccessType,
        _phys_address: u64,
        _value: u32
    ) {
    }
}

pub struct MemoryAccessTracerImpl {
    pub memory_trace: HashMap<usize, (AccessType, u64, u32)>,
}

impl MemoryAccessTracerImpl {
    pub fn new() -> Self {
        Self {
            memory_trace: HashMap::new(),
        }
    }

    pub fn get(&self, ts: usize) -> Option<(AccessType, u64, u32)> {
        self.memory_trace.get(&ts).copied()
    }
}

impl MemoryAccessTracer for MemoryAccessTracerImpl {
    type Timestamp = u32;

    #[inline(always)]
    fn add_query(
        &mut self,
        ts: Self::Timestamp,
        access_type: AccessType,
        phys_address: u64,
        value: u32    
    ) {
       self.memory_trace.insert(ts as usize, (access_type, phys_address, value));
    }
}