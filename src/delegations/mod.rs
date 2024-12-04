use blake2s::blake2s_round_function;
use blake2s::BLAKE2S_ACCESS_ID;

use blake2_round_function::blake2_round_function;
use blake2_round_function::BLAKE2_ROUND_FUNCTION_ACCESS_ID;

use blake2_round_function_with_final_xor::blake2_round_function_with_xor;
use blake2_round_function_with_final_xor::BLAKE2_ROUND_FUNCTION_WITH_XOR_ACCESS_ID;

use blake2_round_function_state_in_registers::blake2_round_function_with_state_in_registers;
use blake2_round_function_state_in_registers::BLAKE2_ROUND_FUNCTION_WITH_STATE_IN_REGISTERS_ACCESS_ID;

use mersenne_ext4_fma::mersenne_ext4_fma_impl;
use mersenne_ext4_fma::MERSENNE_EXT4_FMA_ACCESS_ID;

use poseidon2_provide_witness_and_compress::poseidon2_witness_and_compress;
use poseidon2_provide_witness_and_compress::POSEIDON2_WITNESS_AND_COMPRESS_ACCESS_ID;

use crate::abstractions::csr_processor::CustomCSRProcessor;
use crate::abstractions::memory::*;
use crate::abstractions::non_determinism::NonDeterminismCSRSource;
use crate::abstractions::tracer::*;
use crate::cycle::state::RiscV32State;
use crate::cycle::status_registers::TrapReason;
use crate::cycle::MachineConfig;
use crate::mmu::*;

pub mod blake2_round_function;
pub mod blake2_round_function_state_in_registers;
pub mod blake2_round_function_with_final_xor;
pub mod blake2s;
pub mod mersenne_ext4_fma;
pub mod poseidon2_provide_witness_and_compress;

#[derive(Clone, Copy, Debug)]
pub struct DelegationsCSRProcessor;

impl CustomCSRProcessor for DelegationsCSRProcessor {
    #[inline(always)]
    fn process_read<
        M: MemorySource,
        TR: Tracer<C>,
        ND: NonDeterminismCSRSource<M>,
        MMU: MMUImplementation<M, TR, C>,
        C: MachineConfig,
    >(
        &mut self,
        _state: &mut RiscV32State<C>,
        _memory_source: &mut M,
        _non_determinism_source: &mut ND,
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
            BLAKE2_ROUND_FUNCTION_WITH_STATE_IN_REGISTERS_ACCESS_ID => {}
            MERSENNE_EXT4_FMA_ACCESS_ID => {}
            POSEIDON2_WITNESS_AND_COMPRESS_ACCESS_ID => {}
            _ => {
                *trap = TrapReason::IllegalInstruction;
            }
        }
    }

    #[inline(always)]
    fn process_write<
        M: MemorySource,
        TR: Tracer<C>,
        ND: NonDeterminismCSRSource<M>,
        MMU: MMUImplementation<M, TR, C>,
        C: MachineConfig,
    >(
        &mut self,
        state: &mut RiscV32State<C>,
        memory_source: &mut M,
        non_determinism_source: &mut ND,
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
            BLAKE2_ROUND_FUNCTION_WITH_STATE_IN_REGISTERS_ACCESS_ID => {
                blake2_round_function_with_state_in_registers::<_, _, _, _>(
                    state,
                    memory_source,
                    tracer,
                    mmu,
                    rs1_value,
                    trap,
                    proc_cycle,
                    cycle_timestamp,
                );
            }
            MERSENNE_EXT4_FMA_ACCESS_ID => {
                mersenne_ext4_fma_impl::<_, _, _, _>(
                    state,
                    memory_source,
                    tracer,
                    mmu,
                    rs1_value,
                    trap,
                    proc_cycle,
                    cycle_timestamp,
                );
            }
            POSEIDON2_WITNESS_AND_COMPRESS_ACCESS_ID => {
                poseidon2_witness_and_compress::<_, _, _, _, _>(
                    memory_source,
                    non_determinism_source,
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
