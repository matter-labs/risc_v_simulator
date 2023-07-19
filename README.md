# Purpose

RISC-V 32 bit basic processor simulator for ZK purposes, with ZK-specific wrappers and functions. It supports only RV32IM basic set, and machine + use mode only (no atomic set). Inspired by the `https://github.com/cnlohr/mini-rv32ima`, but Rustified. Can be used for rough system overview while ZK circuit is being written and tested. Not that's it's not intended, and will not be a cycle-precise simulator for any hardware RISC-V processor.

The intention of the system to eventually run even untrusted user programs in "native" (RISC-V 32 or 64 bit) code (that requires good isolation), but so for a start we will do only machine mode.

Also note that circuit implementation will be radically different in many places implementation wise (e.g. we can use memory for registers actually), but both of them will be consistent. 

## Important notes

- Even though SATP register is there, and settable, and usermore is supported, for now it's intended to be used as machine mode only! 
- This implementation is 32-bit, but in practice it'll be 64 bit because circuit size doesn't so linearly affect the circuit size, and 64 bit instructions are beneficial for the software that we would like to run on it. And so memory translation scheme would change for SV39.
- Even though unaligned memory access is a pain in ZK, in practice we have too much byte accesses all over the places, and though we could just work them out through exception handling, we pay small price and allow unaligned access!
- `bin` folder contains an example of how to run the simulator
- Non-deterministic ZK nature is implemented by quasi-UART (to be precise - just word-consuming/replying device) that is an "oracle" to ask for any witness that a programm running on the simulator may want. Writing to there is only intented for debug logging, but may be there are other good use cases
- The ZK part will prove full execution trace without explicit/implicit breaks or continuations, memory dumps, etc. Just assume that your single core processors runs
- MMIO for timer is not yet implemented, but it'll most likely be placed somewhere near the quasi-UART address
- It also means no timer interrupts yet
- It's expected that times will be the only interrupt actually for now
- Interface for MMU and corresponding memory access implementation is actually not too correct and not good for the circuit correspondence too. We may have unaligned loads that cross the page boundaries, so at worst we would need 2 independent memory translations per read/write. In any case it's should not be used for now (so don't write to SATP and don't go usermode!)

## How to run

We also have `zk_os` repo open with basic examples and logic, so it's possible to write Rust `no-std` code and just launch it. Start of the executable code is expected to be mapped directly into `DEFAULT_ENTRY_POINT: u32 = 0x01000000;` and execution starts from there. Note that loading of the initial (fixed) memory content is free in ZK part in our case (for reasonable sizes), so OS or app image can be expected to be always loaded by default. If you need to load more code you can use quasi-UART to read it from "oracle" and do whatever you want with it (remember - machine mode is there for you!)