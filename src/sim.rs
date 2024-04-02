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

    profiler: Option<Profiler>,

}

impl<MS, MT, MMU, ND> Simulator<MS, MT, MMU, ND>
where
    MS: MemorySource,
    MT: MemoryAccessTracer,
    MMU: MMUImplementation<MS, MT>,
    ND: NonDeterminismCSRSource,
{
    pub(crate) fn new(
        config: SimulatorConfig,
        // path_sym: Option<P>,
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
            profiler: Profiler::new(config),
        }
    }

    pub(crate) fn run(&mut self) {

        for cycle in 0 .. self.cycles as usize {
            if let Some(profiler) = self.profiler.as_mut() {
                profiler.pre_cycle(
                    &mut self.state,
                    &mut self.memory_source,
                    &mut self.memory_tracer,
                    &mut self.mmu,
                    cycle as u32,
                );
            }

            self.state.cycle(
                &mut self.memory_source,
                &mut self.memory_tracer,
                &mut self.mmu,
                &mut self.non_determinism_source,
                cycle as u32,
            );
        }

        if let Some(profiler) = self.profiler.as_mut() {
            profiler.write_stacktrace();
        }
    }

    pub(crate) fn deconstruct(self) -> ND {
        self.non_determinism_source
    }
}

// #[derive(Default)]
pub(crate) struct SimulatorConfig {
    pub diagnostics: Option<DiagnosticsConfig>,
}

pub(crate) struct DiagnosticsConfig {
    symbols_path:PathBuf,
    pub profiler_config: Option<ProfilerConfig>,
}

pub(crate) struct ProfilerConfig {
    output_path:PathBuf,
    pub frequency_recip: u32,

}

impl Default for SimulatorConfig {
    fn default() -> Self {
        Self { 
            diagnostics: Default::default(),
        }
    }
}

impl DiagnosticsConfig {
    pub fn new(symbols_path: PathBuf) -> Self {
        Self { 
            symbols_path, 
            profiler_config: None 
        }
    }
}

impl ProfilerConfig {
    pub fn new(output_path: PathBuf) -> Self {
        Self {
            output_path,
            frequency_recip: 100,
        }
    }
}

// impl SimulatorConfig {
//     pub(crate) fn get_frequency_recip(&self) -> u32 {
//         env::var("RISCV_SIM_FREQUENCY_RECIP")
//             .ok()
//             .and_then(|val| val.parse().ok())
//             .unwrap_or(self.profiler_frequency_recip)
//     }
// }

mod diag {
    use std::{ collections::HashMap, hash::Hasher, io::Write, marker::PhantomData, mem::size_of, ops::Deref, path::{Path, PathBuf}, rc::Rc, sync::Arc};

    use addr2line::{gimli::{self, CompleteLineProgram, DebugInfoOffset, Dwarf, EndianSlice, RunTimeEndian, SectionId, UnitOffset, UnitSectionOffset}, Context, Frame, LookupResult, SplitDwarfLoad};
    use memmap2::Mmap;
    use object::{File, Object, ObjectSection};
    use addr2line::LookupContinuation;

    use crate::{abstractions::{mem_read, memory::{MemoryAccessTracer, MemorySource}, non_determinism::NonDeterminismCSRSource}, cycle::{state::RiscV32State, status_registers::TrapReason}, mmu::MMUImplementation, qol::PipeOp as _};

    use super::SimulatorConfig;


    pub(crate) struct Profiler {
        // Safety: DwarfCache references data in symbol info.
        dwarf_cache: DwarfCache,
        symbol_info: SymbolInfo,
        output_path: PathBuf,
        frequency_recip: u32,
        pub stacktraces: StacktraceSet,
    }

