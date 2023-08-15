use std::hint::unreachable_unchecked;

use self::memory::{MemoryAccessTracer, MemorySource};
use crate::abstractions::memory::AccessType;
use crate::abstractions::memory::Timestamp;
use crate::cycle::status_registers::TrapReason;

pub mod memory;

pub struct MemoryImplementation<M: MemorySource, MTR: MemoryAccessTracer> {
    pub memory_source: M,
    pub tracer: MTR,
    pub timestamp: MTR::Timestamp,
}

impl<M: MemorySource, MTR: MemoryAccessTracer> MemoryImplementation<M, MTR> {
    #[must_use]
    #[inline(always)]
    pub fn read(
        &mut self,
        phys_address: u64,
        num_bytes: u32,
        access_type: AccessType,
        trap: &mut u32,
    ) -> u32 {
        let unalignment = phys_address & 3;
        let aligned_address = phys_address & !3;
        let value = match (unalignment, num_bytes) {
            (0, 4) | (0, 2) | (2, 2) | (0, 1) | (1, 1) | (2, 1) | (3, 1) => {
                let value = self.memory_source.get(aligned_address, access_type, trap);
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.tracer.add_query(
                    aligned_address,
                    value,
                    false,
                    access_type,
                    current_ts,
                    *trap,
                );
                let unalignment_bits = unalignment * 8;
                let value = value >> unalignment_bits;

                value
            }
            _ => {
                *trap = TrapReason::LoadAddressMisaligned.as_register_value();

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
    }

    #[inline(always)]
    pub fn write(
        &mut self,
        phys_address: u64,
        value: u32,
        num_bytes: u32,
        access_type: AccessType,
        trap: &mut u32,
    ) {
        let unalignment = phys_address & 3;
        let aligned_address = phys_address & !3;
        match (unalignment, num_bytes) {
            a @ (0, 4)
            | a @ (0, 2)
            | a @ (2, 2)
            | a @ (0, 1)
            | a @ (1, 1)
            | a @ (2, 1)
            | a @ (3, 1) => {
                let (unalignment, num_bytes) = a;

                // we need to load old value
                let old_value = self.memory_source.get(aligned_address, access_type, trap);
                if *trap != 0 {
                    return;
                }
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.tracer.add_query(
                    aligned_address,
                    old_value,
                    false,
                    access_type,
                    current_ts,
                    *trap,
                );

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

                // println!("Writing {} bytes of value 0x{:08x} with unalignment of {} over 0x{:08x} to get 0x{:08x}",
                //     num_bytes,
                //     value,
                //     unalignment,
                //     old_value,
                //     new_value,
                // );

                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.memory_source
                    .set(aligned_address, new_value, access_type, trap);
                if *trap != 0 {
                    return;
                }
                self.tracer.add_query(
                    aligned_address,
                    new_value,
                    true,
                    access_type,
                    current_ts,
                    *trap,
                );
            }

            _ => {
                *trap = TrapReason::StoreOrAMOAddressMisaligned.as_register_value();
            }
        };
    }

    pub fn notify_new_cycle(&mut self) {
        self.timestamp = self.timestamp.new_cycle_timestamp();
    }
}
