use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JTypeOpcode;

impl JTypeOpcode {
    #[must_use]
    #[inline(always)]
    pub const fn imm(src: u32) -> u32 {
        get_bits_and_shift_right(src, 21, 10, 21 - 1)
            | get_bits_and_shift_right(src, 20, 1, 20 - 11)
            | get_bits_and_shift_right(src, 12, 8, 0)
            | get_bits_and_shift_right(src, 31, 1, 31 - 20)
    }
}
