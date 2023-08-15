use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BTypeOpcode;

impl BTypeOpcode {
    #[must_use]
    #[inline(always)]
    pub const fn rs1(src: u32) -> u32 {
        get_bits_and_align_right(src, 15, 5)
    }

    #[must_use]
    #[inline(always)]
    pub const fn rs2(src: u32) -> u32 {
        get_bits_and_align_right(src, 20, 5)
    }

    #[must_use]
    #[inline(always)]
    pub const fn funct3(src: u32) -> u32 {
        get_bits_and_align_right(src, 12, 3)
    }

    #[must_use]
    #[inline(always)]
    pub const fn imm(src: u32) -> u32 {
        get_bits_and_shift_right(src, 8, 4, 8 - 1)
            | get_bits_and_shift_right(src, 25, 6, 25 - 5)
            | get_bits_and_shift_left(src, 7, 1, 11 - 7)
            | get_bits_and_shift_right(src, 31, 1, 31 - 12)
    }
}
