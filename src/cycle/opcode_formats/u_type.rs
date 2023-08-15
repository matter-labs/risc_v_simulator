use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UTypeOpcode;

impl UTypeOpcode {
    #[must_use]
    #[inline(always)]
    pub const fn imm(src: u32) -> u32 {
        get_bits_and_shift_right(src, 12, 20, 0)
    }
}
