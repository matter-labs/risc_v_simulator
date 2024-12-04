use crate::cycle::{state::NON_DETERMINISM_CSR, status_registers::TrapReason};
use blake2s_u32::{mixing_function, IV, SIGMAS};

use super::*;

// blake2 round function binary interface is
// - 16xu32 words of the extended state come from registers x10-x25
// - one u32 bitmask, that will determine a permutation from x26
// - 16xu32 words of the input data to mix in
// at the end we will overwrite first 16 words as the result

pub const BLAKE2_ROUND_FUNCTION_ABI_NUM_MEM_ACCESSES: usize = 16;
pub const BLAKE2_ROUND_FUNCTION_WITH_STATE_IN_REGISTERS_ACCESS_ID: u32 = NON_DETERMINISM_CSR + 4;

pub fn blake2_round_function_with_state_in_registers<
    M: MemorySource,
    TR: Tracer<C>,
    MMU: MMUImplementation<M, TR, C>,
    C: MachineConfig,
>(
    state: &mut RiscV32State<C>,
    memory_source: &mut M,
    tracer: &mut TR,
    _mmu: &mut MMU,
    rs1_value: u32,
    trap: &mut TrapReason,
    proc_cycle: u32,
    cycle_timestamp: u32,
) {
    // we consider high bits as the offset
    assert_eq!(rs1_value as u16, 0, "unaligned");
    let mem_offset = (rs1_value & 0xffff0000) as usize;

    // read registers first

    // we perform batch accesses
    let mut accesses = [BatchAccessPartialData::Read { read_value: 0 };
        BLAKE2_ROUND_FUNCTION_ABI_NUM_MEM_ACCESSES];
    let mut it = accesses.iter_mut();

    let mut extended_state = [0u32; 16];
    for low_offset in 0..16 {
        extended_state[low_offset] = state.registers[10 + low_offset];

        // TODO: trace
    }
    let permutation_bitmask = state.registers[10 + 16];
    // TODO: trace

    let mut message_block = [0u32; 16];
    for low_offset in 0..16 {
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }

        *it.next().unwrap() = BatchAccessPartialData::Read { read_value };
        message_block[low_offset as usize] = read_value;
    }
    assert!(permutation_bitmask.is_power_of_two());
    let permutation_index = permutation_bitmask.trailing_zeros() as usize;

    if permutation_index == 0 {
        // overwrite elements 8-11, 13, 15
        extended_state[8] = IV[0];
        extended_state[9] = IV[1];
        extended_state[10] = IV[2];
        extended_state[11] = IV[3];
        extended_state[13] = IV[5];
        extended_state[15] = IV[7];
    }

    // we expect that caller will supply a bitmask, encoding the corresponding choice of sigmas
    let sigma = &SIGMAS[permutation_index];
    mixing_function(&mut extended_state, &message_block, sigma);

    // write back
    for low_offset in 0..16 {
        state.registers[10 + low_offset] = extended_state[low_offset];

        // TODO: trace
    }

    tracer.trace_batch_memory_access(
        BLAKE2_ROUND_FUNCTION_WITH_STATE_IN_REGISTERS_ACCESS_ID,
        (mem_offset >> 16) as u16,
        &accesses,
        &[],
        proc_cycle,
        cycle_timestamp,
    );
}