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
            // cycles,
            cycles: 561,
            profiler: Profiler::new(path_sym, config.get_frequency_recip()),
        }
    }

    pub(crate) fn run(&mut self) {

        for cycle in 0 .. self.cycles as usize {
            println!("***********************");
            self.profiler.pre_cycle(
                &mut self.state,
                &mut self.memory_source,
                &mut self.memory_tracer,
                &mut self.mmu,
                cycle as u32,
            );

            println!("**** Running cycle ****");

            self.state.cycle(
                &mut self.memory_source,
                &mut self.memory_tracer,
                &mut self.mmu,
                &mut self.non_determinism_source,
                cycle as u32,
            );

            if cycle >= 482 { // fmt messing with s0
            // if cycle >= 558 {
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
            }
        }

        println!("stacktrace: {:#?}", self.profiler.stacktraces);
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
            // frequency_recip: 10
            // frequency_recip: 560
            frequency_recip: 1
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

    use addr2line::{gimli::{self, DebugInfoOffset, Dwarf, EndianSlice, RunTimeEndian, SectionId, UnitOffset}, Context, Frame, LookupResult, SplitDwarfLoad};
    use memmap2::Mmap;
    use object::{File, Object, ObjectSection};
    use addr2line::LookupContinuation;

    use crate::{abstractions::{mem_read, memory::{MemoryAccessTracer, MemorySource}, non_determinism::NonDeterminismCSRSource}, cycle::{state::RiscV32State, status_registers::TrapReason}, mmu::MMUImplementation, qol::PipeOp as _};

    pub(crate) struct Profiler {
        symbol_info: Option<SymbolInfo>,
        frequency_recip: u32,
        pub stacktraces: StacktraceSet,
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

        pub(crate) fn pre_cycle<MS, MT, MMU>(
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
            if cycle % self.frequency_recip == 0 
            { 
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

            println!("--- building callstack ---");
            println!("pc: {:08x}", state.pc);
            println!("cycle: {}", cycle);

            // Saved frames
            loop {
                println!("--- iter ---");
                if fp == 0 { break; }

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

                let next = mem_read(
                    memory_source,
                    memory_tracer,
                    fpp - 8,
                    size_of::<u32>() as u32,
                    crate::abstractions::memory::AccessType::MemLoad,
                    cycle,
                    &mut trap); 

                println!("addr: {:08x}", addr);
                println!("next: {:08x}", next);
                // if state.pc == 183156 { panic!("found"); }
                if addr == 1 { panic!("addr is 1, pc: {}", state.pc); }
                if addr == 1 { break; }
                if addr == 0 { break; }
                // assert_ne!(0, addr);

                // Subbing one instruction because the frame's return address point to instruction
                // that follows the call, not the call itself. In case of inlining this can be
                // several frames away.
                let addr = addr - 4;

                callstack.push(addr as u64);

                fp = next;
            }

            let mut stackframes = Vec::with_capacity(8);
            let mut frame_refs = Vec::new();

            println!("--- frame names ---");

            for addr in callstack {
                let frames = symbol_info.get_address_frames(addr);
                // symbol_info.playground(addr);

                for frame in frames {
                    println!("  :: {}", frame.function.as_ref().unwrap().demangle().unwrap());
                    let offset = frame.dw_die_offset.unwrap();
                    stackframes.push(offset);
                    // symbol_info.is_address_traceable(addr, &frame);
                    // symbol_info.inspect_frame(addr, &frame);
                    frame_refs.push(frame);

                }
            }

            if stackframes.len() == 0 { 
                println!("No frames found, skipping.");
                return; 
            }

            if symbol_info.is_address_traceable(state.pc as u64, &frame_refs[0]) == false {
                println!("Non traceable location, skipping.");
                return;
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
        dwarf: Dwarf<EndianSlice<'static, RunTimeEndian>>,
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

            let dwarf = 
                addr2line::gimli::Dwarf::load(load_section).unwrap();

            SymbolInfo {
                map,
                object,
                dwarf,
                ctx,
                frame_names: HashMap::new(),
            }
        }

        fn is_address_traceable(&self, address: u64, frame: &Frame<'_, EndianSlice<'_, RunTimeEndian>>) -> bool {

            let x = self.ctx.find_dwarf_and_unit(address).skip_all_loads();
            if x.is_none() { return false; }

            let (dw, unit) = x.unwrap();

            let mut cursor = unit.entries_at_offset( frame.dw_die_offset.unwrap()).unwrap();
            cursor.next_entry();
            let die = cursor.current().unwrap();

            if false { // print attrs

                let mut attrs = die.attrs();

                while let Ok(Some(attr)) = attrs.next() {

                    println!("   {:x?} -> {:x?}", attr.name(), attr.value());

                    match attr.name() {
                        gimli::DW_AT_linkage_name | gimli::DW_AT_name => {

                            let n = attr.value();

                            match n {
                                gimli::AttributeValue::DebugStrRef(n) => {
                                    let s = dw.string(n).unwrap();
                                    println!("      value: {}", s.to_string_lossy());
                                },
                                _ => {}
                            }
                        },

                        gimli::DW_AT_frame_base => {
                            match attr.value()  {
                                gimli::AttributeValue::Exprloc(ex) => {
                                    println!("expr decode");
                                    let mut ops = ex.operations(unit.encoding());

                                    while let Ok(Some(op)) = ops.next() {
                                        println!("op: {:?}", op);
                                    }
                                },
                                _ => {}
                            }
                        },
                        gimli::DW_AT_specification => {
                            match attr.value() {
                                gimli::AttributeValue::UnitRef(other_offset) => {
                                    let mut cursor = unit.entries_at_offset(other_offset).unwrap();
                                    cursor.next_entry();
                                    let die2 = cursor.current().unwrap();

                                    let mut attrs = die2.attrs();

                                    while let Ok(Some(attr)) = attrs.next() {

                                        println!("   {:x?} -> {:x?}", attr.name(), attr.value());

                                        match attr.name() {
                                            gimli::DW_AT_linkage_name | gimli::DW_AT_name => {

                                                let n = attr.value();

                                                match n {
                                                    gimli::AttributeValue::DebugStrRef(n) => {
                                                        let s = dw.string(n).unwrap();
                                                        println!("      value: {}", s.to_string_lossy());
                                                    },
                                                    _ => {}
                                                }
                                            },
                                            _ => {}
                                        };
                                    }
                                },
                                _ => {}
                            }
                        },
                        _ => {}
                    }
                }

            }

            let (line_program, sequences) = unit.line_program.clone().unwrap().clone().sequences().unwrap();
            
            let mut found = false;
            let mut prologue_ended = false;
            let mut epilogue_began = false;

            for s in sequences {
                if address >= s.start && address < s.end {
                    println!("found seq: {:x} -> {:x}", s.start, s.end);

                    let mut sm = line_program.resume_from(&s);

                    while let Ok(Some((h, r))) = sm.next_row() {
                        if r.address() > address {
                            // Our address was included in the previous row.
                            break;
                        }

                        let line_num = match r.line() {
                            Some(r) => r.get(),
                            None => 0
                        };
                        // println!("row addr {:08x}, line {}, stmt {}, prol_end {}, epi_start {},",
                        //     r.address(), line_num, r.is_stmt(), r.prologue_end(), r.epilogue_begin());

                        prologue_ended |= r.prologue_end();
                        epilogue_began |= r.epilogue_begin();

                    }

                    found = true;
                    break;
                }

            }

            if found == false {
                panic!("Address not in frame.");
            }
            
            prologue_ended && epilogue_began == false
        }

        fn inspect_frame(&self, address: u64, frame: &Frame<'_, EndianSlice<'_, RunTimeEndian>>) {

            let x = self.ctx.find_dwarf_and_unit(address).skip_all_loads();
            if x.is_none() { return; }

            let (dw, unit) = x.unwrap();

            let mut cursor = unit.entries_at_offset( frame.dw_die_offset.unwrap()).unwrap();
            cursor.next_entry();
            let die = cursor.current().unwrap();


                let mut attrs = die.attrs();

                while let Ok(Some(attr)) = attrs.next() {

                    println!("   {:x?} -> {:x?}", attr.name(), attr.value());

                    match attr.name() {
                        gimli::DW_AT_linkage_name | gimli::DW_AT_name => {

                            let n = attr.value();

                            match n {
                                gimli::AttributeValue::DebugStrRef(n) => {
                                    let s = dw.string(n).unwrap();
                                    println!("      value: {}", s.to_string_lossy());
                                },
                                _ => {}
                            }
                        },

                        gimli::DW_AT_frame_base => {
                            match attr.value()  {
                                gimli::AttributeValue::Exprloc(ex) => {
                                    println!("expr decode");
                                    let mut ops = ex.operations(unit.encoding());

                                    while let Ok(Some(op)) = ops.next() {
                                        println!("op: {:?}", op);
                                    }
                                },
                                _ => {}
                            }
                        },
                        gimli::DW_AT_specification | gimli::DW_AT_abstract_origin => {
                            match attr.value() {
                                gimli::AttributeValue::UnitRef(other_offset) => {
                                    let mut cursor = unit.entries_at_offset(other_offset).unwrap();
                                    cursor.next_entry();
                                    let die2 = cursor.current().unwrap();

                                    let mut attrs = die2.attrs();

                                    while let Ok(Some(attr)) = attrs.next() {

                                        println!("      {:x?} -> {:x?}", attr.name(), attr.value());

                                        match attr.name() {
                                            gimli::DW_AT_linkage_name | gimli::DW_AT_name => {

                                                let n = attr.value();

                                                match n {
                                                    gimli::AttributeValue::DebugStrRef(n) => {
                                                        let s = dw.string(n).unwrap();
                                                        println!("         value: {}", s.to_string_lossy());
                                                    },
                                                    _ => {}
                                                }
                                            },
                                            _ => {}
                                        };
                                    }
                                },
                                _ => {}
                            }
                        },
                        // gimli::DW_AT_abstract_origin => {
                        //     match attr.value() {
                        //         gimli::AttributeValue::UnitRef(r) => {
                        //             let mut cursot = unit.entries_at_offset(r).unwrap();
                        //         },
                        //         _ => {}
                        //     }
                        // },
                        _ => {}
                    }


            }
            let (line_program, sequences) = unit.line_program.clone().unwrap().clone().sequences().unwrap();

            for s in sequences {
                if address >= s.start && address < s.end {
                    println!("found seq: {:x} -> {:x}", s.start, s.end);

                    let mut sm = line_program.resume_from(&s);

                    while let Ok(Some((h, r))) = sm.next_row() {

                        let line_num = match r.line() {
                            Some(r) => r.get(),
                            None => 0
                        };
                        println!("row addr {:08x}, line {}, stmt {}, prol_end {}, epi_start {},",
                            r.address(), line_num, r.is_stmt(), r.prologue_end(), r.epilogue_begin());


                    }

                }

            }

        }

        fn playground(&self, address: u64) -> () {
            let r = self.ctx.find_dwarf_and_unit(address).skip_all_loads();
            if r.is_none() { return; }
            let (dw, unit) = r.unwrap();
            
            println!("dwarf: unit name {}", unit.name.unwrap().to_string_lossy());

            let mut count = 0;
            let mut e_cursor = unit.entries();
            while let Some(entry) = e_cursor.next_entry().unwrap() {
                let die = e_cursor.current();
                if die.is_none() { continue; }
                let die = die.unwrap();

                if matches!(die.tag(), gimli::DW_TAG_subprogram) == false
                && matches!(die.tag(), gimli::DW_TAG_inlined_subroutine) == false{
                    continue;
                }

                count += 1;

                let tag_n = match die.tag() {
                    gimli::DW_TAG_subprogram =>
                        "DW_TAG_subprogram".to_owned(),
                    gimli::DW_TAG_inlined_subroutine =>
                        "DW_TAG_inlined_subroutine".to_owned(),
                    gimli::DW_TAG_variable =>
                        "DW_TAG_variable".to_owned(),
                    gimli::DW_TAG_formal_parameter =>
                        "DW_TAG_formal_parameter".to_owned(),
                    otherwise =>
                        format!("{:x?}", otherwise)
                };
                println!("dwarf: die: #{}, offset {:?}, tag {:?}", count, die.offset(), tag_n);

                let mut attrs = die.attrs();

                while let Ok(Some(attr)) = attrs.next() {

                    println!("   {:x?} -> {:x?}", attr.name(), attr.value());

                    match attr.name() {
                        gimli::DW_AT_linkage_name | gimli::DW_AT_name => {
                            
                            let n = attr.value();

                            match n {
                                gimli::AttributeValue::DebugStrRef(n) => {
                                    let s = dw.string(n).unwrap();
                                    println!("      value: {}", s.to_string_lossy());
                                },
                                _ => {}
                            }
                        },
                        _ => {}
                    }
                }

                // let line_program = unit.line_program.as_ref().unwrap();
            let (line_program, sequences) = unit.line_program.clone().unwrap().clone().sequences().unwrap();

                for s in sequences {
                    println!("seq: {:x} -> {:x}", s.start, s.end);
                }

                // dw.debug_line.program(offset, address_size, comp_dir, comp_name), address_size, comp_dir, comp_name)
                
            }

            
            // let ranges = dw.ranges(&unit, );

            panic!("stop!");
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

            while let Ok(Some(frame)) = frames.next() {
                // let frame = frame.unwrap();

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
    
    #[derive(Debug)]
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
            assert_ne!(0, frames.len());
            Self { frames }
        }
    }

    #[derive(Debug)]
    pub(crate) struct StacktraceSet {
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

        
        fn absorb<S, N>(&mut self, stacktrace: Stacktrace, names: N) 
        where 
            S: Deref<Target = str>, 
            N: Iterator<Item = (UnitOffset<usize>, S)> 
        {
            for (o, n) in names {
                self.names.entry(o).or_insert(n.deref().to_owned());
            }
            self.traces.entry(stacktrace).and_modify(|x| *x += 1).or_insert(1);
        }
    }
}
