use std::{env, path::{Path, PathBuf}};

use crate::{abstractions::{memory::{MemoryAccessTracer, MemorySource}, non_determinism::NonDeterminismCSRSource}, cycle::state::RiscV32State, mmu::MMUImplementation};

use self::diag::Profiler;

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

    pub(crate) fn run(&mut self) {

        for cycle in 0 .. self.cycles as usize {
            self.state.cycle(
                &mut self.memory_source,
                &mut self.memory_tracer,
                &mut self.mmu,
                &mut self.non_determinism_source,
                cycle as u32,
            );

            self.profiler.cycle(
                &mut self.state,
                &mut self.memory_source,
                &mut self.memory_tracer,
                &mut self.mmu,
                cycle as u32,
            );
        }
    }

    pub(crate) fn deconstruct(self) -> ND {
        self.non_determinism_source
    }
}

pub(crate) struct SimulatorConfig {
    frequency_recip: u32,
}

impl SimulatorConfig {
    pub(crate) fn new() -> Self {
        SimulatorConfig { 
            frequency_recip: 10
        }
    }

    pub(crate) fn get_frequency_recip(&self) -> u32 {
        env::var("RISCV_SIM_FREQUENCY_RECIP")
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(self.frequency_recip)
    }
}

mod diag {
    use std::{ collections::HashMap, hash::Hasher, mem::size_of, ops::Deref, path::{Path, PathBuf}, rc::Rc, sync::Arc};

    use addr2line::{gimli::{Dwarf, EndianSlice, RunTimeEndian, SectionId, UnitOffset}, Context, Frame, LookupResult, SplitDwarfLoad};
    use memmap2::Mmap;
    use object::{File, Object, ObjectSection};
    use addr2line::LookupContinuation;

    use crate::{abstractions::{mem_read, memory::{MemoryAccessTracer, MemorySource}, non_determinism::NonDeterminismCSRSource}, cycle::{state::RiscV32State, status_registers::TrapReason}, mmu::MMUImplementation, qol::PipeOp as _};

    pub(crate) struct Profiler {
        symbol_info: Option<SymbolInfo>,
        frequency_recip: u32,
        stacktraces: StacktraceSet,
    }

    impl Profiler {
        pub(crate) fn new<P: AsRef<std::path::Path>>(
            symbol_path: Option<P>,
            frequency_recip: u32,
        ) -> Self {
            match symbol_path {
                Some(p) => 
                    Self {
                        symbol_info: Some(SymbolInfo::new(p)),
                        frequency_recip,
                        stacktraces: StacktraceSet::new(),
                    },
                None =>
                    Self {
                        symbol_info: None,
                        frequency_recip: u32::MAX, // Single chunk on run.
                        stacktraces: StacktraceSet::new(),
                    }
            }
        }

        // pub(crate) fn run<F: FnMut(u32)>(&mut self, mut f: F, cycles: u32) {
        //     let cycles = cycles as usize;
        //     let frequency_recip = self.frequency_recip as usize;
        //
        //     let cycles =
        //         (0 .. cycles)
        //         .step_by(frequency_recip)
        //         .map(|x| (x .. (x + frequency_recip).min(cycles)).into_iter());
        //
        //     for chunk in cycles {
        //         for cycle in chunk {
        //             println!("cycle {}", cycle);
        //             f(cycle as u32);
        //         }
        //         self.collect_stacktrace();
        //     }
        // }

        pub(crate) fn cycle<MS, MT, MMU>(
            &mut self,
            state: &RiscV32State,
            memory_source: &mut MS,
            memory_tracer: &mut MT,
            mmu: &mut MMU,
            cycle: u32)
        where
            MS: MemorySource,
            MT: MemoryAccessTracer,
            MMU: MMUImplementation<MS, MT>,
        {
            if cycle % self.frequency_recip == 0 { 
                self.collect_stacktrace(
                    state,
                    memory_source,
                    memory_tracer,
                    mmu,
                    cycle); 
            }
        }

        fn collect_stacktrace<MS, MT, MMU>(
            &mut self,
            state: &RiscV32State,
            memory_source: &mut MS,
            memory_tracer: &mut MT,
            mmu: &mut MMU,
            cycle: u32)
        where
            MS: MemorySource,
            MT: MemoryAccessTracer,
            MMU: MMUImplementation<MS, MT>,
        {
            let symbol_info = self.symbol_info.as_ref().expect("Symbols weren't provided.");

            let mut callstack = Vec::with_capacity(6);

            // Current frame
            callstack.push(state.pc as u64);

            let mut fp = state.registers[8];

            // Saved frames
            loop {
                let mut trap = TrapReason::NoTrap;

                let fpp = mmu.map_virtual_to_physical(
                    fp, 
                    crate::cycle::state::Mode::Machine,
                    crate::abstractions::memory::AccessType::MemLoad,
                    memory_source,
                    memory_tracer,
                    cycle,
                    &mut trap);

                let addr = mem_read(
                    memory_source,
                    memory_tracer,
                    fpp - 4,
                    size_of::<u32>() as u32,
                    crate::abstractions::memory::AccessType::MemLoad,
                    cycle,
                    &mut trap); 

                // Subbing one instruction because the frame's pc points to the
                // next instruction to execute.
                let addr = addr - 4;

                if addr == 0 { break; }

                let next = mem_read(
                    memory_source,
                    memory_tracer,
                    fpp - 8,
                    size_of::<u32>() as u32,
                    crate::abstractions::memory::AccessType::MemLoad,
                    cycle,
                    &mut trap); 

                callstack.push(addr as u64);

                if next == 0 { break; }
                fp = next;
            }

            let mut stackframes = Vec::with_capacity(8);
            // let mut names = Vec::with_capacity(8);
            let mut frame_refs = Vec::new();

            for addr in callstack {
                let frames = symbol_info.get_address_frames(addr);

                for frame in frames {
                    let offset = frame.dw_die_offset.unwrap();
                    stackframes.push(offset);
                    // names.push((offset, frame.function.as_ref().unwrap().demangle().unwrap().clone()));
                    frame_refs.push(frame);
                }
            }

            let stacktrace = Stacktrace::new(stackframes);

            self.stacktraces.absorb(stacktrace, frame_refs.iter().map(|x|{
                (x.dw_die_offset.unwrap(), x.function.as_ref().unwrap().demangle().unwrap())
            }));
        }


