use super::*;
use crate::cycle::state::NON_DETERMINISM_CSR;

const NUM_MEMORY_ACCESSES: usize = 8 + 1;
pub const POSEIDON2_WITNESS_AND_COMPRESS_ACCESS_ID: u32 = NON_DETERMINISM_CSR + 6;

pub fn poseidon2_witness_and_compress<
    M: MemorySource,
    TR: Tracer<C>,
    ND: NonDeterminismCSRSource<M>,
    MMU: MMUImplementation<M, TR, C>,
    C: MachineConfig,
>(
    memory_source: &mut M,
    non_determinism_source: &mut ND,
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
    let mut accesses = [BatchAccessPartialData::Read { read_value: 0 }; NUM_MEMORY_ACCESSES];
    let mut it = accesses.iter_mut();

    let mut input = [0u32; 8];
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
        input[low_offset as usize] = read_value;
    }

    // bitmask controlling the permutation
    let address: usize = mem_offset + 8 * core::mem::size_of::<u32>();
    let read_value = memory_source.get(address as u64, AccessType::RegWrite, trap);
    if trap.is_a_trap() {
        panic!("error in blake2s memory access");
    }

    *it.next().unwrap() = BatchAccessPartialData::Read { read_value };
    let input_is_right = read_value;
    assert!(input_is_right == 0 || input_is_right == 1);
    let input_is_right = input_is_right == 1;

    let witness: [u32; 8] = std::array::from_fn(|_| non_determinism_source.read());

    let mut output = input;
    for dst in output.iter_mut() {
        *dst += 1;
    }

    // write back
    for (low_offset, (access, value)) in accesses.iter_mut().zip(output.into_iter()).enumerate() {
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
            panic!("error in poseidon2 memory access");
        }
    }

    tracer.trace_batch_memory_access(
        POSEIDON2_WITNESS_AND_COMPRESS_ACCESS_ID,
        (mem_offset >> 16) as u16,
        &accesses,
        &witness,
        proc_cycle,
        cycle_timestamp,
    );
}
