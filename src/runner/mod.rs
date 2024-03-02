use crate::abstractions::memory::MemoryAccessTracerImpl;
use crate::abstractions::non_determinism::QuasiUARTSource;
use crate::cycle::state::StateTracer;
use crate::mmu::NoMMU;
use crate::{abstractions::memory::VectorMemoryImpl, cycle::state::RiscV32State};

pub const DEFAULT_ENTRY_POINT: u32 = 0x01000000;
pub const CUSTOM_ENTRY_POINT: u32 = 0;

pub fn run_simple_simulator(os_image: Vec<u8>, cycles: usize) {
    let mut state = RiscV32State::initial(DEFAULT_ENTRY_POINT);

    assert_eq!(os_image.len() % 4, 0);
    dbg!(os_image.len() / 4);

    let mut memory = VectorMemoryImpl::new_for_byte_size(1 << 32); // use full RAM
    for (word, dst) in os_image
        .array_chunks::<4>()
        .zip(memory.inner[((DEFAULT_ENTRY_POINT / 4) as usize)..].iter_mut())
    {
        *dst = u32::from_le_bytes(*word);
    }

    let mut non_determinism_source = QuasiUARTSource::default();
    let mut memory_tracer = MemoryAccessTracerImpl::new();
    let mut mmu = NoMMU { sapt: 0 };

    for cycle in 0..cycles {
        // state.pretty_dump();
        state.cycle(
            &mut memory,
            &mut memory_tracer,
            &mut mmu,
            &mut non_determinism_source,
            cycle as u32,
        );
    }
}

pub fn run_simulator_with_traces(
    os_image: Vec<u8>,
    cycles: usize,
) -> (StateTracer, MemoryAccessTracerImpl) {
    let mut state = RiscV32State::initial(CUSTOM_ENTRY_POINT);
    let mut state_tracer = StateTracer::new();
    state_tracer.insert(0, state);

    assert_eq!(os_image.len() % 4, 0);

    let mut memory = VectorMemoryImpl::new_for_byte_size(1 << 32); // use full RAM
    for (word, dst) in os_image
        .array_chunks::<4>()
        .zip(memory.inner[((CUSTOM_ENTRY_POINT / 4) as usize)..].iter_mut())
    {
        *dst = u32::from_le_bytes(*word);
    }

    let mut mmu = NoMMU { sapt: state.sapt };
    let mut non_determinism_source = QuasiUARTSource::default();
    let mut memory_tracer = MemoryAccessTracerImpl::new();

    for i in 0..cycles {
        // state.pretty_dump();
        state.cycle(
            &mut memory,
            &mut memory_tracer,
            &mut mmu,
            &mut non_determinism_source,
            i as u32,
        );
        println!("mtvec: {:?}", state.machine_mode_trap_data.setup.tvec);
        state_tracer.insert(i + 1, state);
    }

    (state_tracer, memory_tracer)
}
