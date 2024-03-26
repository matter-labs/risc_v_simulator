use self::memory::{MemoryAccessTracer, MemorySource};
use crate::abstractions::memory::AccessType;
use crate::cycle::status_registers::TrapReason;
use std::hint::unreachable_unchecked;

pub mod memory;
pub mod non_determinism;


#[must_use]
#[inline(always)]
pub fn mem_read<M: MemorySource, MTR: MemoryAccessTracer>(
    memory_source: &mut M, tracer: &mut MTR, phys_address: u64, num_bytes: u32,
    access_type: AccessType, proc_cycle: u32, trap: &mut TrapReason
) -> u32 {
    assert!(access_type == AccessType::Instruction || access_type == AccessType::MemLoad);

    let unalignment = phys_address & 3;
    let aligned_address = phys_address & !3;
    let value = match (unalignment, num_bytes) {
        (0, 4) | (0, 2) | (2, 2) | (0, 1) | (1, 1) | (2, 1) | (3, 1) => {
            let value = memory_source.get(aligned_address, access_type, trap);
            let value_for_oracle  = if access_type == AccessType::Instruction {
                translator(value as u64) as u32
            } else {
                value
            };

            tracer.add_query(proc_cycle, access_type, aligned_address, value_for_oracle);
            let unalignment_bits = unalignment * 8;
            let value = value >> unalignment_bits;
            value
        }
        _ => {
            *trap = TrapReason::LoadAddressMisaligned;

            0u32 // formally
        }
    };

    let mask = match num_bytes {
        1 => 0x000000ff,
        2 => 0x0000ffff,
        4 => 0xffffffff,
        _ => unsafe { unreachable_unchecked() },
    };
    let value = value & mask;

    value
}


#[inline(always)]
pub fn mem_write<M: MemorySource, MTR: MemoryAccessTracer>(
    memory_source: &mut M, tracer: &mut MTR,
    phys_address: u64, value: u32, num_bytes: u32,
    proc_cycle: u32, trap: &mut TrapReason
) {
    let unalignment = phys_address & 3;
    let aligned_address = phys_address & !3;
    match (unalignment, num_bytes) {
        a @ (0, 4)
        | a @ (0, 2)
        | a @ (2, 2)
        | a @ (0, 1)
        | a @ (1, 1)
        | a @ (2, 1)
        | a @ (3, 1) => {
            let (unalignment, num_bytes) = a;

            // we need to load old value - just for easier comparison of simulator/in_circuit implementation
            let old_value = memory_source.get(aligned_address, AccessType::MemLoad, trap);
            if trap.is_a_trap() {
                return;
            }
            tracer.add_query(proc_cycle, AccessType::MemLoad, aligned_address, old_value);

            let value_mask = match num_bytes {
                1 => 0x000000ffu32,
                2 => 0x0000ffffu32,
                4 => 0xffffffffu32,
                _ => unsafe { unreachable_unchecked() },
            };

            let mask_old = match (unalignment, num_bytes) {
                (0, 1) => 0xffffff00u32,
                (0, 2) => 0xffff0000u32,
                (0, 4) => 0x00000000u32,
                (1, 1) => 0xffff00ffu32,
                (1, 2) => 0xffff00ffu32,
                (2, 1) => 0xff00ffffu32,
                (2, 2) => 0x0000ffffu32,
                (3, 1) => 0x00ffffffu32,
                _ => unsafe { unreachable_unchecked() },
            };

            let new_value =
                ((value & value_mask) << (unalignment * 8)) | (old_value & mask_old);

            memory_source.set(aligned_address, new_value, AccessType::MemStore, trap);
            if trap.is_a_trap() {
                return;
            }
            tracer.add_query(proc_cycle, AccessType::MemStore, aligned_address, new_value);
        }

        _ => {
            *trap = TrapReason::StoreOrAMOAddressMisaligned;
        }
    };
}
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[repr(u32)]
pub enum CsrRegisters {
    Invalid,
    Satp, 
    Mstatus, 
    Mie, 
    Mtvec, 
    Mscratch, 
    Mepc, 
    Mcause, 
    Mtval, 
    Mip,
    Mcustom,
}

