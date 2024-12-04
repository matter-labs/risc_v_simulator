use super::*;
use crate::cycle::state::NON_DETERMINISM_CSR;
use ::field::*;

// binary interface is
// - 12xu32 words of the extended state come from registers x10-x21

pub const MERSENNE_EXT4_FMA_ACCESS_ID: u32 = NON_DETERMINISM_CSR + 5;

pub fn mersenne_ext4_fma_impl<
    M: MemorySource,
    TR: Tracer<C>,
    MMU: MMUImplementation<M, TR, C>,
    C: MachineConfig,
>(
    state: &mut RiscV32State<C>,
    _memory_source: &mut M,
    tracer: &mut TR,
    _mmu: &mut MMU,
    _rs1_value: u32,
    _trap: &mut TrapReason,
    proc_cycle: u32,
    cycle_timestamp: u32,
) {
    let dst_c0_c0 = state.registers[10];
    let dst_c0_c1 = state.registers[11];
    let dst_c1_c0 = state.registers[12];
    let dst_c1_c1 = state.registers[13];

    let a_c0_c0 = state.registers[14];
    let a_c0_c1 = state.registers[15];
    let a_c1_c0 = state.registers[16];
    let a_c1_c1 = state.registers[17];

    let b_c0_c0 = state.registers[18];
    let b_c0_c1 = state.registers[19];
    let b_c1_c0 = state.registers[20];
    let b_c1_c1 = state.registers[21];

    let mut dst = Mersenne31Quartic {
        c0: Mersenne31Complex {
            c0: Mersenne31Field::from_nonreduced_u32(dst_c0_c0),
            c1: Mersenne31Field::from_nonreduced_u32(dst_c0_c1),
        },
        c1: Mersenne31Complex {
            c0: Mersenne31Field::from_nonreduced_u32(dst_c1_c0),
            c1: Mersenne31Field::from_nonreduced_u32(dst_c1_c1),
        },
    };

    let a = Mersenne31Quartic {
        c0: Mersenne31Complex {
            c0: Mersenne31Field::from_nonreduced_u32(a_c0_c0),
            c1: Mersenne31Field::from_nonreduced_u32(a_c0_c1),
        },
        c1: Mersenne31Complex {
            c0: Mersenne31Field::from_nonreduced_u32(a_c1_c0),
            c1: Mersenne31Field::from_nonreduced_u32(a_c1_c1),
        },
    };

    let b = Mersenne31Quartic {
        c0: Mersenne31Complex {
            c0: Mersenne31Field::from_nonreduced_u32(b_c0_c0),
            c1: Mersenne31Field::from_nonreduced_u32(b_c0_c1),
        },
        c1: Mersenne31Complex {
            c0: Mersenne31Field::from_nonreduced_u32(b_c1_c0),
            c1: Mersenne31Field::from_nonreduced_u32(b_c1_c1),
        },
    };

    let mut t = a;
    t.mul_assign(&b);
    dst.add_assign(&t);

    // NOTE: even though the circuit will not guarantee a reduction here,
    // it's easier for witness consistency
    state.registers[10] = dst.c0.c0.to_reduced_u32();
    state.registers[11] = dst.c0.c1.to_reduced_u32();
    state.registers[12] = dst.c1.c0.to_reduced_u32();
    state.registers[13] = dst.c1.c1.to_reduced_u32();

    // TODO: trace

    tracer.trace_batch_memory_access(
        BLAKE2_ROUND_FUNCTION_WITH_STATE_IN_REGISTERS_ACCESS_ID,
        0,
        &[],
        &[],
        proc_cycle,
        cycle_timestamp,
    );
}
