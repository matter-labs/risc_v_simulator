use crate::utils::*;

#[must_use]
#[inline(always)]
pub const fn get_opcode(src: u32) -> u32 {
    src & 0b01111111 // opcode is always lowest 7 bits
}

#[must_use]
#[inline(always)]
pub const fn get_rd(src: u32) -> u32 {
    (src >> 7) & 0b00011111
}

pub mod b_type;
pub mod i_type;
pub mod j_type;
pub mod r_type;
pub mod s_type;
pub mod u_type;

pub use self::b_type::BTypeOpcode;
pub use self::i_type::ITypeOpcode;
pub use self::j_type::JTypeOpcode;
pub use self::r_type::RTypeOpcode;
pub use self::s_type::STypeOpcode;
pub use self::u_type::UTypeOpcode;
