use std::{env, path::{Path, PathBuf}};

use crate::{abstractions::{memory::{MemoryAccessTracer, MemorySource}, non_determinism::NonDeterminismCSRSource}, cycle::state::RiscV32State, mmu::MMUImplementation};

use self::profiling::Profiler;

pub(crate) struct Simulator<MS, MT, MMU, ND>
    where
    MS: MemorySource,
    MT: MemoryAccessTracer,
    MMU: MMUImplementation<MS, MT>,
    ND: NonDeterminismCSRSource,
{
    memory_source: MS,
    memory_tracer: MT,
    mmu: MMU,
    non_determinism_source: ND,

    state: RiscV32State,
    cycles: usize,

    profiler: Profiler,

}

impl<MS, MT, MMU, ND> Simulator<MS, MT, MMU, ND>
where
    MS: MemorySource,
    MT: MemoryAccessTracer,
    MMU: MMUImplementation<MS, MT>,
    ND: NonDeterminismCSRSource,
{
    pub(crate) fn new<P: AsRef<Path>>(
        config: SimulatorConfig,
        path_sym: Option<P>,
        memory_source: MS,
        memory_tracer: MT,
        mmu: MMU,
        non_determinism_source: ND,
        entry_point: u32,
        cycles: usize,
    ) -> Self 
    {
        Self {
            memory_source,
            memory_tracer,
            mmu,
            non_determinism_source,
            state: RiscV32State::initial(entry_point),
            cycles,
            profiler: Profiler::new(path_sym, config.get_frequency_recip()),
        }
    }

    // pub(crate) fn add_profiler<P: AsRef<std::path::Path>>(
    //     &mut self,
    //     symbol_path: P,
    //     frequency_recip: usize,
    // ) {
    //     self.profiler = Profiler::new(symbol_path, frequency_recip);
    // }

    pub(crate) fn run(&mut self) {
        for cycle in 0 .. self.cycles {
            // state.pretty_dump();
            self.state.cycle(
                &mut self.memory_source,
                &mut self.memory_tracer,
                &mut self.mmu,
                &mut self.non_determinism_source,
                cycle as u32,
            );
        }
    }

    pub(crate) fn deconstruct(self) -> ND {
        self.non_determinism_source
    }
}

pub(crate) struct SimulatorConfig {
    frequency_recip: usize,
}

impl SimulatorConfig {
    pub(crate) fn new() -> Self {
        SimulatorConfig { 
            frequency_recip: 1000
        }
    }

    pub(crate) fn get_frequency_recip(&self) -> usize {
        env::var("RISCV_SIM_FREQUENCY_RECIP")
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(self.frequency_recip)
    }
}

mod profiling {
    use crate::qol::PipeOp as _;

    pub(crate) struct Profiler {
        symbol_path: Option<std::path::PathBuf>,
        frequency_recip: usize,
    }

    impl Profiler {
        pub(crate) fn new<P: AsRef<std::path::Path>>(
            symbol_path: Option<P>,
            frequency_recip: usize,
        ) -> Self {
            match symbol_path {
                Some(p) => 
                    Self {
                        symbol_path: p.as_ref().to_owned().to(Some),
                        frequency_recip,
                    },
                None =>
                    Self {
                        symbol_path: None,
                        frequency_recip: usize::MAX, // One chunk on run.
                    }
            }
        }

        pub(crate) fn run<F: Fn()>(&mut self, f: F, cycles: u32) {
            let cycles = cycles as usize;

            for _ in 0 .. cycles / self.frequency_recip {
                for _ in 0 .. self.frequency_recip {
                    f();
                }
                self.collect_stacktrace();
            }

            for _ in 0 .. cycles % self.frequency_recip {
                f();
            }
        }

        fn collect_stacktrace(&mut self) {
        }
    }
}