        // fn pack_stacktraces(&self) {
        //     self.symbol_info.
        // }
    }

    struct SymbolInfo {
        // Safety: Values must be dropped in the dependency order.
        ctx: Context<EndianSlice<'static, RunTimeEndian>>,
        // dwarf: Dwarf<EndianSlice<'static, RunTimeEndian>>, 
        object: object::File<'static>,
        map: Mmap,

        frame_names: HashMap<UnitOffset<usize>, String>,
    }

    impl SymbolInfo {
        fn new<P: AsRef<Path>>(path: P) -> Self {
            let x = std::fs::File::open(path).unwrap();
            let map = unsafe { memmap2::Mmap::map(&x).unwrap() };

            // Safety: map contains a raw pointer, so it is safe to move.
            let object = object::File::parse(&*map).unwrap();
            let object = unsafe { std::mem::transmute::<_, File<'static>>(object) };

            let endian = match object.is_little_endian() {
                true  => RunTimeEndian::Little,
                false => RunTimeEndian::Big
            };

            let load_section = |id: SectionId| -> Result<_, ()> {
                let name = id.name();

                match object.section_by_name(name) {
                    Some(section) => match section.uncompressed_data().unwrap() {
                        std::borrow::Cow::Borrowed(section) => Ok(EndianSlice::new(section, endian)),
                        std::borrow::Cow::Owned(_) => unreachable!("We're following the borrowed path."),
                    },
                    None => Ok(EndianSlice::new(&[][..], endian))
                }
            };

            let dwarf = 
                addr2line::gimli::Dwarf::load(load_section)
                .expect("Debug symbols could not be loaded.");

            let ctx = Context::from_dwarf(dwarf).unwrap();

            SymbolInfo {
                map,
                object,
                // dwarf,
                ctx,
                frame_names: HashMap::new(),
            }
        }

        fn get_address_frames<'a>(&self, address: u64) -> Vec<Frame<'a, EndianSlice<'a, RunTimeEndian>>> {
            let mut frames = self.ctx.find_frames(address);

            let mut frames = loop {
                match frames {
                    LookupResult::Output(r) => break r,
                    LookupResult::Load { load: _, continuation } => {

                        // Not using split load.
                        frames = continuation.resume(None);
                    }
                }
            }.unwrap();

            let mut r = Vec::with_capacity(8);

            while let Ok(frame) = frames.next() {
                let frame = frame.unwrap();

                // self.frame_names.entry(frame.dw_die_offset.unwrap()).or_insert_with(||
                // );

                // match self.frame_names.entry(frame.dw_die_offset.unwrap()) {
                //     entry @ std::collections::hash_map::Entry::Vacant(_) => {
                //         let x = frame.function.as_ref().unwrap().demangle().unwrap().deref().to_owned();
                //         entry.or_insert( x);
                //     },
                //
                //     _ => {}
                // }

                // Safety: The borrow checker assumes that the frame lives for 'const (derived from
                // `ctx` field in `Self`). The actual lifetime is the lifetime of `self`. So we're
                // adjusting the lifetime args in the return type accordingly.
                unsafe { r.push(std::mem::transmute(frame)) };
            }

            r
        }

        // fn get_frame(&self) {
        //     self.dwarf.
        // }
    }
    
    struct Stacktrace {
        frames: Vec<UnitOffset<usize>>
    }

    impl PartialEq for Stacktrace {
        fn eq(&self, other: &Self) -> bool {
            self.frames == other.frames
        }
    }

    impl Eq for Stacktrace {}

    impl std::hash::Hash for Stacktrace {
        fn hash<H: Hasher>(&self, state: &mut H) {
            for frame in &self.frames {
                frame.hash(state);
            }
        }
    }

    impl Stacktrace {
        pub(crate) fn new(frames: Vec<UnitOffset<usize>>) -> Self {
            Self { frames }
        }
    }

    struct StacktraceSet {
        names: HashMap<UnitOffset<usize>, String>,
        traces: HashMap<Stacktrace, usize>,
    }
    
    impl StacktraceSet {
        fn new() -> Self {
            Self {
                names: HashMap::new(),
                traces: HashMap::new()
            }
        }

        
        fn absorb<'a, S: Deref<Target = str>, N: Iterator<Item = (UnitOffset<usize>, S)>>(&mut self, stacktrace: Stacktrace, names: N) {
            for (o, n) in names {
                self.names.entry(o).or_insert(n.deref().to_owned());
            }
            self.traces.entry(stacktrace).and_modify(|x| *x += 1).or_insert(0);
        }
    }
}
