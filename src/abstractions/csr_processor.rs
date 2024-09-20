use std::fmt::Debug;

use crate::abstractions::*;
use crate::cycle::status_registers::TrapReason;
use crate::mmu::MMUImplementation;

pub trait CustomCSRProcessor: 'static + Clone + Debug {
    // we are only interested in CSRs that are NOT in out basic list
    fn process_read<M: MemorySource, TR: Tracer, MMU: MMUImplementation<M, TR>>(
        &mut self,
        memory_source: &mut M,
        tracer: &mut TR,
        mmu: &mut MMU,
        csr_index: u32,
        rs1_value: u32,
        zimm: u32,
        ret_val: &mut u32,
        trap: &mut TrapReason,
        proc_cycle: u32,
        cycle_timestamp: u32,
    );
    fn process_write<M: MemorySource, TR: Tracer, MMU: MMUImplementation<M, TR>>(
        &mut self,
        memory_source: &mut M,
        tracer: &mut TR,
        mmu: &mut MMU,
        csr_index: u32,
        rs1_value: u32,
        zimm: u32,
        trap: &mut TrapReason,
        proc_cycle: u32,
        cycle_timestamp: u32,
    );
}

#[derive(Clone, Copy, Debug)]
pub struct NoExtraCSRs;

impl CustomCSRProcessor for NoExtraCSRs {
    #[inline(always)]
    fn process_read<M: MemorySource, TR: Tracer, MMU: MMUImplementation<M, TR>>(
        &mut self,
        _memory_source: &mut M,
        _tracer: &mut TR,
        _mmu: &mut MMU,
        _csr_index: u32,
        _rs1_value: u32,
        _zimm: u32,
        ret_val: &mut u32,
        _trap: &mut TrapReason,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
        *ret_val = 0;
    }

    #[inline(always)]
    fn process_write<M: MemorySource, TR: Tracer, MMU: MMUImplementation<M, TR>>(
        &mut self,
        _memory_source: &mut M,
        _tracer: &mut TR,
        _mmu: &mut MMU,
        _csr_index: u32,
        _rs1_value: u32,
        _zimm: u32,
        trap: &mut TrapReason,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
        *trap = TrapReason::IllegalInstruction;
    }
}
