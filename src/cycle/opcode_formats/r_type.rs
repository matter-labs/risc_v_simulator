use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RTypeOpcode;

impl RTypeOpcode {
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
    pub const fn funct7(src: u32) -> u32 {
        get_bits_and_align_right(src, 25, 7)
    }
}
