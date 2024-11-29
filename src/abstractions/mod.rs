use self::memory::MemorySource;
use crate::abstractions::memory::AccessType;
use crate::cycle::status_registers::TrapReason;
use crate::cycle::MachineConfig;
use std::hint::unreachable_unchecked;
use tracer::Tracer;

pub mod csr_processor;
pub mod memory;
pub mod non_determinism;
pub mod tracer;

#[must_use]
#[inline(always)]
pub fn mem_read<
    M: MemorySource,
    TR: Tracer<C>,
    C: MachineConfig,
    const SUPPORT_LOAD_LESS_THAN_WORD: bool,
>(
    memory_source: &mut M,
    tracer: &mut TR,
    phys_address: u64,
    num_bytes: u32,
    access_type: AccessType,
    proc_cycle: u32,
    cycle_timestamp: u32,
    trap: &mut TrapReason,
) -> u32 {
    assert!(access_type == AccessType::Instruction || access_type == AccessType::MemLoad);

    let unalignment = phys_address & 3;
    let aligned_address = phys_address & !3;
    if SUPPORT_LOAD_LESS_THAN_WORD {
        let value = match (unalignment, num_bytes) {
            (0, 4) | (0, 2) | (2, 2) | (0, 1) | (1, 1) | (2, 1) | (3, 1) => {
                let value = memory_source.get(aligned_address, access_type, trap);
                if access_type == AccessType::Instruction {
                    tracer.trace_opcode_read(aligned_address, value, proc_cycle, cycle_timestamp);
                } else {
                    tracer.trace_ram_read(aligned_address, value, proc_cycle, cycle_timestamp);
                }

                let unalignment_bits = unalignment * 8;
                let value = value >> unalignment_bits;
                value
            }
            _ => {
                *trap = TrapReason::LoadAddressMisaligned;

                0u32 // formally
            }
        };

        let mask = match num_bytes {
            1 => 0x000000ff,
            2 => 0x0000ffff,
            4 => 0xffffffff,
            _ => unsafe { unreachable_unchecked() },
        };
        let value = value & mask;

        value
    } else {
        let value = match (unalignment, num_bytes) {
            (0, 4) => {
                let value = memory_source.get(aligned_address, access_type, trap);
                if access_type == AccessType::Instruction {
                    tracer.trace_opcode_read(aligned_address, value, proc_cycle, cycle_timestamp);
                } else {
                    tracer.trace_ram_read(aligned_address, value, proc_cycle, cycle_timestamp);
                }

                value
            }
            _ => {
                *trap = TrapReason::LoadAddressMisaligned;

                0u32 // formally
            }
        };

        value
    }
}

#[inline(always)]
pub fn mem_write<
    M: MemorySource,
    TR: Tracer<C>,
    C: MachineConfig,
    const SUPPORT_LOAD_LESS_THAN_WORD: bool,
>(
    memory_source: &mut M,
    tracer: &mut TR,
    phys_address: u64,
    value: u32,
    num_bytes: u32,
    proc_cycle: u32,
    cycle_timestamp: u32,
    trap: &mut TrapReason,
) {
    let unalignment = phys_address & 3;
    let aligned_address = phys_address & !3;
    if SUPPORT_LOAD_LESS_THAN_WORD {
        match (unalignment, num_bytes) {
            a @ (0, 4)
            | a @ (0, 2)
            | a @ (2, 2)
            | a @ (0, 1)
            | a @ (1, 1)
            | a @ (2, 1)
            | a @ (3, 1) => {
                let (unalignment, num_bytes) = a;

                // we need to load old value - just for easier comparison of simulator/in_circuit implementation
                let old_value = memory_source.get(aligned_address, AccessType::MemLoad, trap);
                if trap.is_a_trap() {
                    return;
                }

                let value_mask = match num_bytes {
                    1 => 0x000000ffu32,
                    2 => 0x0000ffffu32,
                    4 => 0xffffffffu32,
                    _ => unsafe { unreachable_unchecked() },
                };

                let mask_old = match (unalignment, num_bytes) {
                    (0, 1) => 0xffffff00u32,
                    (0, 2) => 0xffff0000u32,
                    (0, 4) => 0x00000000u32,
                    (1, 1) => 0xffff00ffu32,
                    (1, 2) => 0xffff00ffu32,
                    (2, 1) => 0xff00ffffu32,
                    (2, 2) => 0x0000ffffu32,
                    (3, 1) => 0x00ffffffu32,
                    _ => unsafe { unreachable_unchecked() },
                };

                let new_value =
                    ((value & value_mask) << (unalignment * 8)) | (old_value & mask_old);

                memory_source.set(aligned_address, new_value, AccessType::MemStore, trap);
                tracer.trace_ram_read_write(
                    aligned_address,
                    old_value,
                    new_value,
                    proc_cycle,
                    cycle_timestamp,
                );
                if trap.is_a_trap() {
                    return;
                }
            }
            _ => {
                *trap = TrapReason::StoreOrAMOAddressMisaligned;
            }
        }
    } else {
        match (unalignment, num_bytes) {
            _a @ (0, 4) => {
                // we need to load old value - just for easier comparison of simulator/in_circuit implementation
                let old_value = memory_source.get(aligned_address, AccessType::MemLoad, trap);
                if trap.is_a_trap() {
                    return;
                }

                let new_value = value;

                memory_source.set(aligned_address, new_value, AccessType::MemStore, trap);
                tracer.trace_ram_read_write(
                    aligned_address,
                    old_value,
                    new_value,
                    proc_cycle,
                    cycle_timestamp,
                );
                if trap.is_a_trap() {
                    return;
                }
            }
            _ => {
                *trap = TrapReason::StoreOrAMOAddressMisaligned;
            }
        }
    }
}
