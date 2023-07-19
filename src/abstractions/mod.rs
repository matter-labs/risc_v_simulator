use std::hint::unreachable_unchecked;

use self::memory::{MemorySource, MemoryAccessTracer};
use crate::abstractions::memory::Timestamp;
use crate::abstractions::memory::AccessType;

pub mod memory;

pub struct MemoryImplementation<M: MemorySource, MTR: MemoryAccessTracer> {
    pub memory_source: M,
    pub tracer: MTR,
    pub timestamp: MTR::Timestamp,
}

impl<M: MemorySource, MTR: MemoryAccessTracer> MemoryImplementation<M, MTR> {
    #[must_use]
    #[inline(always)]
    pub fn read(&mut self, phys_address: u64, num_bytes: u32, access_type: AccessType, trap: &mut u32) -> u32 {
        let unalignment = phys_address & 3;
        let aligned_address = phys_address & !3;
        let value = match (unalignment, num_bytes) {
            a @ (1, 4) |
            a @ (2, 4) |
            a @ (3, 4) |
            a @ (3, 2) => {
                let (unalignment, _num_bytes) = a;
                // we need two memory accesses :(
                let value_low = self.memory_source.get(aligned_address, access_type, trap);
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.tracer.add_query(aligned_address, value_low, false, access_type, current_ts, *trap);
                if *trap != 0 {
                    return 0;
                }
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                let value_high = self.memory_source.get(aligned_address.wrapping_add(4), access_type, trap);
                self.tracer.add_query(aligned_address.wrapping_add(4), value_high, false, access_type, current_ts, *trap);
                if *trap != 0 {
                    return 0;
                }

                let concatenated = (value_low as u64) | ((value_high as u64) << 32);
                let value = (concatenated >> (unalignment * 8)) as u32;

                // println!("Reading with unalignment of {} from 0x{:08x} and 0x{:08x} to get 0x{:08x}",
                //     unalignment,
                //     value_low,
                //     value_high,
                //     value,
                // );

                value
            },
            (_, _) => {
                let value = self.memory_source.get(aligned_address, access_type, trap);
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.tracer.add_query(aligned_address, value, false, access_type, current_ts, *trap);

                value
            }
        };

        let mask = match num_bytes {
            1 => 0x000000ff,
            2 => 0x0000ffff,
            4 => 0xffffffff,
            _ => unsafe {unreachable_unchecked()},
        };
        let value = value & mask;

        value
    }

    #[inline(always)]
    pub fn write(&mut self, phys_address: u64, value: u32, num_bytes: u32, access_type: AccessType, trap: &mut u32) {
        let unalignment = phys_address & 3;
        let aligned_address = phys_address & !3;
        match (unalignment, num_bytes) {
            a @ (1, 4) |
            a @ (2, 4) |
            a @ (3, 4) |
            a @ (3, 2) => {
                let (unalignment, num_bytes) = a;
                // we need two memory accesses :(
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                let value_low = self.memory_source.get(aligned_address, access_type, trap);
                self.tracer.add_query(aligned_address, value_low, false, access_type, current_ts, *trap);
                if *trap != 0 {
                    return;
                }
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                let value_high = self.memory_source.get(aligned_address.wrapping_add(4), access_type, trap);
                self.tracer.add_query(aligned_address.wrapping_add(4), value_high, false, access_type, current_ts, *trap);
                if *trap != 0 {
                    return;
                }

                let value_mask = match num_bytes {
                    2 => 0x0000ffffu32,
                    4 => 0xffffffffu32,
                    _ => unsafe {unreachable_unchecked()},
                };

                let (mask_existing_low, mask_existing_high) = match (unalignment, num_bytes) {
                    (1, 4) => (0x000000ffu32, 0xffffff00u32),
                    (2, 4) => (0x0000ffffu32, 0xffff0000u32),
                    (3, 4) => (0x00ffffffu32, 0xff000000u32),
                    (3, 2) => (0x00ffffffu32, 0xffffff00u32),
                    _ => unsafe {unreachable_unchecked()},
                };

                let masked_value = value & value_mask;
                let new_low = (value_low & mask_existing_low) | (masked_value << (unalignment * 8));
                let new_high = (value_high & mask_existing_high) | (masked_value >> (32 - unalignment * 8));

                // println!("Writing {} bytes of value 0x{:08x} with unalignment of {} over 0x{:08x} and 0x{:08x} to get 0x{:08x} and 0x{:08x}",
                //     num_bytes,
                //     value,
                //     unalignment,
                //     value_low,
                //     value_high,
                //     new_low,
                //     new_high,
                // );
                
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.memory_source.set(aligned_address, new_low, access_type, trap);
                if *trap != 0 {
                    return;
                }
                self.tracer.add_query(aligned_address, new_low, true, access_type, current_ts, *trap);

                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.memory_source.set(aligned_address.wrapping_add(4), new_high, access_type, trap);
                if *trap != 0 {
                    return;
                }
                self.tracer.add_query(aligned_address.wrapping_add(4), new_high, true, access_type, current_ts, *trap);
            },
            (unalignment, num_bytes) => {
                // we need only low value
                let old_value = self.memory_source.get(aligned_address, access_type, trap);
                if *trap != 0 {
                    return;
                }
                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.tracer.add_query(aligned_address, old_value, false, access_type, current_ts, *trap);

                let value_mask = match num_bytes {
                    1 => 0x000000ffu32,
                    2 => 0x0000ffffu32,
                    4 => 0xffffffffu32,
                    _ => unsafe {unreachable_unchecked()},
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
                    _ => unsafe {unreachable_unchecked()},
                };

                let new_value = ((value & value_mask) << (unalignment * 8)) | (old_value & mask_old);

                // println!("Writing {} bytes of value 0x{:08x} with unalignment of {} over 0x{:08x} to get 0x{:08x}",
                //     num_bytes,
                //     value,
                //     unalignment,
                //     old_value,
                //     new_value,
                // );

                let current_ts = self.timestamp;
                self.timestamp.update_after_subaccess();
                self.memory_source.set(aligned_address, new_value, access_type, trap);
                if *trap != 0 {
                    return;
                }
                self.tracer.add_query(aligned_address, new_value, true, access_type, current_ts, *trap);
            }
        };
    }

    pub fn notify_new_cycle(&mut self) {
        self.timestamp = self.timestamp.new_cycle_timestamp();
    }
}