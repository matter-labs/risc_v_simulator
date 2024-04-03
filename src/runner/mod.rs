use std::path::Path;

use crate::abstractions::memory::MemoryAccessTracerImpl;
use crate::abstractions::non_determinism::NonDeterminismCSRSource;
use crate::abstractions::non_determinism::QuasiUARTSource;
use crate::cycle::state::StateTracer;
use crate::mmu::NoMMU;
use crate::sim::Simulator;
use crate::sim::SimulatorConfig;
use crate::{abstractions::memory::VectorMemoryImpl, cycle::state::RiscV32State};

pub const DEFAULT_ENTRY_POINT: u32 = 0x01000000;
pub const CUSTOM_ENTRY_POINT: u32 = 0;

pub fn run_simple_simulator(
    config: SimulatorConfig,
) {
    run_simple_with_entry_point(config)
}

pub fn run_simple_with_entry_point(
    config: SimulatorConfig,
) 
{
    run_simple_with_entry_point_and_non_determimism_source(
        config,
        QuasiUARTSource::default(),
    );
}

pub fn run_simple_with_entry_point_and_non_determimism_source<
    S: NonDeterminismCSRSource<VectorMemoryImpl>> (
    config: SimulatorConfig,
    non_determinism_source: S,
) -> S 
{
    let state = RiscV32State::initial(config.entry_point);
    let memory_tracer = MemoryAccessTracerImpl::new();
    let mmu = NoMMU { sapt: 0 };

    let mut memory = VectorMemoryImpl::new_for_byte_size(1 << 32); // use full RAM
    memory.load_image(config.entry_point, read_bin(&config.bin_path).into_iter());

    let mut sim = Simulator::new(
        config,
        state,
        memory,
        memory_tracer,
        mmu,
        non_determinism_source
    );

    sim.run(|_,_| {}, |_,_| {});

    sim.non_determinism_source
}

pub fn run_simulator_with_traces(
    config: SimulatorConfig,
) -> (StateTracer, MemoryAccessTracerImpl) 
{
    let state = RiscV32State::initial(CUSTOM_ENTRY_POINT);
    let memory_tracer = MemoryAccessTracerImpl::new();
    let mmu = NoMMU { sapt: state.sapt };
    let non_determinism_source = QuasiUARTSource::default();

    let mut memory = VectorMemoryImpl::new_for_byte_size(1 << 32); // use full RAM
    memory.load_image(config.entry_point, read_bin(&config.bin_path).into_iter());

    let mut sim = Simulator::new(
        config,
        state,
        memory,
        memory_tracer,
        mmu,
        non_determinism_source
    );

    let mut state_tracer = StateTracer::new();
    state_tracer.insert(0, sim.state);

    sim.run(|_,_| {}, |sim, cycle| {
        println!("mtvec: {:?}", sim.state.machine_mode_trap_data.setup.tvec);
        state_tracer.insert(cycle + 1, sim.state);
    });
    
    (state_tracer, sim.memory_tracer)
}

fn read_bin<P: AsRef<Path>>(path: P) -> Vec<u8> {
    let mut file = std::fs::File::open(path).expect("must open provided file");
    let mut buffer = vec![];
    std::io::Read::read_to_end(&mut file, &mut buffer).expect("must read the file");
    
    assert_eq!(buffer.len() % 4, 0);
    dbg!(buffer.len() / 4);

    buffer
}
