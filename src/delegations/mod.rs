use blake2s::blake2s_round_function;
use blake2s::BLAKE2S_ACCESS_ID;

use blake2_round_function::blake2_round_function;
use blake2_round_function::BLAKE2_ROUND_FUNCTION_ACCESS_ID;

use blake2_round_function_with_final_xor::blake2_round_function_with_xor;
use blake2_round_function_with_final_xor::BLAKE2_ROUND_FUNCTION_WITH_XOR_ACCESS_ID;

use crate::abstractions::csr_processor::CustomCSRProcessor;
use crate::abstractions::memory::*;
use crate::abstractions::tracer::*;
use crate::cycle::status_registers::TrapReason;
use crate::cycle::MachineConfig;
use crate::mmu::*;

pub mod blake2_round_function;
pub mod blake2_round_function_with_final_xor;
pub mod blake2s;

#[derive(Clone, Copy, Debug)]
pub struct DelegationsCSRProcessor;

impl CustomCSRProcessor for DelegationsCSRProcessor {
    #[inline(always)]
    fn process_read<
        M: MemorySource,
        TR: Tracer<C>,
        MMU: MMUImplementation<M, TR, C>,
        C: MachineConfig,
    >(
        &mut self,
        _memory_source: &mut M,
        _tracer: &mut TR,
        _mmu: &mut MMU,
        csr_index: u32,
        _rs1_value: u32,
        _zimm: u32,
        ret_val: &mut u32,
        trap: &mut TrapReason,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
        *ret_val = 0;
        match csr_index {
            BLAKE2S_ACCESS_ID => {}
            BLAKE2_ROUND_FUNCTION_ACCESS_ID => {}
            BLAKE2_ROUND_FUNCTION_WITH_XOR_ACCESS_ID => {}
            _ => {
                *trap = TrapReason::IllegalInstruction;
            }
        }
    }

    #[inline(always)]
    fn process_write<
        M: MemorySource,
        TR: Tracer<C>,
        MMU: MMUImplementation<M, TR, C>,
        C: MachineConfig,
    >(
        &mut self,
        memory_source: &mut M,
        tracer: &mut TR,
        mmu: &mut MMU,
        csr_index: u32,
        rs1_value: u32,
        _zimm: u32,
        trap: &mut TrapReason,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
        match csr_index {
            BLAKE2S_ACCESS_ID => {
                blake2s_round_function::<_, _, _, _, false>(
                    memory_source,
                    tracer,
                    mmu,
                    rs1_value,
                    trap,
                    proc_cycle,
                    cycle_timestamp,
                );
            }
            BLAKE2_ROUND_FUNCTION_ACCESS_ID => {
                blake2_round_function::<_, _, _, _>(
                    memory_source,
                    tracer,
                    mmu,
                    rs1_value,
                    trap,
                    proc_cycle,
                    cycle_timestamp,
                );
            }
            BLAKE2_ROUND_FUNCTION_WITH_XOR_ACCESS_ID => {
                blake2_round_function_with_xor::<_, _, _, _>(
                    memory_source,
                    tracer,
                    mmu,
                    rs1_value,
                    trap,
                    proc_cycle,
                    cycle_timestamp,
                );
            }
            _ => {
                *trap = TrapReason::IllegalInstruction;
            }
        }
    }
}
