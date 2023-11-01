use crate::cycle::status_registers::TrapReason;

pub mod quasi_uart;

// we assume that any necessary tracing is INSIDE of the MMIO
pub trait MMIOSource {
    fn address_range(&self) -> std::ops::Range<u64>;
    fn read(&mut self, address: u64, trap: &mut TrapReason) -> u32;
    fn write(&mut self, address: u64, value: u32, trap: &mut TrapReason);
}

pub struct MMIOImplementation<'a, const N: usize> {
    pub sources: [(std::ops::Range<u64>, &'a mut dyn MMIOSource); N],
}

impl<'a, const N: usize> MMIOImplementation<'a, N> {
    pub fn construct(sources: &'a mut [Box<dyn MMIOSource>]) -> Self {
        assert_eq!(sources.len(), N);
        let mut inners: Vec<(std::ops::Range<u64>, &'a mut dyn MMIOSource)> = Vec::with_capacity(N);
        let mut checker: Vec<std::ops::Range<u64>> = Vec::with_capacity(N);
        for source in sources.iter_mut() {
            let range = source.address_range();
            for other_range in checker.iter() {
                if other_range.contains(&range.start)
                    || (range.end > 0 && other_range.contains(&(range.end - 1)))
                {
                    panic!("Intersecting MMIO ranges");
                }
            }
            checker.push(range.clone());

            inners.push((range, source.as_mut()));
        }

        assert_eq!(inners.len(), N);

        Self {
            #[allow(unreachable_code)]
            sources: unsafe { inners.try_into().unwrap_unchecked() },
        }
    }

    pub fn read(&mut self, phys_address: u64, trap: &mut TrapReason) -> Result<u32, ()> {
        for (range, source) in self.sources.iter_mut() {
            if range.contains(&phys_address) {
                let value = source.read(phys_address, trap);

                return Ok(value);
            }
        }

        Err(())
    }

    pub fn write(
        &mut self,
        phys_address: u64,
        value: u32,
        trap: &mut TrapReason,
    ) -> Result<(), ()> {
        for (range, source) in self.sources.iter_mut() {
            if range.contains(&phys_address) {
                source.write(phys_address, value, trap);

                return Ok(());
            }
        }

        Err(())
    }
}
