use crate::cycle::{state::RiscV32State, MachineConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BatchAccessPartialData {
    Read { read_value: u32 },
    Write { read_value: u32, written_value: u32 },
}

pub trait Tracer<C: MachineConfig> {
    type AuxData;

    fn create_from_initial_state(state: &RiscV32State<C>, aux_data: Self::AuxData) -> Self;

    #[inline(always)]
    fn at_cycle_start(&mut self, _current_state: &RiscV32State<C>) {}

    #[inline(always)]
    fn at_cycle_end(&mut self, _current_state: &RiscV32State<C>) {}

    #[inline(always)]
    fn trace_opcode_read(
        &mut self,
        _phys_address: u64,
        _read_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_rs1_read(
        &mut self,
        _reg_idx: u32,
        _read_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_rs2_read(
        &mut self,
        _reg_idx: u32,
        _read_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_rd_write(
        &mut self,
        _reg_idx: u32,
        _read_value: u32,
        _written_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_non_determinism_read(
        &mut self,
        _read_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_non_determinism_write(
        &mut self,
        _written_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_ram_read(
        &mut self,
        _phys_address: u64,
        _read_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_ram_read_write(
        &mut self,
        _phys_address: u64,
        _read_value: u32,
        _written_value: u32,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_address_translation(
        &mut self,
        _satp_value: u32,
        _virtual_address: u64,
        _phys_address: u64,
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_batch_memory_access(
        &mut self,
        _access_id: u32,
        _phys_address_high: u16,
        _accesses: &[BatchAccessPartialData],
        _proc_cycle: u32,
        _cycle_timestamp: u32,
    ) {
    }
}

impl<C: MachineConfig> Tracer<C> for () {
    type AuxData = ();

    fn create_from_initial_state(_state: &RiscV32State<C>, _aux_data: Self::AuxData) -> Self {
        ()
    }
}
