pub const INTERRUPT_MASK: u32 = 0x80000000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum InterruptReason {
    Reserved = 0,
    SupervisorSoftwareInterrupt = 1,
    MachineSoftwareInterrupt = 3,
    SupervisorTimerInterrupt = 5,
    MachineTimerInterrupt = 7,
    SupervisorExternalInterrupt = 9,
    MachineExternalInterrupt = 11,
}

impl InterruptReason {
    #[inline(always)]
    pub const fn as_register_value(self) -> u32 {
        (self as u32) | 0x80000000
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum TrapReason {
    InstructionAddressMisaligned = 0,
    InstructionAccessFault = 1,
    IllegalInstruction = 2,
    Breakpoint = 3,
    LoadAddressMisaligned = 4,
    LoadAccessFault = 5,
    StoreOrAMOAddressMisaligned = 6,
    StoreOrAMOAccessFault = 7,
    EnvironmentCallFromUMode = 8,
    EnvironmentCallFromSMode = 9,
    EnvironmentCallFromMMode = 11,
    InstructionPageFault = 12,
    LoadPageFault = 13,
    StoreOrAMOPageFault = 15,
    NoTrap = 0xff,
}

impl TrapReason {
    #[must_use]
    #[inline(always)]
    pub const fn is_a_trap(&self) -> bool {
        self.as_register_value() != Self::NoTrap.as_register_value()
    }

    #[must_use]
    #[inline(always)]
    pub const fn as_register_value(self) -> u32 {
        self as u32
    }

    #[must_use]
    #[inline(always)]
    pub fn from_register_value(value: u32) -> Self {
        match value {
            a if a == TrapReason::InstructionAddressMisaligned as u32 => {
                TrapReason::InstructionAddressMisaligned
            }
            a if a == TrapReason::InstructionAccessFault as u32 => {
                TrapReason::InstructionAccessFault
            }
            a if a == TrapReason::IllegalInstruction as u32 => TrapReason::IllegalInstruction,
            a if a == TrapReason::Breakpoint as u32 => TrapReason::Breakpoint,
            a if a == TrapReason::LoadAddressMisaligned as u32 => TrapReason::LoadAddressMisaligned,
            a if a == TrapReason::LoadAccessFault as u32 => TrapReason::LoadAccessFault,
            a if a == TrapReason::StoreOrAMOAddressMisaligned as u32 => {
                TrapReason::StoreOrAMOAddressMisaligned
            }
            a if a == TrapReason::StoreOrAMOAccessFault as u32 => TrapReason::StoreOrAMOAccessFault,
            a if a == TrapReason::EnvironmentCallFromUMode as u32 => {
                TrapReason::EnvironmentCallFromUMode
            }
            a if a == TrapReason::EnvironmentCallFromSMode as u32 => {
                TrapReason::EnvironmentCallFromSMode
            }
            a if a == TrapReason::EnvironmentCallFromMMode as u32 => {
                TrapReason::EnvironmentCallFromMMode
            }
            a if a == TrapReason::InstructionPageFault as u32 => TrapReason::InstructionPageFault,
            a if a == TrapReason::LoadPageFault as u32 => TrapReason::LoadPageFault,
            a if a == TrapReason::StoreOrAMOPageFault as u32 => TrapReason::StoreOrAMOPageFault,
            _ => {
                panic!("unknown trap reason")
            }
        }
    }
}
