use std::collections::VecDeque;

use crate::abstractions::MemoryImplementation;
use crate::mmio::quasi_uart::QuasiUART;
use crate::mmio::MMIOSource;
use crate::{
    abstractions::memory::VectorMemoryImpl, cycle::state::RiscV32State, mmio::MMIOImplementation,
};

pub const DEFAULT_ENTRY_POINT: u32 = 0x01000000;

pub fn run_simple_simulator(os_image: Vec<u8>, cycles: usize) {
    let mut state = RiscV32State::initial(DEFAULT_ENTRY_POINT);

    assert_eq!(os_image.len() % 4, 0);

    let mut memory = VectorMemoryImpl::new_for_byte_size(1 << 32); // use full RAM
    for (word, dst) in os_image
        .array_chunks::<4>()
        .zip(memory.inner[((DEFAULT_ENTRY_POINT / 4) as usize)..].iter_mut())
    {
        *dst = u32::from_le_bytes(*word);
    }

    // let mut mmu = SimpleMMU::default();

    use crate::mmu::NoMMU;
    let mut mmu = NoMMU;

    let quasi_uart = QuasiUART {
        oracle: VecDeque::new(),
        buffer: Vec::new(),
    };
    let quasi_uart = Box::new(quasi_uart) as Box<dyn MMIOSource>;
    let mut sources = [quasi_uart];
    let mut mmio = MMIOImplementation::<1>::construct(&mut sources);

    let mut memory = MemoryImplementation {
        memory_source: memory,
        tracer: (),
        timestamp: 0u32,
    };

    for _ in 0..cycles {
        // state.pretty_dump();
        state.cycle(&mut memory, &mut mmu, &mut mmio);
    }
}
