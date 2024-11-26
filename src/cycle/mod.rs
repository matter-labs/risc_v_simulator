use std::hash::Hash;

pub mod opcode_formats;
pub mod state;
pub mod status_registers;

pub trait MachineConfig: 'static + Clone + Copy + Hash + std::fmt::Debug + PartialEq + Eq {
    const SUPPORT_SIGNED_MUL: bool;
    const SUPPORT_SIGNED_DIV: bool;
    const SUPPORT_SIGNED_LOAD: bool;
    const SUPPORT_LOAD_LESS_THAN_WORD: bool;
    const SUPPORT_SRA: bool;
    const SUPPORT_ROT: bool;
    const SUPPORT_MOPS: bool;
    const HANDLE_EXCEPTIONS: bool;
    const SUPPORT_STANDARD_CSRS: bool;
    const SUPPORT_ONLY_CSRRW: bool;
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct IMStandardIsaConfig;

impl MachineConfig for IMStandardIsaConfig {
    const SUPPORT_SIGNED_MUL: bool = true;
    const SUPPORT_SIGNED_DIV: bool = true;
    const SUPPORT_SIGNED_LOAD: bool = true;
    const SUPPORT_LOAD_LESS_THAN_WORD: bool = true;
    const SUPPORT_SRA: bool = true;
    const SUPPORT_ROT: bool = false;
    const SUPPORT_MOPS: bool = false;
    const HANDLE_EXCEPTIONS: bool = false;
    const SUPPORT_STANDARD_CSRS: bool = false;
    const SUPPORT_ONLY_CSRRW: bool = true;
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ReducedIMIsaConfig;

impl MachineConfig for ReducedIMIsaConfig {
    const SUPPORT_SIGNED_MUL: bool = false;
    const SUPPORT_SIGNED_DIV: bool = false;
    const SUPPORT_SIGNED_LOAD: bool = false;
    const SUPPORT_LOAD_LESS_THAN_WORD: bool = false;
    const SUPPORT_SRA: bool = false;
    const SUPPORT_ROT: bool = false;
    const SUPPORT_MOPS: bool = true;
    const HANDLE_EXCEPTIONS: bool = false;
    const SUPPORT_STANDARD_CSRS: bool = false;
    const SUPPORT_ONLY_CSRRW: bool = true;
}
