use crate::cycle::state::RiscV32State;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BatchAccessPartialData {
    Read { read_value: u32 },
    Write { read_value: u32, written_value: u32 },
}

pub trait Tracer {
    type AuxData;

    fn create_from_initial_state(state: &RiscV32State, aux_data: Self::AuxData) -> Self;

    #[inline(always)]
    fn at_cycle_start(&mut self, current_state: &RiscV32State) {}

    #[inline(always)]
    fn at_cycle_end(&mut self, current_state: &RiscV32State) {}

    #[inline(always)]
    fn trace_opcode_read(
        &mut self,
        phys_address: u64,
        read_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_rs1_read(
        &mut self,
        reg_idx: u32,
        read_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_rs2_read(
        &mut self,
        reg_idx: u32,
        read_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_rd_write(
        &mut self,
        reg_idx: u32,
        read_value: u32,
        written_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_non_determinism_read(
        &mut self,
        read_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_non_determinism_write(
        &mut self,
        written_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_ram_read(
        &mut self,
        phys_address: u64,
        read_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_ram_read_write(
        &mut self,
        phys_address: u64,
        read_value: u32,
        written_value: u32,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_address_translation(
        &mut self,
        satp_value: u32,
        virtual_address: u64,
        phys_address: u64,
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }

    #[inline(always)]
    fn trace_batch_memory_access(
        &mut self,
        access_id: u32,
        phys_address_high: u16,
        accesses: &[BatchAccessPartialData],
        proc_cycle: u32,
        cycle_timestamp: u32,
    ) {
    }
}

impl Tracer for () {
    type AuxData = ();

    fn create_from_initial_state(_state: &RiscV32State, _aux_data: Self::AuxData) -> Self {
        ()
    }
}
