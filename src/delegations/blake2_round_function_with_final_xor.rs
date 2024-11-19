use crate::cycle::{state::NON_DETERMINISM_CSR, status_registers::TrapReason};
use blake2s_u32::{mixing_function, IV, SIGMAS};

use super::*;

// blake2 round function binary interface is
// - 8xu32 initial state to be output of the final round, and input of the first round
// - 16xu32 words of the extended state scratch space. Control registers will be taken from there
// - 16x32 words of the input data to mix in
// - one u32 bitmask, that will determine a permutation
// - one boolean flag that will tell to xor-out into first 8 words

pub const BLAKE2_ROUND_FUNCTION_ABI_NUM_MEM_ACCESSES: usize = 8 + 16 + 16 + 1 + 1;
pub const BLAKE2_ROUND_FUNCTION_WITH_XOR_ACCESS_ID: u32 = NON_DETERMINISM_CSR + 3;

pub fn blake2_round_function_with_xor<
    M: MemorySource,
    TR: Tracer<C>,
    MMU: MMUImplementation<M, TR, C>,
    C: MachineConfig,
>(
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

    // we perform batch accesses
    let mut accesses = [BatchAccessPartialData::Read { read_value: 0 };
        BLAKE2_ROUND_FUNCTION_ABI_NUM_MEM_ACCESSES];
    let mut it = accesses.iter_mut();

    let mut initial_state = [0u32; 8];
    for (low_offset, dst) in (0..8).zip(initial_state.iter_mut()) {
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }

        *it.next().unwrap() = BatchAccessPartialData::Write {
            read_value: read_value,
            written_value: 0,
        };
        *dst = read_value;
    }

    let mut extended_state = [0u32; 16];
    for (low_offset, dst) in (8..24usize).zip(extended_state.iter_mut()) {
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }

        *it.next().unwrap() = BatchAccessPartialData::Write {
            read_value: read_value,
            written_value: 0,
        };
        *dst = read_value;
    }

    let mut message_block = [0u32; 16];
    for (low_offset, dst) in (24..40usize).zip(message_block.iter_mut()) {
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }

        *it.next().unwrap() = BatchAccessPartialData::Read { read_value };
        *dst = read_value;
    }

    // bitmask controlling the permutation
    let address: usize = mem_offset + 40 * core::mem::size_of::<u32>();
    let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
    if trap.is_a_trap() {
        panic!("error in blake2s memory access");
    }
    *it.next().unwrap() = BatchAccessPartialData::Read { read_value };
    let permutation_bitmask = read_value;
    assert!(permutation_bitmask.is_power_of_two());
    // bit to control final output
    let address: usize = mem_offset + 41 * core::mem::size_of::<u32>();
    let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
    if trap.is_a_trap() {
        panic!("error in blake2s memory access");
    }
    *it.next().unwrap() = BatchAccessPartialData::Read { read_value };
    let flush_bit = read_value;
    assert!(flush_bit == 0 || flush_bit == 1);

    let permutation_index = permutation_bitmask.trailing_zeros() as usize;

    if permutation_index == 0 {
        // we take values from the initial state, and put them into 0..8 range
        extended_state[0] = initial_state[0];
        extended_state[1] = initial_state[1];
        extended_state[2] = initial_state[2];
        extended_state[3] = initial_state[3];
        extended_state[4] = initial_state[4];
        extended_state[5] = initial_state[5];
        extended_state[6] = initial_state[6];
        extended_state[7] = initial_state[7];

        // and then overwrite elements 8-11, 13, 15 to initial values
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

    if flush_bit == 1 {
        assert!(permutation_index == 6 || permutation_index == 9, "expected to support only 7 or 10 round invocaitons, but got a request to flush after {} rounds", permutation_index + 1);
        // xor it and write into first 8 elements
        for i in 0..8 {
            initial_state[i] ^= extended_state[i];
            initial_state[i] ^= extended_state[i + 8];
        }
    }

    // write back
    for ((access, value), low_offset) in
        accesses.iter_mut().zip(initial_state.into_iter()).zip(0..8)
    {
        let BatchAccessPartialData::Write {
            read_value: _,
            written_value,
        } = access
        else {
            unreachable!()
        };

        *written_value = value;
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        memory_source.set(address as u64, value, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }
    }

    // write back
    for ((access, value), low_offset) in accesses[8..]
        .iter_mut()
        .zip(extended_state.into_iter())
        .zip(8..24)
    {
        let BatchAccessPartialData::Write {
            read_value: _,
            written_value,
        } = access
        else {
            unreachable!()
        };

        *written_value = value;
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        memory_source.set(address as u64, value, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }
    }

    tracer.trace_batch_memory_access(
        BLAKE2_ROUND_FUNCTION_WITH_XOR_ACCESS_ID,
        (mem_offset >> 16) as u16,
        &accesses,
        proc_cycle,
        cycle_timestamp,
    );
}
