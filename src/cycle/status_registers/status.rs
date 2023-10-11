use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MStatusRegister;

impl MStatusRegister {
    #[must_use]
    #[inline(always)]
    pub const fn sie(src: u32) -> u32 {
        get_bit_unaligned(src, 1)
    }

    #[must_use]
    #[inline(always)]
    pub const fn mie(src: u32) -> u32 {
        get_bit_unaligned(src, 3)
    }

    #[must_use]
    #[inline(always)]
    pub const fn mie_aligned_bit(src: u32) -> u32 {
        get_bits_and_align_right(src, 3, 1)
    }

    #[inline(always)]
    pub const fn set_mie(dst: &mut u32) {
        set_bit(dst, 3)
    }

    #[inline(always)]
    pub const fn clear_mie(dst: &mut u32) {
        clear_bit(dst, 3)
    }

    #[inline(always)]
    pub const fn set_mie_to_value(dst: &mut u32, value: u32) {
        Self::clear_mie(dst);
        set_bits_to_value(dst, 3, value);
    }

    #[must_use]
    #[inline(always)]
    pub const fn spie(src: u32) -> u32 {
        get_bit_unaligned(src, 5)
    }

    #[must_use]
    #[inline(always)]
    pub const fn ube(src: u32) -> u32 {
        get_bit_unaligned(src, 6)
    }

    #[must_use]
    #[inline(always)]
    pub const fn mpie(src: u32) -> u32 {
        get_bit_unaligned(src, 7)
    }

    #[inline(always)]
    pub const fn set_mpie(dst: &mut u32) {
        set_bit(dst, 7)
    }

    #[inline(always)]
    pub const fn clear_mpie(dst: &mut u32) {
        clear_bit(dst, 7)
    }

    #[inline(always)]
    pub const fn set_mpie_to_value(dst: &mut u32, value: u32) {
        Self::clear_mpie(dst);
        set_bits_to_value(dst, 7, value);
    }

    #[inline(always)]
    pub const fn set_mpp_to_machine(dst: &mut u32) {
        Self::clear_mpp(dst);
        set_bits_to_value(dst, 11, 3);
    }

    #[must_use]
    #[inline(always)]
    pub const fn mpie_aligned_bit(src: u32) -> u32 {
        get_bits_and_align_right(src, 7, 1)
    }

    #[must_use]
    #[inline(always)]
    pub const fn spp(src: u32) -> u32 {
        get_bit_unaligned(src, 8)
    }

    #[must_use]
    #[inline(always)]
    pub const fn vs(src: u32) -> u32 {
        get_bits_and_align_right(src, 9, 2)
    }

    #[must_use]
    #[inline(always)]
    pub const fn mpp(src: u32) -> u32 {
        get_bits_and_align_right(src, 11, 2)
    }

    #[inline(always)]
    pub const fn clear_mpp(dst: &mut u32) {
        clear_bits(dst, 11, 2)
    }

    #[must_use]
    #[inline(always)]
    pub const fn fs(src: u32) -> u32 {
        get_bits_and_align_right(src, 13, 2)
    }

    #[must_use]
    #[inline(always)]
    pub const fn cs(src: u32) -> u32 {
        get_bits_and_align_right(src, 15, 2)
    }

    #[must_use]
    #[inline(always)]
    pub const fn mprv(src: u32) -> u32 {
        get_bit_unaligned(src, 17)
    }

    #[inline(always)]
    pub const fn clear_mprv(dst: &mut u32) {
        clear_bit(dst, 17);
    }

    #[must_use]
    #[inline(always)]
    pub const fn mprv_aligned_bit(src: u32) -> u32 {
        get_bits_and_align_right(src, 17, 11)
    }

    #[must_use]
    #[inline(always)]
    pub const fn sum(src: u32) -> u32 {
        get_bit_unaligned(src, 18)
    }

    #[must_use]
    #[inline(always)]
    pub const fn mxr(src: u32) -> u32 {
        get_bit_unaligned(src, 19)
    }

    #[must_use]
    #[inline(always)]
    pub const fn tvm(src: u32) -> u32 {
        get_bit_unaligned(src, 20)
    }

    #[must_use]
    #[inline(always)]
    pub const fn tw(src: u32) -> u32 {
        get_bit_unaligned(src, 21)
    }

    #[must_use]
    #[inline(always)]
    pub const fn tsr(src: u32) -> u32 {
        get_bit_unaligned(src, 22)
    }

    #[must_use]
    #[inline(always)]
    pub const fn sd(src: u32) -> u32 {
        get_bit_unaligned(src, 31)
    }
}