    impl Profiler {
        pub(crate) fn new(
            config: SimulatorConfig,
        ) -> Option<Self> {
            let dwarf_cache = DwarfCache {
                unit_data: HashMap::new()
            };

            if let Some(d) = config.diagnostics
            && let Some(p) = d.profiler_config {
                Self {
                    symbol_info: SymbolInfo::new(d.symbols_path),
                    frequency_recip: p.frequency_recip,
                    stacktraces: StacktraceSet::new(),
                    dwarf_cache,
                    output_path: p.output_path,
                }
                .to(Some)
            }
            else {
                None
            }
        }

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
            if  cycle % self.frequency_recip == 0 
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
            let symbol_info = &self.symbol_info;

            let mut callstack = Vec::with_capacity(6);

            // Current frame
            callstack.push(state.pc as u64);

            let mut fp = state.registers[8];

            // Saved frames
            loop {
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

                // TODO: remove once the issue with non complying functions is solved.
                if fpp < 8 { break; }

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

                // TODO: Remove once the issue with non complying functions is solved.
                if addr < 4 { break; }
                if next as u64 == fpp { break; }
                if addr == 0 { break; }

                // Subbing one instruction because the frame's return address point to instruction
                // that follows the call, not the call itself. In case of inlining this can be
                // several frames away.
                let addr = addr - 4;

                callstack.push(addr as u64);

                fp = next;
            }

            let mut stackframes = Vec::with_capacity(8);
            // let mut frame_refs = Vec::new();

            for (i, addr) in callstack.iter().enumerate() {
                let r = symbol_info.get_address_frames(&mut self.dwarf_cache, *addr);

                let (frames, section_offset) =
                    match r {
                    Some(r) => r,
                    // None if stackframes.len() != 0 => panic!("Non top frame couldn't be retreived."),
                    None => break,
                };

                for frame in frames {
                    let offset = frame.dw_die_offset.unwrap();
                    stackframes.push(FrameKey { section_offset, unit_offset: offset });
                    // frame_refs.push(frame);

                    if i == 0 && symbol_info.is_address_traceable(&self.dwarf_cache, state.pc as u64, &frame) {
                        // We're in a service code.
                        return;
                    }
                }
            }

            if stackframes.len() == 0 { 
                return; 
            }

            let stacktrace = Stacktrace::new(stackframes);

            self.stacktraces.absorb(stacktrace);
        }

        pub(crate) fn write_stacktrace(&self) {
            let mut file = match std::fs::File::create("/home/aikixd/temp/trace.svg") {
                Err(why) => panic!("couldn't create file {}", why),
                Ok(file) => file,
            };

            let mut mapped = Vec::with_capacity(self.stacktraces.traces.len());

            // file.write("stacktrace,count\n".as_bytes());

            for (st, c) in &self.stacktraces.traces {
                // println!(" ----- Callstack start -----");
                //
                let names =
                    st
                    .frames
                    .iter()
                    .rev()
                    .map(|frame| {
                        self.dwarf_cache.unit_data.get(&frame.section_offset).unwrap()
                        .frames.get(&frame.unit_offset).unwrap().name.as_str()
                    })
                        .collect::<Vec<_>>()
                    ;
                names.join(";")
                    .op(|x| 
                        format!("{} {}", x, c)
                        .to_owned()
                        .to(|x| mapped.push(x)));
                //
                //
                //
                // format!("{},\"", c).to(|x| file.write(x.as_bytes()));
                //
                // for frame in &st.frames {
                //
                //     format!(
                //         "{}\n", 
                //         self
                //             .dwarf_cache
                //             .unit_data
                //             .get(&frame.section_offset)
                //             .unwrap()
                //             .frames
                //             .get(&frame.unit_offset)
                //             .unwrap()
                //             .name)
                //         .as_bytes()
                //         .to(|x| file.write(x));
                // }

                // format!("\"\n").to(|x| file.write(x.as_bytes()));
            }

            let mut opts = inferno::flamegraph::Options::default();
        

            inferno::flamegraph::from_lines(&mut opts, mapped.iter().map(|x| x.as_str()), file);
        }


