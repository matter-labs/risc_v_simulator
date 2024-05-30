use crate::cycle::status_registers::TrapReason;
use std::{collections::BTreeMap, f32::consts::E};


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum AccessType {
    Instruction = 0,
    RegReadFirst = 3,
    RegReadSecond = 4,
    MemLoad = 1,
    MemStore = 2,
    RegWrite = 5,
    PadAccess = 6
}
pub const NUM_DIFFERENT_ACCESS_TYPES: usize = 6;

impl AccessType {
    pub fn is_write_access(&self) -> bool {
        match self {
            AccessType::MemStore | AccessType::RegWrite => true,
            _ => false
        }
    }

    pub fn is_read_access(&self) -> bool {
        !self.is_write_access()
    }

    pub fn is_reg_access(&self) -> bool {
        match self {
            AccessType::RegReadFirst | AccessType::RegReadSecond | AccessType::RegWrite => true,
            _ => false
        }
    }

    pub fn from_idx(idx: u32) -> Self {
        match idx {
            0 => AccessType::Instruction,
            1 => AccessType::RegReadFirst,
            2 => AccessType::RegReadSecond,
            3 => AccessType::MemLoad,
            4 => AccessType::MemStore,
            5 => AccessType::RegWrite,
            _ => AccessType::PadAccess
        }
    }
}

pub trait MemorySource {
    fn set(&mut self, phys_address: u64, value: u32,access_type: AccessType, trap: &mut TrapReason);
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
                _ => unreachable!()
            }
        }
    }
}


pub struct MemoryAuxData {
    pub unsorted_mem_queries: Vec<IndexedMemQuery>,
    pub sorted_mem_queries: Vec<IndexedMemQuery>
}

impl MemoryAuxData {
    pub fn stub() -> Self {
        MemoryAuxData {
            unsorted_mem_queries: vec![],
            sorted_mem_queries: vec![]
        }
    }
}

pub trait MemoryAccessTracer {
    fn add_query(&mut self, proc_cycle: u32, access_type: AccessType, phys_address: u64, value: u32);
    fn sort_queries(&self, should_include_reg_queries: bool, height: u32) -> MemoryAuxData;
}

impl MemoryAccessTracer for () {
    #[inline(always)]
    fn add_query(&mut self, _proc_cycle: u32, _access_type: AccessType, _phys_address: u64, _value: u32) {}
    fn sort_queries(&self, _should_include_reg_queries: bool, _height: u32) -> MemoryAuxData { MemoryAuxData::stub() }
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemQuery {
    pub access_type: AccessType,
    pub address: u64,
    pub value: u32
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedMemQuery {
    pub execute: bool,
    pub rw_flag: bool,
    pub timestamp: u32,
    pub address: u64,
    pub value: u32
}

impl IndexedMemQuery {
    pub fn stub(timestamp: u32, access_type: AccessType) -> Self {
        IndexedMemQuery {
            execute: false,
            rw_flag: access_type.is_write_access(),
            timestamp,
            address: 0,
            value: 0
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
    pub memory_trace: BTreeMap<u32, MemoryAccesesPerStep>,
}

impl MemoryAccessTracerImpl {
    pub fn new() -> Self {
        Self {
            memory_trace: BTreeMap::new(),
        }
    }
}


impl MemoryAccessTracer for MemoryAccessTracerImpl {
    #[inline(always)]
    fn add_query(&mut self, proc_cycle: u32, access_type: AccessType, phys_address: u64, value: u32) {
        let entry = self.memory_trace.entry(proc_cycle).or_insert(MemoryAccesesPerStep::new());
        let query = MemQuery { access_type, address: phys_address, value };
        entry.0[access_type as usize] = Some(query);
    }

    fn sort_queries(&self, should_include_reg_queries: bool, height: u32) -> MemoryAuxData {
        assert!(should_include_reg_queries);
        let mut unsorted_mem_queries: Vec<IndexedMemQuery> = Vec::with_capacity(height as usize * self.memory_trace.len());
        
        for (&proc_cycle, query_arr) in self.memory_trace.iter() {
            let mut instruction: u32 = 0;
            for sub_idx in 0..height {
                let access_type = AccessType::from_idx(sub_idx);
                let timestamp = proc_cycle * height + sub_idx;

                if sub_idx as usize >= NUM_DIFFERENT_ACCESS_TYPES {
                    let indexed_query = IndexedMemQuery::stub(timestamp, access_type);
                    unsorted_mem_queries.push(indexed_query);
                    continue;
                }

                let query_option = query_arr.0[sub_idx as usize];
                let indexed_query = query_option.map(|actual_query| {
                    if access_type == AccessType::Instruction {
                        instruction = actual_query.value
                    }
    
                    IndexedMemQuery {
                        execute: true,
                        rw_flag: access_type.is_write_access(),
                        timestamp,
                        address: actual_query.address,
                        value: actual_query.value
                    }
                }).unwrap_or(
                    IndexedMemQuery::stub(timestamp, access_type)
                );
                unsorted_mem_queries.push(indexed_query);
            }
        }

        let mut sorted_mem_queries = unsorted_mem_queries.clone();
        sorted_mem_queries.sort_by_key(|query| {
            // | execute | address | proc_cycle
            let mut key: u128 = query.execute as u128;
            key <<= 64;
            key += query.address as u128;
            key <<= 32;
            key += query.timestamp as u128;
            key
        });
        
        MemoryAuxData {
            unsorted_mem_queries,
            sorted_mem_queries
        }
    }
}


pub struct TestMemoryTracer {
    memory_queries: Vec<IndexedMemQuery>
}

impl TestMemoryTracer {
    pub fn new() -> Self {
        TestMemoryTracer { memory_queries: vec![] }
    }

    pub fn add_raw_query(&mut self, raw_query: [u32; 8]) {
        let [timestamp_low, timestamp_high, rw, execute, addr_low, addr_high, value_low, value_high] = raw_query;
        let timestamp = (timestamp_high << 20) + timestamp_low;
        let address = (addr_high << 16) + addr_low;
        let value = (value_high << 16) + value_low;

        let memory_query = IndexedMemQuery {
            execute: execute != 0,
            rw_flag: rw != 0,
            timestamp,
            address: address as u64,
            value
        };

        self.memory_queries.push(memory_query);
    }
}

impl MemoryAccessTracer for TestMemoryTracer {
    #[inline(always)]
    fn add_query(&mut self, _proc_cycle: u32, _access_type: AccessType, _phys_address: u64, _value: u32) {
    }

    fn sort_queries(&self, _should_include_reg_queries: bool, _height: u32) -> MemoryAuxData {
        let unsorted_mem_queries: Vec<IndexedMemQuery> = self.memory_queries.clone();
        let mut sorted_mem_queries = self.memory_queries.clone();
      
        sorted_mem_queries.sort_by_key(|query| {
            // | execute | address | proc_cycle
            let mut key: u128 = query.execute as u128;
            key <<= 64;
            key += query.address as u128;
            key <<= 32;
            key += query.timestamp as u128;
            key
        });
        
        MemoryAuxData {
            unsorted_mem_queries,
            sorted_mem_queries
        }
    }
}
