use crate::cycle::status_registers::TrapReason;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum AccessType {
    Instruction = 0,
    MemLoad = 1,
    MemStore = 2,
    RegReadFirst = 3,
    RegReadSecond = 4,
    RegWrite = 5,
    None = 6,
}
pub const NUM_DIFFERENT_ACCESS_TYPES: usize = 6;

impl AccessType {
    pub fn is_write_access(&self) -> bool {
        match self {
            AccessType::MemStore | AccessType::RegWrite | AccessType::None => true,
            _ => false,
        }
    }

    pub fn is_read_access(&self) -> bool {
        !self.is_write_access()
    }

    pub fn is_reg_access(&self) -> bool {
        match self {
            AccessType::RegReadFirst | AccessType::RegReadSecond | AccessType::RegWrite => true,
            _ => false,
        }
    }

    pub fn from_idx(idx: u32) -> Self {
        match idx {
            0 => AccessType::Instruction,
            1 => AccessType::MemLoad,
            2 => AccessType::MemStore,
            3 => AccessType::RegReadFirst,
            4 => AccessType::RegReadSecond,
            5 => AccessType::RegWrite,
            _ => AccessType::None,
        }
    }
}

pub trait MemorySource {
    fn set(
        &mut self,
        phys_address: u64,
        value: u32,
        access_type: AccessType,
        trap: &mut TrapReason,
    );
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
                AccessType::Instruction => *trap = TrapReason::InstructionAccessFault,
                AccessType::MemLoad => *trap = TrapReason::LoadAccessFault,
                AccessType::MemStore => *trap = TrapReason::StoreOrAMOAccessFault,
                _ => unreachable!(),
            }

            0
        }
    }

    #[inline(always)]
    fn set(
        &mut self,
        phys_address: u64,
        value: u32,
        access_type: AccessType,
        trap: &mut TrapReason,
    ) {
        if ((phys_address / 4) as usize) < self.inner.len() {
            self.inner[(phys_address / 4) as usize] = value;
        } else {
            match access_type {
                AccessType::Instruction => *trap = TrapReason::InstructionAccessFault,
                AccessType::MemLoad => *trap = TrapReason::LoadAccessFault,
                AccessType::MemStore => *trap = TrapReason::StoreOrAMOAccessFault,
                _ => unreachable!(),
            }
        }
    }
}

pub trait MemoryAccessTracer {
    fn add_query(
        &mut self,
        proc_cycle: u32,
        access_type: AccessType,
        phys_address: u64,
        value: u32,
    );
    fn sort_queries(&self, should_include_reg_queries: bool, height: u32) -> Vec<IndexedMemQuery>;
}

impl MemoryAccessTracer for () {
    #[inline(always)]
    fn add_query(
        &mut self,
        _proc_cycle: u32,
        _access_type: AccessType,
        _phys_address: u64,
        _value: u32,
    ) {
    }
    fn sort_queries(
        &self,
        _should_include_reg_queries: bool,
        _height: u32,
    ) -> Vec<IndexedMemQuery> {
        vec![]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemQuery {
    pub access_type: AccessType,
    pub address: u64,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedMemQuery {
    pub execute: bool,
    pub is_read_flag: bool,
    pub timestamp: u32,
    pub address: u64,
    pub value: u32,
}

impl IndexedMemQuery {
    pub fn default_for_timestamp(timestamp: u32) -> Self {
        IndexedMemQuery {
            execute: false,
            is_read_flag: false, // by our conventon default operation is write (see air_compiler for details)
            timestamp,
            address: 0,
            value: 0,
        }
    }
}

pub struct MemoryAccesesPerStep(pub [Option<MemQuery>; NUM_DIFFERENT_ACCESS_TYPES]);

impl MemoryAccesesPerStep {
    pub fn new() -> Self {
        MemoryAccesesPerStep([None; NUM_DIFFERENT_ACCESS_TYPES])
    }
}

pub struct MemoryAccessTracerImpl {
    pub memory_trace: HashMap<u32, MemoryAccesesPerStep>,
}

impl MemoryAccessTracerImpl {
    pub fn new() -> Self {
        Self {
            memory_trace: HashMap::new(),
        }
    }
}

impl MemoryAccessTracer for MemoryAccessTracerImpl {
    #[inline(always)]
    fn add_query(
        &mut self,
        proc_cycle: u32,
        access_type: AccessType,
        phys_address: u64,
        value: u32,
    ) {
        let entry = self
            .memory_trace
            .entry(proc_cycle)
            .or_insert(MemoryAccesesPerStep::new());
        let query = MemQuery {
            access_type,
            address: phys_address,
            value,
        };
        entry.0[access_type as usize] = Some(query);
    }

    fn sort_queries(&self, should_include_reg_queries: bool, height: u32) -> Vec<IndexedMemQuery> {
        let mut res: Vec<IndexedMemQuery> =
            Vec::with_capacity(height as usize * self.memory_trace.len());

        for (&proc_cycle, query_arr) in self.memory_trace.iter() {
            for sub_idx in 0..height {
                let timestamp = proc_cycle * height + sub_idx;
                let query_option = query_arr.0.get(sub_idx as usize);
                let indexed_query = query_option
                    .map(|x| {
                        let access_type = AccessType::from_idx(sub_idx);
                        if access_type.is_reg_access() && !should_include_reg_queries {
                            IndexedMemQuery::default_for_timestamp(timestamp)
                        } else {
                            IndexedMemQuery {
                                execute: x.is_some(),
                                is_read_flag: access_type.is_read_access(),
                                timestamp,
                                address: x.map(|x| x.address).unwrap_or(0),
                                value: x.map(|x| x.value).unwrap_or(0),
                            }
                        }
                    })
                    .unwrap_or(IndexedMemQuery::default_for_timestamp(timestamp));
                res.push(indexed_query);
            }
        }

        res.sort_by_key(|query| {
            // | execute | address | proc_cycle
            let mut key: u128 = query.execute as u128;
            key <<= 64;
            key += query.address as u128;
            key <<= 32;
            key += query.timestamp as u128;
            key
        });
        res
    }
}
