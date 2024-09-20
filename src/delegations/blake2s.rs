use crate::cycle::{state::NON_DETERMINISM_CSR, status_registers::TrapReason};
use blake2s_u32::*;

use super::*;

// blake2s binary interface is
// - 8xu32 words of the existing state
// - one word (word 12 of extended state), that encodes absorbed length
// - one word (word 14 of the extended state), that controls finalization
// - 16x32 words of the input data to mix in
// at the end we will overwrite first 8 words as the result

pub const BLAKE2S_ROUND_FUNCTION_ABI_NUM_MEM_ACCESSES: usize = 8 + 2 + 16;
pub const BLAKE2S_ACCESS_ID: u32 = NON_DETERMINISM_CSR + 1;

pub fn blake2s_round_function<
    M: MemorySource,
    TR: Tracer,
    MMU: MMUImplementation<M, TR>,
    const REDUCED_ROUNDS: bool,
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
        BLAKE2S_ROUND_FUNCTION_ABI_NUM_MEM_ACCESSES];
    let mut it = accesses.iter_mut();

    let mut extended_state = [0u32; 16];
    for low_offset in 0..8 {
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }

        *it.next().unwrap() = BatchAccessPartialData::Write {
            read_value: read_value,
            written_value: 0,
        };
        extended_state[low_offset as usize] = read_value;
    }

    for (low_offset, dst_index) in (8..10usize).zip([12, 14].into_iter()) {
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }

        *it.next().unwrap() = BatchAccessPartialData::Read { read_value };
        extended_state[dst_index] = read_value;
    }

    // init the rest of extended state
    for i in 0..4 {
        extended_state[i + 8] = IV[i];
    }
    // 12th element is known
    extended_state[13] = IV[5];
    // 14th element is known
    extended_state[15] = IV[7];

    let mut message_block = [0u32; 16];
    for (low_offset, dst) in (10..26usize).zip(message_block.iter_mut()) {
        let address: usize = mem_offset + low_offset * core::mem::size_of::<u32>();
        let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
        if trap.is_a_trap() {
            panic!("error in blake2s memory access");
        }

        *it.next().unwrap() = BatchAccessPartialData::Read { read_value };
        *dst = read_value;
    }

    let mut h = [
        extended_state[0],
        extended_state[1],
        extended_state[2],
        extended_state[3],
        extended_state[4],
        extended_state[5],
        extended_state[6],
        extended_state[7],
    ];

    if REDUCED_ROUNDS {
        round_function_reduced_rounds(&mut extended_state, &message_block);
    } else {
        round_function_full_rounds(&mut extended_state, &message_block);
    }

    for i in 0..8 {
        h[i] ^= extended_state[i];
        h[i] ^= extended_state[i + 8];
    }

    // write back
    for (low_offset, (access, value)) in accesses.iter_mut().zip(h.into_iter()).enumerate() {
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
        BLAKE2S_ACCESS_ID,
        (mem_offset >> 16) as u16,
        &accesses,
        proc_cycle,
        cycle_timestamp,
    );
}