        // fn pack_stacktraces(&self) {
        //     self.symbol_info.
        // }
    }

    #[derive(Debug, PartialEq, Eq, Hash)]
    struct FrameKey {
        section_offset: UnitSectionOffset,
        unit_offset: UnitOffset<usize>,
    }

    struct FrameInfo {
        // Address of one instruction beyond last the prologue instruction.
        prologue_end: u64,
        // Address of the first epilogue instruction.
        epilogue_begin: u64,
        no_return: bool,
        name: String,
    }

    struct UnitInfo<'a> {
        line_program_complete: CompleteLineProgram<EndianSlice<'a, RunTimeEndian>, usize>,
        line_sequences: Vec<gimli::LineSequence<EndianSlice<'a, RunTimeEndian>>>,
        frames: HashMap<UnitOffset<usize>, FrameInfo>,
    }

    struct DwarfCache {
        unit_data: HashMap<UnitSectionOffset, UnitInfo<'static>>,
    }

    struct SymbolInfo {
        // Safety: Values must be dropped in the dependency order.
        ctx: Context<EndianSlice<'static, RunTimeEndian>>,
        // dwarf: Dwarf<EndianSlice<'static, RunTimeEndian>>,
        object: object::File<'static>,
        // Holds the slice that all above fields reference.
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

            // let dwarf = addr2line::gimli::Dwarf::load(load_section).unwrap();

            SymbolInfo {
                map,
                object,
                ctx,
                frame_names: HashMap::new(),
            }
        }

        fn is_address_traceable(&self, cache: &DwarfCache, address: u64, frame: &Frame<'_, EndianSlice<'_, RunTimeEndian>>) -> bool {

            let (dw, unit) = 
                self
                .ctx
                .find_dwarf_and_unit(address)
                .skip_all_loads()
                .expect("Frame existence implies unit.");

            cache
                .unit_data
                .get(&unit.header.offset())
                .expect("Unit info should've been created on frame loading.")
                .frames
                .get(&frame.dw_die_offset.unwrap())
                .expect("Frame info should've been created on frame loading.")
                .to(|x| address >= x.prologue_end && address < x.epilogue_begin)
        }

        fn inspect_frame(&self, address: u64, frame: &Frame<'_, EndianSlice<'_, RunTimeEndian>>) {

            let x = self.ctx.find_dwarf_and_unit(address).skip_all_loads();
            if x.is_none() { return; }

            let (dw, unit) = x.unwrap();

            let mut cursor = unit.entries_at_offset( frame.dw_die_offset.unwrap()).unwrap();
            cursor.next_entry();
            let die = cursor.current().unwrap();

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
                println!("tag {:?}", tag_n);

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

        fn get_address_frames<'a>(&'a self, cache: &mut DwarfCache, address: u64) -> Option<(Vec<Frame<'a, EndianSlice<'a, RunTimeEndian>>>, UnitSectionOffset)> {

            let (dw, unit, unit_info) =
            if let Some((dw, unit)) = self.ctx.find_dwarf_and_unit(address).skip_all_loads() {
                let unit_locator = unit.header.offset();

                let unit_info = cache.unit_data.entry(unit_locator).or_insert_with(|| {
                    let (line_program, sequences) = unit.line_program.clone().unwrap().clone().sequences().unwrap();

                    UnitInfo { 
                        line_program_complete: line_program,
                        line_sequences: sequences,
                        frames: HashMap::new(),
                    }
                });

                (dw, unit, unit_info)
            } else {
                return None
            };

            let mut frames = self.ctx.find_frames(address);

            let mut frames = loop {
                match frames {
                    LookupResult::Output(r) => break r,
                    LookupResult::Load { load: _, continuation } => {

                        // Not using split DWARF.
                        frames = continuation.resume(None);
                    }
                }
            }.unwrap();

            let mut result = Vec::with_capacity(8);

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
                //
                //

                unit_info.frames.entry(frame.dw_die_offset.unwrap()).or_insert_with(|| {
                    let sequence = &unit_info.line_sequences;
                    for s in sequence {
                        if address >= s.start && address < s.end {
                            let mut sm = unit_info.line_program_complete.resume_from(&s);

                            let mut prologue_end = None;
                            let mut epilogue_begin = None;
                            let mut no_return = false;
                            let mut inlined = false;

                            while let Ok(Some((_h, r))) = sm.next_row() {
                                assert!(r.address() <= s.end);

                                if r.prologue_end() { prologue_end = Some(r.address()) }
                                if r.epilogue_begin() { epilogue_begin = Some(r.address()) }
                            }

                            // if epilogue_begin.is_none() { self.inspect_frame(address, &frame) }

                            let cursor = 
                                unit
                                .entries_at_offset(frame.dw_die_offset.unwrap())
                                .unwrap()
                                .op(|x| { x.next_entry(); });

                            let die =
                                cursor
                                .current()
                                .unwrap();

                            match die.tag() {
                                gimli::DW_TAG_inlined_subroutine => inlined = true,
                                _ => ()
                            }

                            let mut attrs = die.attrs();

                            while let Ok(Some(attr)) = attrs.next() {
                                // println!("attr {:?}", attr);
                                match attr.name() {
                                    gimli::DW_AT_noreturn if epilogue_begin.is_some() => 
                                        panic!("Non returning functions shouln't have an epilogue."),
                                    gimli::DW_AT_noreturn =>
                                        no_return = true,
                                    _ => (),
                                }
                            }

                            return FrameInfo {
                                prologue_end: prologue_end.expect(format!("A function must have a prologue. 0x{:08x}", address).as_str()),
                                epilogue_begin: epilogue_begin.unwrap_or_else(|| {
                                    u64::MAX
                                    // if no_return || inlined { u64::MAX }
                                    // else { 
                                    //     self.inspect_frame(address, &frame);
                                    //     panic!("A returning function must have an epilogue. 0x{:08x}", address) 
                                    // }
                                }),
                                no_return,
                                name: frame.function.as_ref().unwrap().demangle().unwrap().to_string()
                            }
                        }
                    }

                    panic!(
                        "An line sequence was not found for frame {:?}, addr {}",
                        frame.function.as_ref().unwrap().demangle(),
                        address);
                });

                // Safety: The borrow checker assumes that the frame lives for 'const (derived from
                // `ctx` field in `Self`). The actual lifetime is the lifetime of `self`. So we're
                // adjusting the lifetime args in the return type accordingly.
                unsafe { result.push(std::mem::transmute(frame)) };
            }

            Some((result, unit.header.offset()))
        }

        // fn get_frame(&self) {
        //     self.dwarf.
        // }
    }
    
    #[derive(Debug)]
    struct Stacktrace {
        frames: Vec<FrameKey>,
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
        pub(crate) fn new(frames: Vec<FrameKey>) -> Self {
            assert_ne!(0, frames.len());
            Self { frames }
        }
    }

    // struct StacktraceIterator<'a, I: Iterator<Item = FrameKey>> {
    //     // stacktrace: &'a Stacktrace,
    //     dwarf_cache: &'a DwarfCache,
    //     stacktrace_iter: I,
    // }
    //
    // impl<'a, I: Iterator<Item = FrameKey>> Iterator for StacktraceIterator<'a, I> {
    //     type Item = &'a str;
    //
    //     fn next(&mut self) -> Option<Self::Item> {
    //         self
    //             .stacktrace_iter
    //             .next()
    //             .map(|x| {
    //                 kj
    //             })
    //     }
    // }


    #[derive(Debug)]
    pub(crate) struct StacktraceSet {
        // names: HashMap<UnitOffset<usize>, String>,
        traces: HashMap<Stacktrace, usize>,
    }
    
    impl StacktraceSet {
        fn new() -> Self {
            Self {
                // names: HashMap::new(),
                traces: HashMap::new()
            }
        }

        
        fn absorb(&mut self, stacktrace: Stacktrace) 
        {
            // for (o, n) in names {
            //     self.names.entry(o).or_insert(n.deref().to_owned());
            // }
            self.traces.entry(stacktrace).and_modify(|x| *x += 1).or_insert(1);
        }
    }
}
