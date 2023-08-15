use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SATPRegister;

impl SATPRegister {
    #[must_use]
    #[inline(always)]
    pub const fn is_bare_aligned_bit(src: u32) -> u32 {
        get_bits_and_align_right(src, 31, 1)
    }

    #[must_use]
    #[inline(always)]
    pub const fn ppn(src: u32) -> u32 {
        get_bits_and_align_right(src, 0, 22)
    }
}
