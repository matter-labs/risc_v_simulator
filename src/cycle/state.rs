use std::collections::HashMap;
use std::hint::unreachable_unchecked;

use super::status_registers::*;
use crate::abstractions::memory::{AccessType, MemoryAccessTracer, MemorySource};
use crate::abstractions::MemoryImplementation;
use crate::mmio::MMIOImplementation;
use crate::mmu::MMUImplementation;

use crate::utils::*;

use super::opcode_formats::*;

pub const NUM_REGISTERS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Mode {
    User = 0,
    Supervisor = 1,
    Reserved = 2,
    Machine = 3,
}

impl Mode {
    #[must_use]
    #[inline(always)]
    pub const fn as_register_value(self) -> u32 {
        self as u32
    }

    #[must_use]
    #[inline(always)]
    pub const fn from_proper_bit_value(src: u32) -> Self {
        match src {
            i if i == Mode::User as u32 => Mode::User,
            i if i == Mode::Supervisor as u32 => Mode::Supervisor,
            i if i == Mode::Reserved as u32 => Mode::Reserved,
            i if i == Mode::Machine as u32 => Mode::Machine,
            _ => unsafe { unreachable_unchecked() },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TrapStateRegisters {
    pub status: u32, // status register
    pub ie: u32,     // interrupt-enable register
    pub ip: u32,     // interrupt-pending register
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TrapSetupRegisters {
    // pub isa: u32, // we will not use it
    // pub edeleg: u32, // we will not use it
    // pub ideleg: u32, // we will not use it
    // pub counteren: u32, // we will not use it
    pub tvec: u32, // trap-handler base address
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TrapHandlingRegisters {
    pub scratch: u32, // scratch register for machine trap handlers
    pub epc: u32,     // exception program counter
    pub cause: u32,   // trap cause
    pub tval: u32,    // bad address or instruction
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ModeStatusAndTrapRegisters {
    pub state: TrapStateRegisters,
    pub setup: TrapSetupRegisters,
    pub handling: TrapHandlingRegisters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ExtraFlags(pub u32);

impl ExtraFlags {
    pub const WAIT_FOR_INTERRUPT_BIT: u32 = 2;

    #[must_use]
    #[inline(always)]
    pub const fn get_current_mode(self) -> Mode {
        Mode::from_proper_bit_value(get_bits_and_align_right(self.0, 0, 2))
    }

    #[inline(always)]
    pub const fn set_mode(&mut self, mode: Mode) {
        self.0 = (self.0 & !0x3) | mode.as_register_value();
    }

    #[inline(always)]
    pub const fn set_mode_raw(&mut self, mode_bits: u32) {
        self.0 = (self.0 & !0x3) | mode_bits;
    }

    #[must_use]
    #[inline(always)]
    pub const fn get_wait_for_interrupt(self) -> u32 {
        get_bit_unaligned(self.0, Self::WAIT_FOR_INTERRUPT_BIT)
    }

    #[inline(always)]
    pub const fn set_wait_for_interrupt_bit(&mut self) {
        set_bit(&mut self.0, Self::WAIT_FOR_INTERRUPT_BIT)
    }

    #[inline(always)]
    pub const fn clear_wait_for_interrupt_bit(&mut self) {
        clear_bit(&mut self.0, Self::WAIT_FOR_INTERRUPT_BIT)
    }
}

#[derive(Clone, Debug)]
pub struct StateTracer {
    pub tracer: HashMap<usize, RiscV32State>
}

impl StateTracer {
    pub fn new() -> Self {
        Self {
            tracer: HashMap::new(),
        }
    }

    pub fn insert(&mut self, idx: usize, state: RiscV32State) {
        self.tracer.insert(idx, state);
    }

    pub fn get(&self, idx: usize) -> Option<&RiscV32State> {
        self.tracer.get(&idx)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RiscV32State {
    pub registers: [u32; NUM_REGISTERS],
    pub pc: u32,
    pub extra_flags: ExtraFlags, // everything that doesn't need full register

    pub cycle_counter: u64,
    pub timer: u64,
    pub timer_match: u64,

    pub machine_mode_trap_data: ModeStatusAndTrapRegisters,

    pub sapt: u32 // for debugging
}

impl RiscV32State {
    pub fn initial(initial_pc: u32) -> Self {
        // we should start in machine mode, the rest is not important and can be by default
        let registers = [0u32; NUM_REGISTERS];
        let pc = initial_pc;
        let mut extra_flags = ExtraFlags(0u32);
        extra_flags.set_mode(Mode::Machine);

        let cycle_counter = 0u64;
        let timer = 0u64;
        let timer_match = u64::MAX;

        let machine_mode_trap_data = ModeStatusAndTrapRegisters {
            state: TrapStateRegisters {
                status: 0,
                ie: 0,
                ip: 0,
            },
            setup: TrapSetupRegisters { tvec: 0 },
            handling: TrapHandlingRegisters {
                scratch: 0,
                epc: 0,
                cause: 0,
                tval: 0,
            },
        };

        let sapt = 0u32;

        Self {
            registers,
            pc,
            extra_flags,
            cycle_counter,
            timer,
            timer_match,
            machine_mode_trap_data,
            sapt,
        }
    }

    #[must_use]
    #[inline(always)]
    pub const fn get_register(&self, reg_idx: u32) -> u32 {
        self.registers[reg_idx as usize]
    }

    #[inline(always)]
    pub const fn set_register(&mut self, reg_idx: u32, value: u32) {
        if reg_idx != 0 {
            self.registers[reg_idx as usize] = value;
        }
    }

    pub fn cycle<
        'a,
        'b: 'a,
        const N: usize,
        M: MemorySource,
        MTR: MemoryAccessTracer,
        MMU: MMUImplementation<M, MTR, AuxilarySource = MemoryImplementation<M, MTR>>,
    >(
        &'a mut self,
        memory: &'a mut MemoryImplementation<M, MTR>,
        mmu: &'a mut MMU,
        mmio: &'a mut MMIOImplementation<'b, N>,
    ) {
        // increment cycle
        self.timer = self.timer.wrapping_add(1u64);
        let current_privilege_mode = self.extra_flags.get_current_mode();
        // timer interrupt. We assume timer to be just a reflection of the cycle
        if self.timer > self.timer_match {
            self.extra_flags.clear_wait_for_interrupt_bit();
            // clear WFI
            clear_bit(&mut self.extra_flags.0, ExtraFlags::WAIT_FOR_INTERRUPT_BIT);
            // set interrupt mode pending
            set_bit(
                &mut self.machine_mode_trap_data.state.ip,
                InterruptReason::MachineTimerInterrupt as u32,
            );
        } else {
            clear_bit(
                &mut self.machine_mode_trap_data.state.ip,
                InterruptReason::MachineTimerInterrupt as u32,
            );
        }

        if self.extra_flags.get_wait_for_interrupt() != 0 {
            return;
        }

        let mut pc = self.pc;
        let mut ret_val: u32 = 0;
        let mut trap: u32 = 0;
        let mut instr: u32 = 0;

        // println!("PC = 0x{:08x}", pc);

        // interrupt handling, if it's enabled
        if MStatusRegister::mie(self.machine_mode_trap_data.state.status) != 0
            && test_bit(
                self.machine_mode_trap_data.state.ip,
                InterruptReason::MachineTimerInterrupt as u32,
            )
            && test_bit(
                self.machine_mode_trap_data.state.ie,
                InterruptReason::MachineTimerInterrupt as u32,
            )
        {
            trap = InterruptReason::MachineExternalInterrupt.as_register_value();
            self.pc = self.pc.wrapping_sub(4u32);
        } else {
            'cycle_block: {
                self.cycle_counter += 1;
                memory.notify_new_cycle();

                // normal cycle
                // check PC is aligned
                if pc & 0x3 != 0 {
                    // unaligned PC
                    trap = TrapReason::InstructionAddressMisaligned.as_register_value();
                    break 'cycle_block;
                }
                // we assume no InstructionAccessFault here
                let instruction_phys_address = mmu.map_virtual_to_physical(
                    pc,
                    current_privilege_mode,
                    AccessType::Instruction,
                    memory,
                    &mut trap,
                );
                if trap != 0 {
                    // error during address translation
                    debug_assert_eq!(trap, TrapReason::InstructionPageFault.as_register_value());
                    break 'cycle_block;
                }

                instr = memory.read(
                    instruction_phys_address,
                    4,
                    AccessType::Instruction,
                    &mut trap,
                );

                println!("PC = 0x{:08x}, instr = 0x{:08x}", pc, instr);

                if trap != 0 {
                    // error during address translation
                    debug_assert_eq!(trap, TrapReason::InstructionAccessFault.as_register_value());
                    break 'cycle_block;
                }
                // decode the instruction and perform cycle

                // destination register
                let mut rd = get_rd(instr);

                // note on all the PC operations below: if we modify PC in the opcode,
                // we subtract 4 from it, to later on add 4 once at the end of the loop. For MOST
                // of the opcodes it makes sense to shorten the opcode body

                const LOWEST_7_BITS_MASK: u32 = 0x7f;

                let bp = 0x0;
                // let bp = 0x0100045c;

                // if pc == bp {
                //     println!("Breakpoint at 0x{:08x}", bp);
                //     self.pretty_dump();
                //     self.stack_dump(memory, mmu);
                // } else {
                //     // println!("Pc = 0x{:08x}", pc);
                // }
                // println!("Pc = 0x{:08x}; insn = 0x{:08x}", pc, instr);

                match instr & LOWEST_7_BITS_MASK {
                    0b0110111 => {
                        // LUI
                        let imm = UTypeOpcode::imm(instr);

                        ret_val = imm;
                    },
                    0b0010111 => {
                        // AUIPC
                        let imm = UTypeOpcode::imm(instr);

                        ret_val = pc.wrapping_add(imm);
                    },
                    0b1101111 => {
                        // JAL
                        let mut rel_addr: u32 = JTypeOpcode::imm(instr);
                        // quasi-sign-extend
                        sign_extend(&mut rel_addr, 21);

                        ret_val = pc.wrapping_add(4u32);
                        pc = pc.wrapping_sub(4u32).wrapping_add(rel_addr);
                    },
                    0b1100111 => {
                        // JALR
                        let mut imm: u32 = ITypeOpcode::imm(instr);
                        // quasi sign extend
                        sign_extend(&mut imm, 12);

                        ret_val = pc.wrapping_add(4u32);
                        let rs1 = ITypeOpcode::rs1(instr);
                        let reg_value = self.get_register(rs1);
                        //  The target address is obtained by adding the 12-bit signed I-immediate 
                        // to the register rs1, then setting the least-significant bit of the result to zero
                        pc = (reg_value.wrapping_add(imm) & !0x1).wrapping_sub(4u32);
                    }
                    0b1100011 => {
                        // BRANCH
                        let mut imm = BTypeOpcode::imm(instr);
                        sign_extend(&mut imm, 12);

                        let rs1 = BTypeOpcode::rs1(instr);
                        let rs1 = self.get_register(rs1);
                        let rs2 = BTypeOpcode::rs2(instr);
                        let rs2 = self.get_register(rs2);
                        rd = 0;
                        let dst = pc.wrapping_add(imm).wrapping_sub(4u32);
                        let funct3 = BTypeOpcode::funct3(instr);
                        match funct3 {
                            0 => { if rs1 == rs2 { pc = dst } },
                            1 => { if rs1 != rs2 { pc = dst } },
                            4 => { if (rs1 as i32) < (rs2 as i32) { pc = dst } },
                            5 => { if (rs1 as i32) >= (rs2 as i32) { pc = dst } },
                            6 => { if rs1 < rs2 { pc = dst } },
                            7 => { if rs1 >= rs2 { pc = dst } },
                            _ => {
                                trap = TrapReason::IllegalInstruction.as_register_value();
                                break 'cycle_block;
                            },
                        }
                    },
                    0b0000011 => {
                        // LOAD

                        if rd == 0 {
                            // Exception raised: loads with a destination of x0 must still raise 
                            // any exceptions and action any other side effects 
                            // even though the load value is discarded
                            trap = TrapReason::IllegalInstruction.as_register_value();
                            break 'cycle_block;
                        }

                        let mut imm = ITypeOpcode::imm(instr);
                        sign_extend(&mut imm, 12);

                        let rs1 = ITypeOpcode::rs1(instr);
                        let rs1 = self.get_register(rs1);
                        let virtual_address = rs1.wrapping_add(imm);

                        // println!("Load into x{:02} from 0x{:08x} at PC = 0x{:08x}", rd, virtual_address, pc);

                        // we formally access it once, but most likely full memory access
                        // will be abstracted away into external interface hiding memory translation too
                        let operand_phys_address = mmu.map_virtual_to_physical(virtual_address, current_privilege_mode, AccessType::Load, memory, &mut trap);
                        if trap != 0 {
                            // error during address translation
                            debug_assert_eq!(trap, TrapReason::LoadPageFault.as_register_value());
                            break 'cycle_block;
                        }

                        // try MMIO first
                        if let Ok(val) = mmio.read(operand_phys_address, &mut trap) {
                            if trap != 0 {
                                break 'cycle_block;
                            }
                            ret_val = val;
                        } else {
                            // access memory
                            let funct3 = ITypeOpcode::funct3(instr);
                            match funct3 {
                                a @ 0 | a @ 1 | a @ 2 | a @ 4 | a @ 5 => {
                                    let num_bytes = match a {
                                        0 | 4 => 1,
                                        1 | 5 => 2,
                                        2 => 4,
                                        _ => unsafe {unreachable_unchecked()}
                                    };
                                    // Memory implementation should handle read in full. For now we only use one
                                    // that doesn't step over 4 byte boundary ever, meaning even though formal address is not 4 byte aligned,
                                    // loads of u8/u16/u32 are still "aligned"
                                    let operand = memory.read(operand_phys_address, num_bytes, AccessType::Load, &mut trap);
                                    if trap != 0 {
                                        debug_assert_eq!(trap, TrapReason::LoadAddressMisaligned.as_register_value());
                                        break 'cycle_block;
                                    }
                                    // now depending on the type of load we extend it
                                    ret_val = match a {
                                        0 => sign_extend_8(operand),
                                        1 => sign_extend_16(operand),
                                        2 => operand,
                                        4 => zero_extend_8(operand),
                                        5 => zero_extend_16(operand),
                                        _ => unsafe {unreachable_unchecked()}
                                    };
                                },
                                _ => {
                                    trap = TrapReason::IllegalInstruction.as_register_value();
                                    break 'cycle_block;
                                },
                            }
                        }
                    },
                    0b0100011 => {
                        // STORE
                        let mut imm = STypeOpcode::imm(instr);
                        sign_extend(&mut imm, 12);

                        let rs1 = STypeOpcode::rs1(instr);
                        let rs1 = self.get_register(rs1);
                        let rs2 = STypeOpcode::rs2(instr);
                        let rs2 = self.get_register(rs2);
                        let virtual_address = imm.wrapping_add(rs1);
                        // it's S-type, that has no RD, so set it to x0
                        rd = 0;

                        // println!("Store of x{:02} = 0x{:08x} into 0x{:08x} at PC = 0x{:08x}", STypeOpcode::rs2(instr), rs2, virtual_address, pc);

                        // store operand rs2

                        // we formally access it once, but most likely full memory access
                        // will be abstracted away into external interface hiding memory translation too
                        let operand_phys_address = mmu.map_virtual_to_physical(virtual_address, current_privilege_mode, AccessType::Store, memory, &mut trap);
                        if trap != 0 {
                            debug_assert_eq!(trap, TrapReason::StoreOrAMOPageFault.as_register_value());
                            break 'cycle_block;
                        }

                        if let Ok(_) = mmio.write(operand_phys_address, rs2, &mut trap) {
                            if trap != 0 {
                                break 'cycle_block;
                            }
                        } else {
                            // access memory
                            let funct3 = STypeOpcode::funct3(instr);
                            match funct3 {
                                a @ 0 | a @ 1 | a @ 2 => {
                                    let store_length = 1 << a;
                                    // memory handles the write in full, whether it's aligned or not, or whatever
                                    memory.write(operand_phys_address, rs2, store_length, AccessType::Store, &mut trap);
                                    if trap != 0 {
                                        // error during address translation
                                        debug_assert_eq!(trap, TrapReason::StoreOrAMOAddressMisaligned.as_register_value());
                                        break 'cycle_block;
                                    }
                                },
                                _ => {
                                    trap = TrapReason::IllegalInstruction.as_register_value();
                                    break 'cycle_block;
                                },
                            }
                        }
                    },
                    0b0010011 | // Op-immediate
                    0b0110011 // op 
                    => {
                        const TEST_REG_REG_MASK: u32 = 0x20;
                        let is_r_type = instr & TEST_REG_REG_MASK != 0;
                        let rs1 = RTypeOpcode::rs1(instr); // same as IType
                        let operand_1 = self.get_register(rs1);
                        let operand_2 = if is_r_type {
                            let rs2 = BTypeOpcode::rs2(instr);
                            self.get_register(rs2)
                        } else {
                            let mut imm = ITypeOpcode::imm(instr);
                            sign_extend(&mut imm, 12);

                            imm
                        };

                        let funct3 = RTypeOpcode::funct3(instr); // same as IType
                        let funct7 = RTypeOpcode::funct7(instr);
                        if is_r_type && funct7 & 1 != 0 {
                            // RV32M - multiplication subset
                            ret_val = match funct3 {
                                0 => { (operand_1 as i32).wrapping_mul(operand_2 as i32) as u32}, // signed MUL
                                1 => { (((operand_1 as i32) as i64).wrapping_mul((operand_2 as i32) as i64) >> 32) as u32}, // MULH
                                2 => { (((operand_1 as i32) as i64).wrapping_mul(((operand_2 as u32) as u64) as i64) >> 32) as u32}, // MULHSU
                                3 => { ((operand_1 as u64).wrapping_mul(operand_2 as u64) >> 32) as u32}, // MULHU
                                4 => {
                                    // DIV
                                    if operand_2 == 0 {
                                        -1i32 as u32
                                    } else {
                                        if operand_1 as i32 == i32::MIN && operand_1 as i32 == -1 {
                                            operand_1
                                        } else {
                                            ((operand_1 as i32) / (operand_2 as i32)) as u32
                                        }
                                    }
                                },
                                5 => {
                                    // DIVU
                                    if operand_2 == 0 {
                                        0xffffffff
                                    } else {
                                        operand_1 / operand_2
                                    }
                                },
                                6 => {
                                    // REM
                                    if operand_2 == 0 {
                                        operand_1
                                    } else {
                                        if operand_1 as i32 == i32::MIN && operand_1 as i32 == -1 {
                                            0u32
                                        } else {
                                            ((operand_1 as i32) % (operand_2 as i32)) as u32
                                        }
                                    }
                                },
                                7 => {
                                    // REM
                                    if operand_2 == 0 {
                                        operand_1
                                    } else {
                                        operand_1 % operand_2
                                    }
                                },
                                _ => unsafe {
                                    unreachable_unchecked()
                                },
                            };
                        } else {
                            // basic set
                            const ARITHMETIC_SHIFT_RIGHT_TEST_MASK: u32 = 0x40000000;
                            const SUB_TEST_MASK: u32 = 0x40000000;
                            ret_val = match funct3 {
                                0 => {
                                    if is_r_type && instr & SUB_TEST_MASK != 0 {
                                        operand_1.wrapping_sub(operand_2)
                                    } else {
                                        operand_1.wrapping_add(operand_2)
                                    }
                                },
                                1 => {
                                    // Shift left
                                    // shift is encoded in lowest 5 bits
                                    operand_1 << (operand_2 & 0x1f)
                                },
                                2 => {
                                    // Signed LT
                                    ((operand_1 as i32) < (operand_2 as i32)) as u32
                                },
                                3 => {
                                    // Unsigned LT
                                    (operand_1 < operand_2) as u32
                                },
                                4 => {
                                    // XOR
                                    operand_1 ^ operand_2
                                },
                                5 => {
                                    // Arithmetic shift right
                                    // shift is encoded in lowest 5 bits
                                    if instr & ARITHMETIC_SHIFT_RIGHT_TEST_MASK != 0 {
                                        ((operand_1 as i32) >> (operand_2 & 0x1f)) as u32
                                    } else {
                                        operand_1  >> (operand_2 & 0x1f)
                                    }
                                },
                                6 => {
                                    // OR
                                    operand_1 | operand_2
                                },
                                7 => {
                                    // AND
                                    operand_1 & operand_2
                                },
                                _ => unsafe {
                                    unreachable_unchecked()
                                },
                            };
                        }
                    },
                    0b0001111 => {
                        // nothing to do in fence, our memory is linear
                        rd = 0;
                    },
                    0b1110011 => {
                        // various control instructions, we implement only a subset
                        const ZICSR_MASK: u32 = 0x3;

                        let funct3 = ITypeOpcode::funct3(instr);
                        let csr_number = ITypeOpcode::imm(instr);
                        let csr_privilege_mode = get_bits_and_align_right(csr_number, 8, 2);
                        let csr_privilege_mode = Mode::from_proper_bit_value(csr_privilege_mode);
                        if csr_privilege_mode.as_register_value() > current_privilege_mode.as_register_value() {
                            trap = TrapReason::IllegalInstruction.as_register_value();
                            break 'cycle_block;
                        }

                        // so now we can just use full integer values for csr numbers
                        if funct3 & ZICSR_MASK != 0 {
                            let rs1_as_imm = ITypeOpcode::rs1(instr);
                            let rs1 = self.get_register(rs1_as_imm);

                            let mut write_val = rs1;

                            // read
                            match csr_number {
                                0x180 => {
                                    // satp
                                    ret_val = mmu.read_sapt(current_privilege_mode, &mut trap);
                                    if trap != 0 {
                                        break 'cycle_block;
                                    }
                                },
                                0x300 => ret_val = self.machine_mode_trap_data.state.status, // mstatus
                                0x301 => ret_val = 0b01_00_0000_0001_0000_0001_0001_0000_0000u32, //misa (I + M + usemode)
                                0x304 => ret_val = self.machine_mode_trap_data.state.ie, // mie
                                0x305 => ret_val = self.machine_mode_trap_data.setup.tvec, // mtvec
                                0x340 => ret_val = self.machine_mode_trap_data.handling.scratch, // mscratch
                                0x341 => ret_val = self.machine_mode_trap_data.handling.epc, // mepc
                                0x342 => ret_val = self.machine_mode_trap_data.handling.cause, // mcause
                                0x343 => ret_val = self.machine_mode_trap_data.handling.tval, // mtval
                                0x344 => ret_val = self.machine_mode_trap_data.state.ip, // mip
                                0xc00 => ret_val = self.cycle_counter as u32, // cycle
                                0xf11 => ret_val = 0, // vendor ID, will come up later on,
                                _ => {
                                    ret_val = 0; // safe default
                                }
                            }

                            // update
                            match funct3 {
                                1 => write_val = rs1, //CSRRW
                                2 => write_val = ret_val | rs1, //CSRRS
                                3 => write_val = ret_val & !rs1, //CSRRC
                                5 => write_val = rs1_as_imm, //CSRRWI
                                6 => write_val = ret_val | rs1_as_imm, //CSRRSI
                                7 => write_val = ret_val & !rs1_as_imm, //CSRRCI
                                _ => {}
                            }

                            match csr_number {
                                0x180 => {
                                    // satp
                                    mmu.write_sapt(write_val, current_privilege_mode, &mut trap);
                                    if trap != 0 {
                                        break 'cycle_block;
                                    }
                                },
                                0x300 => self.machine_mode_trap_data.state.status = write_val, // mstatus
                                0x304 => self.machine_mode_trap_data.state.ie = write_val, // mie
                                0x305 => self.machine_mode_trap_data.setup.tvec = write_val, // mtvec
                                0x340 => self.machine_mode_trap_data.handling.scratch = write_val, // mscratch
                                0x341 => self.machine_mode_trap_data.handling.epc = write_val, // mepc
                                0x342 => self.machine_mode_trap_data.handling.cause = write_val, // mcause
                                0x343 => self.machine_mode_trap_data.handling.tval = write_val, // mtval
                                0x344 => self.machine_mode_trap_data.state.ip = write_val, // mip
                                _ => {
                                    trap = TrapReason::IllegalInstruction.as_register_value();
                                    break 'cycle_block;
                                }
                            }

                            // and writeback
                        } else if funct3 == 0b000 {
                            // SYSTEM
                            rd = 0;
                            // mainly we support WFI, MRET, ECALL and EBREAK
                            if csr_number == 0x105 {
                                // WFI
                                MStatusRegister::set_mie(&mut self.machine_mode_trap_data.state.status);
                                self.extra_flags.set_wait_for_interrupt_bit();
                                self.pc = pc.wrapping_add(4u32);
                                println!("WFI");
                                return;
                            } else if csr_number & 0xff == 0x02 {
                                // MRET
                                let existing_mstatus = self.machine_mode_trap_data.state.status;

                                // let existing_flags = self.extra_flags;
                                // let previous_virtualization_bit = MStatusRegister::mprv_aligned_bit(existing_mstatus);

                                let previous_privilege = MStatusRegister::mpp(existing_mstatus);
                                // MRET then in mstatus/mstatush sets MPV=0, MPP=0,
                                // MIE=MPIE, and MPIE=1. Lastly, MRET sets the privilege mode as previously determined, and
                                // sets pc=mepc.
                                MStatusRegister::clear_mpp(&mut self.machine_mode_trap_data.state.status);
                                MStatusRegister::clear_mprv(&mut self.machine_mode_trap_data.state.status);
                                let mpie = MStatusRegister::mpie_aligned_bit(self.machine_mode_trap_data.state.status);
                                MStatusRegister::set_mie_to_value(&mut self.machine_mode_trap_data.state.status, mpie);
                                MStatusRegister::set_mpie(&mut self.machine_mode_trap_data.state.status);

                                // set privilege
                                self.extra_flags.set_mode_raw(Mode::User.as_register_value() | previous_privilege);
                                pc = self.machine_mode_trap_data.handling.epc.wrapping_sub(4u32);
                            } else {
                                match csr_number {
                                    0 => {
                                        // ECALL
                                        trap = match current_privilege_mode {
                                            Mode::Machine => TrapReason::EnvironmentCallFromMMode.as_register_value(),
                                            Mode::User => TrapReason::EnvironmentCallFromUMode.as_register_value(),
                                            _ => TrapReason::IllegalInstruction.as_register_value(),
                                        };

                                        break 'cycle_block;
                                    },
                                    1 => {
                                        // EBREAK
                                        trap = TrapReason::Breakpoint.as_register_value();
                                        break 'cycle_block;
                                    },
                                    _ => {
                                        trap = TrapReason::IllegalInstruction.as_register_value();
                                        break 'cycle_block;
                                    }
                                }
                            }
                        } else {
                            trap = TrapReason::IllegalInstruction.as_register_value();
                            break 'cycle_block;
                        }
                    },
                    0b00101111 => {
                        // RV32A, explicitly not supported
                        trap = TrapReason::IllegalInstruction.as_register_value();
                        break 'cycle_block;
                    },
                    _ => {
                        // any other instruction
                        trap = TrapReason::IllegalInstruction.as_register_value();
                        break 'cycle_block;
                    }
                }

                // If there was a trap, do NOT allow register writeback.
                debug_assert_eq!(trap, 0);
                // println!("Set x{:02} = 0x{:08x}", rd, ret_val);
                self.set_register(rd, ret_val);

                // traps below will update PC themself, so it only happens if we have NO trap
                pc = pc.wrapping_add(4u32);
            }
        }

        // Handle traps and interrupts.
        if trap > 0 {
            if trap & INTERRUPT_MASK != 0 {
                // interrupt, not a trap. Always machine level in our system
                self.machine_mode_trap_data.handling.cause = trap;
                self.machine_mode_trap_data.handling.tval = 0;
                pc = pc.wrapping_add(4u32); // PC points to where the PC will return!
            } else {
                // actually a trap
                println!(
                    "Trap reason = {:?} at pc = 0x{:08x}",
                    TrapReason::from_register_value(trap),
                    pc
                );
                self.machine_mode_trap_data.handling.cause = trap;
                // TODO: here we have a freedom of what to put into tval. We place opcode value now, because PC will be placed into EPC below
                self.machine_mode_trap_data.handling.tval = instr;
            }
            // println!("Trapping at pc = 0x{:08x} into PC = 0x{:08x}. MECP is set to 0x{:08x}", pc, self.machine_mode_trap_data.setup.tvec, pc);
            // self.pretty_dump();
            // self.stack_dump(memory, mmu);

            self.machine_mode_trap_data.handling.epc = pc;
            // update machine status register to reflect previous privilege

            // On an interrupt, the system moves current MIE into MPIE
            let mie = MStatusRegister::mie_aligned_bit(self.machine_mode_trap_data.state.status);
            MStatusRegister::set_mpie_to_value(&mut self.machine_mode_trap_data.state.status, mie);

            // go to trap vector
            pc = self.machine_mode_trap_data.setup.tvec;

            // Enter machine mode
            self.extra_flags.set_mode(Mode::Machine);
        }

        self.pc = pc;

        // for debugging
        self.sapt = mmu.read_sapt(current_privilege_mode, &mut trap);

        println!("end of cycle: PC = 0x{:08x}, trap = 0x{:08x}, interrupt = {:?}", self.pc, trap, trap & INTERRUPT_MASK != 0);
    }

    pub fn pretty_dump(&self) {
        println!(
            "PC = 0x{:08x}, RA = 0x{:08x}, SP = 0x{:08x}, GP = 0x{:08x}",
            self.pc, self.registers[1], self.registers[2], self.registers[3]
        );
        for chunk in self.registers.iter().enumerate().array_chunks::<4>() {
            for (idx, reg) in chunk.iter() {
                print!("x{:02} = 0x{:08x}, ", idx, reg);
            }
            println!("");
        }
    }

    pub fn stack_dump<
        'a,
        M: MemorySource,
        MTR: MemoryAccessTracer,
        MMU: MMUImplementation<M, MTR, AuxilarySource = MemoryImplementation<M, MTR>>,
    >(
        &'a self,
        memory: &'a mut MemoryImplementation<M, MTR>,
        mmu: &'a mut MMU,
    ) {
        let sp = self.registers[2];
        let mut trap = 0;
        let current_privilege_mode = self.extra_flags.get_current_mode();
        for i in 0..10 {
            for j in 0..4 {
                let offset = i * 4 + j;
                let addr = sp.wrapping_add(offset * 4);
                let addr = mmu.map_virtual_to_physical(
                    addr,
                    current_privilege_mode,
                    AccessType::Load,
                    memory,
                    &mut trap,
                );
                let value = memory.read(addr, 4, AccessType::Load, &mut trap);
                print!("SP-{} = 0x{:08x}, ", offset * 4, value);
            }
            println!("");
        }
    }
}