impl CsrRegisters {
    pub fn to_encoding_variable(&self) -> u64 {
        let encoding: u64 = match self {
            Self::Satp    => 0,
            Self::Mstatus => 1,
            Self::Mie   => 2,
            Self::Mtvec   => 3,
            Self::Mscratch  => 4,
            Self::Mepc     => 5,
            Self::Mcause  => 6,
            Self::Mtval    => 7,
            Self::Mip     => 8,
            // TODO: double check please
            Self::Mcustom => 9,
            Self::Invalid => 10,
        };
        encoding
    }
    pub fn from_natural_encoding(num: u32) -> Self {
        match num {
            0x180 => Self::Satp,
            0x300 => Self::Mstatus,
            0x304 => Self::Mie, 
            0x305 => Self::Mtvec,
            0x340 => Self::Mscratch,
            0x341 => Self::Mepc,
            0x342 => Self::Mcause,
            0x343 => Self::Mtval,
            0x344 => Self::Mip,
            0x7c0 => Self::Mcustom,
            _    => Self::Invalid,
        }
    }

    pub fn from_encoding(num: u32) -> Self {
        match num {
            0 => Self::Satp,
            1 => Self::Mstatus,
            2 => Self::Mie, 
            3 => Self::Mtvec,
            4 => Self::Mscratch,
            5 => Self::Mepc,
            6 => Self::Mcause,
            7 => Self::Mtval,
            8 => Self::Mip,
            9 => Self::Mcustom,
            _    => Self::Invalid,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Opcode {
    OpcodeLoad, OpcodeMiscMem, OpcodeOpImm, OpcodeAuipc, OpcodeStore, OpcodeOp, 
    OpcodeLui, OpcodeBranch, OpcodeJalr, OpcodeJal, OpcodeSystem, OpcodeInvalid
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Instruction {
    LUI, AUIPC, JAL, JALR, BEQ, BNE, BLT, BGE, BLTU, BGEU, LB, LH, LW, LBU, LHU, SB, SH, SW, ADDI, SLTI, SLTIU,
    XORI, ORI, ANDI, SLLI, SRLI, SRAI, ADD, SUB, SLL, SLT, SLTU, XOR, SRL, SRA, OR, AND, FENCE, FENCEI, 
    CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI, MUL, MULH, MULHSU, MULHU, DIV, DIVU, REM, REMU,
    ECALL, EBREAK, MRET, WFI, InstructionInvalid
}
pub const NUM_OF_RISC_V_INSNS: usize = Instruction::InstructionInvalid as usize;

impl Instruction {
    pub fn list_all_valid_insns() -> [Instruction; NUM_OF_RISC_V_INSNS] {
        std::array::from_fn(|idx| unsafe { std::mem::transmute(idx as u8) } )
    }
    pub fn from_instruction_type_to_opcode(self) -> Opcode {
        match self {
            Self::LUI   => Opcode::OpcodeLui,
            Self::AUIPC => Opcode::OpcodeAuipc,
            Self::JAL   => Opcode::OpcodeJal,
            Self::JALR  => Opcode::OpcodeJalr,
            Self::BEQ   | Self::BNE    | Self::BLT    | Self::BGE   | Self::BLTU  | Self::BGEU   => Opcode::OpcodeBranch,
            Self::LB    | Self::LH     | Self::LW     | Self::LBU   | Self::LHU   => Opcode::OpcodeLoad,
            Self::SB    | Self::SH     | Self::SW     => Opcode::OpcodeStore,
            Self::ADDI  | Self::SLTI   | Self::SLTIU  | Self::XORI  | Self::ORI   | Self::ANDI   | Self::SLLI   | Self::SRLI   | Self::SRAI => Opcode::OpcodeOpImm,
            Self::ADD   | Self::SUB    | Self::SLL    | Self::SLT   | Self::SLTU  | Self::XOR    | Self::SRL    | Self::SRA    | Self::OR   | Self::AND => Opcode::OpcodeOp,
            Self::CSRRW  | Self::CSRRS | Self::CSRRC | Self::CSRRWI | Self::CSRRSI | Self::CSRRCI => Opcode::OpcodeSystem,
            Self::MUL   | Self::MULH   | Self::MULHSU | Self::MULHU | Self::DIV   | Self::DIVU   | Self::REM    | Self::REMU   => Opcode::OpcodeOp,
            Self::FENCE | Self::FENCEI   => Opcode::OpcodeMiscMem,
            Self::ECALL | Self::EBREAK | Self::MRET   | Self::WFI   => Opcode::OpcodeSystem,
            _ => Opcode::OpcodeInvalid,
        }
    }
    pub fn from_instr_to_opcode(instr: Instruction) -> u32 {
        match instr {
            Instruction::LUI => 0b00000000,
            Instruction::AUIPC => 0b00000001,
            Instruction::JAL => 0b00000010,
            Instruction::JALR => 0b00000011,
            Instruction::BEQ => 0b00000100,
            Instruction::BNE => 0b00000101,
            Instruction::BLT => 0b00000110,
            Instruction::BGE => 0b00000111,
            Instruction::BLTU => 0b00001000,
            Instruction::BGEU => 0b00001001,
            Instruction::LB => 0b00001010,
            Instruction::LH => 0b00001011,
            Instruction::LW => 0b00001100,
            Instruction::LBU => 0b00001101,
            Instruction::LHU => 0b00001110,
            Instruction::SB => 0b00001111,
            Instruction::SH => 0b00010000,
            Instruction::SW => 0b00010001,
            Instruction::ADDI => 0b00010010,
            Instruction::SLTI => 0b00010011,
            Instruction::SLTIU => 0b00010100,
            Instruction::XORI => 0b00010101,
            Instruction::ORI => 0b00010110,
            Instruction::ANDI => 0b00010111,
            Instruction::SLLI => 0b00011000,
            Instruction::SRLI => 0b00011001,
            Instruction::SRAI => 0b00011010,
            Instruction::ADD => 0b00011011,
            Instruction::SUB => 0b00011100,
            Instruction::SLL => 0b00011101,
            Instruction::SLT => 0b00011110,
            Instruction::SLTU => 0b00011111,
            Instruction::XOR => 0b00100000,
            Instruction::SRL => 0b00100001,
            Instruction::SRA => 0b00100010,
            Instruction::OR => 0b00100011,
            Instruction::AND => 0b00100100,
            Instruction::FENCE => 0b00100101,
            Instruction::FENCEI => 0b00100110,
            Instruction::CSRRW => 0b00100111,
            Instruction::CSRRS => 0b00101000,
            Instruction::CSRRC => 0b00101001,
            Instruction::CSRRWI => 0b00101010,
            Instruction::CSRRSI => 0b00101011,
            Instruction::CSRRCI => 0b00101100,
            Instruction::MUL => 0b00101101,
            Instruction::MULH => 0b00101110,
            Instruction::MULHSU => 0b00101111,
            Instruction::MULHU => 0b00110000,
            Instruction::DIV => 0b00110001,
            Instruction::DIVU => 0b00110010,
            Instruction::REM => 0b00110011,
            Instruction::REMU => 0b00110100,
            Instruction::ECALL => 0b00110101,
            Instruction::EBREAK => 0b00110110,
            Instruction::MRET => 0b00110111,
            Instruction::WFI => 0b00111000,
            Instruction::InstructionInvalid => 0b1111111,
        }
    }
}
fn define_instruction(instruction: u64) -> Instruction {
    const LOWEST_7_BITS_MASK: u64 = 0x7f;
    let instr = match instruction & LOWEST_7_BITS_MASK {
        0b0110111 => {
            // LUI
            Instruction::LUI
        },
        0b0010111 => {
            // AUIPC
            Instruction::AUIPC
        },
        0b1101111 => {
            // JAL
            Instruction::JAL
        },
        0b1100111 => {
            // JALR
            Instruction::JALR
        }
        0b1100011 => {
            // BRANCH
            let funct3 = funct3(instruction);

            let kind_of_instr = match funct3 {
                0 => Instruction::BEQ,
                1 => Instruction::BNE,
                4 => Instruction::BLT,
                5 => Instruction::BGE,
                6 => Instruction::BLTU,
                7 => Instruction::BGEU,
                _ => Instruction::InstructionInvalid
            };

            kind_of_instr
        },

        0b0000011 => {
            // LOAD
            let funct3 = funct3(instruction);

            let kind_of_instr = match funct3 {
                0 => Instruction::LB,
                1 => Instruction::LH,
                2 => Instruction::LW,
                4 => Instruction::LBU,
                5 => Instruction::LHU,
                _ => Instruction::InstructionInvalid
            };
            kind_of_instr
        },
        0b0100011 => {
            // STORE
            let funct3 = funct3(instruction);

            let kind_of_instr = match funct3 {
                0 => Instruction::SB,
                1 => Instruction::SH,
                2 => Instruction::SW,
                _ => Instruction::InstructionInvalid
            };
            kind_of_instr
        },

        0b0110011 => {
            // op 
            let funct3 = funct3(instruction);
            let funct7 = funct7(instruction);
            if funct7 == 1 {
                let kind_of_instr = match funct3 {
                    0 => Instruction::MUL, 
                    1 => Instruction::MULH, 
                    2 => Instruction::MULHSU, 
                    3 => Instruction::MULHU,
                    4 => Instruction::DIV,
                    5 => Instruction::DIVU,
                    6 => Instruction::REM,
                    7 => Instruction::REMU,
                    _ => Instruction::InstructionInvalid
                };
                return kind_of_instr;
            } else if funct7 == 0 {
                let kind_of_instr = match funct3 {
                    0 => Instruction::ADD, 
                    1 => Instruction::SLL,
                    2 => Instruction::SLT, 
                    3 => Instruction::SLTU,
                    4 => Instruction::XOR,
                    5 => Instruction::SRL,
                    6 => Instruction::OR,
                    7 => Instruction::AND,
                    _ => Instruction::InstructionInvalid
                };
                return kind_of_instr;
            } else if funct7 == 32{
                let kind_of_instr = match funct3 {
                    0 => Instruction::SUB, 
                    5 => Instruction::SRA,
                    _ => Instruction::InstructionInvalid
                };
                return kind_of_instr;
            } else {
                Instruction::InstructionInvalid
            }
        },
        0b0010011 => { 
            // op-imm
            let funct3 = funct3(instruction);
            let funct7 = funct7(instruction);

            let kind_of_instr = match funct3 {
                0 => Instruction::ADDI, 
                1 => Instruction::SLLI,
                2 => Instruction::SLTI, 
                3 => Instruction::SLTIU,
                4 => Instruction::XORI,
                5 => { 
                    if funct7 == 0 {
                        Instruction::SRLI

                    } else if funct7 == 32 {
                        Instruction::SRAI
                    } else{
                        Instruction::InstructionInvalid
                    }

                },
                6 => Instruction::ORI,
                7 => Instruction::ANDI,
                _ => Instruction::InstructionInvalid
            };
            kind_of_instr

        },
        0b0001111 => {
            let funct3 = funct3(instruction);
            let kind_of_instr = match funct3 {
                0 => Instruction::FENCE,
                1 => Instruction::FENCEI,
                _ => Instruction::InstructionInvalid
            };
            kind_of_instr
        },
        0b1110011 => {
            let funct3 = funct3(instruction);
            let kind_of_instr = match funct3 {
                0 => {
                    let funct12 = funct12(instruction);
                    match funct12 {
                        0 => Instruction::ECALL,
                        1 => Instruction::EBREAK,
                        0b1100000010 => Instruction::MRET,
                        0b100000101 => Instruction::WFI,
                        _ => Instruction::InstructionInvalid
                        
                    }
                }, 
                1 => Instruction::CSRRW,
                2 => Instruction::CSRRS,
                3 => Instruction::CSRRC,
                5 => Instruction::CSRRWI,
                6 => Instruction::CSRRSI,
                7 => Instruction::CSRRCI,
                _ => Instruction::InstructionInvalid
            };
            kind_of_instr
        }
        _ => Instruction::InstructionInvalid
    };
    instr

}
pub fn translator(init: u64) -> u64 {
    let instr = define_instruction(init);
    let opcode = instr.from_instruction_type_to_opcode();
    let new_instr_form = match opcode {
        Opcode::OpcodeMiscMem | Opcode::OpcodeOp => { 
            let instruction_bits = Instruction::from_instr_to_opcode(instr);
            let mut tmp = clear_bits(init, 12, 3);
            tmp = clear_bits(tmp, 25, 7);
            replace_opcode(tmp, instruction_bits as u64)
        },
        Opcode::OpcodeOpImm => {
            //             Self::ADDI  | Self::SLTI   | Self::SLTIU  | Self::XORI  | Self::ORI   | Self::ANDI   | Self::SLLI   | Self::SRLI   | Self::SRAI 
            let instruction_bits = Instruction::from_instr_to_opcode(instr);
            match instr {
                Instruction::SLLI | Instruction::SRLI | Instruction::SRAI => {
                    let mut tmp = clear_bits(init, 12, 3);
                    tmp = clear_bits(tmp, 25, 7);
                    replace_opcode(tmp, instruction_bits as u64)
                },
                Instruction::ADDI | Instruction::SLTI | Instruction::SLTIU |Instruction::XORI | Instruction::ORI | Instruction::ANDI => {
                    let tmp = clear_bits(init, 12, 3);
                    replace_opcode(tmp, instruction_bits as u64)
                }
                _ => panic!("new instruction wasn`t added")
            }
        },
        Opcode::OpcodeLui | Opcode::OpcodeAuipc | Opcode::OpcodeJal | Opcode::OpcodeJalr => { 
            // LUI, AUIPC
            let instruction_bits = Instruction::from_instr_to_opcode(instr);
            replace_opcode(init, instruction_bits as u64)

        },
        Opcode::OpcodeBranch | Opcode::OpcodeLoad | Opcode::OpcodeStore => { 
            let instruction_bits = Instruction::from_instr_to_opcode(instr);
            let tmp = clear_bits(init, 12, 3);
            replace_opcode(tmp, instruction_bits as u64)
        },
        Opcode::OpcodeSystem => { 
            let instruction_bits = Instruction::from_instr_to_opcode(instr);
            let funct12 = funct12(init);
            match instr {
                // CSRRW, CSRRS, CSRRC, CSRRWI, CSRRSI, CSRRCI
                Instruction::ECALL | Instruction::EBREAK | Instruction::MRET | Instruction::WFI => {
                    let tmp = clear_bits(init, 20, 12);
                    replace_opcode(tmp, instruction_bits as u64)
                },
                Instruction::FENCE | Instruction::FENCEI => {
                    let tmp = clear_bits(init, 12, 3);
                    replace_opcode(tmp, instruction_bits as u64)
                },
                Instruction::CSRRW | Instruction::CSRRS | Instruction::CSRRC | Instruction::CSRRWI | Instruction::CSRRSI | Instruction::CSRRCI => {
                    let mut tmp = clear_bits(init, 12, 3);
                    let csr_register = CsrRegisters::from_natural_encoding(funct12.try_into().unwrap());
                    let new_csr_reg_encoding = CsrRegisters::to_encoding_variable(&csr_register);
                    tmp = clear_bits(tmp, 20, 12);
                    let fin = tmp | (new_csr_reg_encoding << 20);
                    replace_opcode(fin, instruction_bits as u64)
                }
                _ => panic!("new instruction wasn`t added")
            }
        },
        Opcode::OpcodeInvalid => {
            let instruction_bits = Instruction::from_instr_to_opcode(instr);
            // let tmp = clear_bits(init, 7, 25);
            replace_opcode(init, instruction_bits as u64)
        },
    };
    new_instr_form
}
#[inline(always)]
pub const fn get_bits(src: u64, from_bit: u64, num_bits: u64) -> u64 {
    let mask = ((1 << num_bits) - 1) << from_bit;
    (src & mask) >> from_bit
}
#[inline(always)]
pub const fn funct3(src: u64) -> u64 {
    get_bits(src, 12, 3)
}
#[inline(always)]
pub const fn funct7(src: u64) -> u64 {
    get_bits(src, 25, 7)
}
#[inline(always)]
pub const fn funct12(src: u64) -> u64 {
    get_bits(src, 20, 12)
}
#[inline(always)]
pub const fn get_rd(src: u64) -> u64 {
    get_bits(src, 7, 5)
}
#[inline(always)]
pub const fn imm_chunk(src: u64, from_bit: u64, num_bits: u64) -> u64 {
    get_bits(src, from_bit, num_bits)
}
pub const fn replace_opcode(instruction: u64, new_instruction_bits: u64) -> u64 {
    let mask: u64 = !0b0111_1111; 
    let cleared_instruction = instruction & mask;
    let new_instruction = cleared_instruction | (new_instruction_bits as u64);

    new_instruction
}
#[inline(always)]
pub const fn clear_bits(dst: u64, from_bit: u64, num_bits: u64) -> u64 {
    let mask = !(((1 << num_bits) - 1) << from_bit);
    dst & mask
}




