use crate::abstractions::memory::{AccessType, MemoryAccessTracer, MemorySource};
use crate::abstractions::MemoryImplementation;
use crate::cycle::state::Mode;
use crate::cycle::status_registers::{SATPRegister, TrapReason};
use crate::utils::*;

pub trait MMUImplementation<M: MemorySource, MTR: MemoryAccessTracer> {
    // we may need to consult memory source and tracer here
    type AuxilarySource;

    fn read_sapt(&mut self, mode: Mode, trap: &mut u32) -> u32;
    fn write_sapt(&mut self, value: u32, mode: Mode, trap: &mut u32);
    fn map_virtual_to_physical(
        &self,
        virt_address: u32,
        mode: Mode,
        access_type: AccessType,
        aux_source: &mut Self::AuxilarySource,
        trap: &mut u32,
    ) -> u64;
}

#[derive(Clone, Copy, Debug)]
pub struct PageTable {
    pub entries: [PageTableEntry; 1024],
}

impl PageTable {
    pub fn new() -> Self {
        Self {
            entries: std::array::from_fn(|_| PageTableEntry::new()),
        }
    }

    pub const fn len() -> usize {
        1024
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum EntryBit {
    Valid = 0,
    Read = 1,
    Write = 2,
    Execute = 3,
    UserMode = 4,
    Global = 5,
    Accessed = 6,
    Dirty = 7,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageTableEntry {
    pub value: u32,
}

#[must_use]
#[inline(always)]
pub const fn vpn_0(src: u32) -> u32 {
    get_bits_and_align_right(src, 12, 10)
}

#[must_use]
#[inline(always)]
pub const fn vpn_1(src: u32) -> u32 {
    get_bits_and_align_right(src, 22, 10)
}

#[must_use]
#[inline(always)]
pub const fn ppn_0(src: u32) -> u32 {
    get_bits_and_align_right(src, 10, 10)
}

#[must_use]
#[inline(always)]
pub const fn ppn_1(src: u32) -> u32 {
    get_bits_and_align_right(src, 20, 12)
}

#[must_use]
#[inline(always)]
pub const fn ppn(src: u32) -> u32 {
    get_bits_and_align_right(src, 10, 22)
}

impl PageTableEntry {
    #[must_use]
    #[inline(always)]
    pub const fn new() -> Self {
        Self { value: 0u32 }
    }

    #[must_use]
    #[inline(always)]
    pub const fn from_value(value: u32) -> Self {
        Self { value }
    }

    #[must_use]
    #[inline(always)]
    pub const fn test_bit(&self, bit: EntryBit) -> u32 {
        get_bit_right_aligned(self.value, bit as u32)
    }

    #[must_use]
    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.test_bit(EntryBit::Valid) != 0
            && !(self.test_bit(EntryBit::Write) != 0 && self.test_bit(EntryBit::Read) == 0)
    }

    #[must_use]
    #[inline(always)]
    pub fn is_valid_for_access_type_in_privilege(
        &self,
        access_type: AccessType,
        privilege: Mode,
    ) -> bool {
        let mut invalid = privilege == Mode::User && self.test_bit(EntryBit::UserMode) == 0;
        invalid = invalid || access_type == AccessType::Load && self.test_bit(EntryBit::Read) == 0;
        invalid =
            invalid || access_type == AccessType::Store && self.test_bit(EntryBit::Write) == 0;
        invalid = invalid
            || access_type == AccessType::Instruction && self.test_bit(EntryBit::Execute) == 0;

        !invalid
    }

    #[must_use]
    #[inline(always)]
    pub fn is_valid_for_ad_flags(&self, access_type: AccessType) -> bool {
        // When a virtual page is accessed and the A bit is clear, or is written and the D bit is clear, a
        // page-fault exception is raised

        let mut invalid = self.test_bit(EntryBit::Accessed) == 0;
        invalid =
            invalid || access_type == AccessType::Store && self.test_bit(EntryBit::Dirty) == 0;

        !invalid
    }
}

const SV32_PAGE_SIZE_LOG_2: u32 = 12;
const SV32_SUPERPAGE_SIZE_LOG_2: u32 = 22;
const SV32_LEVELS: usize = 2;
const SV32_PTE_SHIFT: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct NoMMU;

impl<M: MemorySource, MTR: MemoryAccessTracer> MMUImplementation<M, MTR> for NoMMU {
    type AuxilarySource = MemoryImplementation<M, MTR>;

    #[must_use]
    #[inline(always)]
    fn read_sapt(&mut self, _mode: Mode, _trap: &mut u32) -> u32 {
        0
    }

    #[inline(always)]
    fn write_sapt(&mut self, _value: u32, _mode: Mode, _trap: &mut u32) {}

    #[must_use]
    #[inline(always)]
    fn map_virtual_to_physical(
        &self,
        virt_address: u32,
        _mode: Mode,
        _access_type: AccessType,
        _aux_source: &mut Self::AuxilarySource,
        _trap: &mut u32,
    ) -> u64 {
        virt_address as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SimpleMMU {
    pub sapt: u32,
}

impl<M: MemorySource, MTR: MemoryAccessTracer> MMUImplementation<M, MTR> for SimpleMMU {
    type AuxilarySource = MemoryImplementation<M, MTR>;

    #[must_use]
    #[inline(always)]
    fn read_sapt(&mut self, _mode: Mode, _trap: &mut u32) -> u32 {
        self.sapt
    }

    #[inline(always)]
    fn write_sapt(&mut self, value: u32, mode: Mode, trap: &mut u32) {
        if mode != Mode::Machine {
            *trap = TrapReason::IllegalInstruction.as_register_value();
        } else {
            self.sapt = value;
        }
    }

    #[must_use]
    #[inline(always)]
    fn map_virtual_to_physical(
        &self,
        virt_address: u32,
        mode: Mode,
        access_type: AccessType,
        aux_source: &mut Self::AuxilarySource,
        trap: &mut u32,
    ) -> u64 {
        let should_translate = mode.as_register_value() < Mode::Machine.as_register_value()
            && SATPRegister::is_bare_aligned_bit(self.sapt) != 0;

        if should_translate == false {
            // no translation
            virt_address as u64
        } else {
            let mut a = SATPRegister::ppn(self.sapt) << SV32_PAGE_SIZE_LOG_2;
            let mut i: i32 = (SV32_LEVELS as i32) - 1;
            let vpns: [u32; 2] = [vpn_0(virt_address), vpn_1(virt_address)];

            let mut pte = PageTableEntry::new();

            for _j in 0..SV32_LEVELS {
                let pte_addr = a.wrapping_add(vpns[i as usize]);
                let pte_value = aux_source.read(pte_addr as u64, 4, access_type, trap);
                if *trap != 0 {
                    return 0;
                }
                pte = PageTableEntry::from_value(pte_value);
                if pte.is_valid() == false {
                    *trap = match access_type {
                        AccessType::Instruction => {
                            TrapReason::InstructionPageFault.as_register_value()
                        }
                        AccessType::Load => TrapReason::LoadPageFault.as_register_value(),
                        AccessType::Store => TrapReason::StoreOrAMOPageFault.as_register_value(),
                    };
                    return 0;
                }
                if pte.test_bit(EntryBit::Read) == 0 && pte.test_bit(EntryBit::Execute) == 0 {
                    // non-leaf PTE
                    i -= 1;
                    a = ppn(pte.value) << SV32_PAGE_SIZE_LOG_2;
                    continue;
                } else {
                    // leaf PTE!
                    break;
                }
            }

            if i < 0 {
                *trap = match access_type {
                    AccessType::Instruction => TrapReason::InstructionPageFault.as_register_value(),
                    AccessType::Load => TrapReason::LoadPageFault.as_register_value(),
                    AccessType::Store => TrapReason::StoreOrAMOPageFault.as_register_value(),
                };
                return 0;
            }

            if pte.is_valid_for_access_type_in_privilege(access_type, mode) == false {
                *trap = match access_type {
                    AccessType::Instruction => TrapReason::InstructionPageFault.as_register_value(),
                    AccessType::Load => TrapReason::LoadPageFault.as_register_value(),
                    AccessType::Store => TrapReason::StoreOrAMOPageFault.as_register_value(),
                };
                return 0;
            }

            let pte_value = pte.value << SV32_PTE_SHIFT;

            let ppns: [u32; 2] = [ppn_0(pte_value), ppn_1(pte_value)];

            // need to fit 34 bits here
            let physical_address_candidates: [u64; 2] = [
                (ppns[1] as u64) << 22
                    | (ppns[0] as u64) << 12
                    | get_bits_and_align_right(virt_address, 0, SV32_PAGE_SIZE_LOG_2) as u64,
                (ppns[1] as u64) << 22
                    | get_bits_and_align_right(virt_address, 0, SV32_SUPERPAGE_SIZE_LOG_2) as u64,
            ];

            if i > 0 && ppns[(i - 1) as usize] != 0 {
                // unaligned superpage
                *trap = match access_type {
                    AccessType::Instruction => TrapReason::InstructionPageFault.as_register_value(),
                    AccessType::Load => TrapReason::LoadPageFault.as_register_value(),
                    AccessType::Store => TrapReason::StoreOrAMOPageFault.as_register_value(),
                };
                return 0;
            }

            // check A and D flags
            if pte.is_valid_for_ad_flags(access_type) == false {
                *trap = match access_type {
                    AccessType::Instruction => TrapReason::InstructionPageFault.as_register_value(),
                    AccessType::Load => TrapReason::LoadPageFault.as_register_value(),
                    AccessType::Store => TrapReason::StoreOrAMOPageFault.as_register_value(),
                };
                return 0;
            }

            physical_address_candidates[i as usize]
        }
    }
}
