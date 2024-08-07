use crate::cycle::state::RiscV32State;

pub trait Tracer {
    fn create_from_initial_state(state: &RiscV32State) -> Self;

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
}

impl Tracer for () {
    fn create_from_initial_state(_state: &RiscV32State) -> Self {
        ()
    }
}

pub trait TracerExt<I: 'static + Clone>: Tracer {
    fn inflate(self) -> I;
}
