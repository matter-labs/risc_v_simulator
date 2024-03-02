// Hook zk_ee IOOracle to be NonDeterminismCSRSource

use std::pin::Pin;
use zk_ee::system::kv_markers::UsizeDeserializable;
use zk_ee::system::system_io_oracle::*;
use zk_ee::system::types_config::*;

use super::non_determinism::NonDeterminismCSRSource;

pub struct ZkEEOracleWrapper<'this, IOTypes: SystemIOTypesConfig, O: IOOracle<IOTypes>> {
    // unfortunately self-ref
    current_iterator: Option<O::MarkerTiedIterator<'this>>,
    oracle: O,
    query_buffer: Option<QueryBuffer>,
    high_half: Option<u32>,
    _marker: std::marker::PhantomPinned,
}

impl<'this, IOTypes: SystemIOTypesConfig, O: IOOracle<IOTypes>> Drop
    for ZkEEOracleWrapper<'this, IOTypes, O>
{
    fn drop(&mut self) {
        // we can not assert that iterator is dropped in general (we want to have no panics even if payload panics),
        // but we can still drop it first
        drop(self.current_iterator.take());
        // and oracle doesn't require anything special
    }
}

struct QueryBuffer {
    query_type: u32,
    remaining_len: Option<usize>,
    write_low: bool,
    buffer: Vec<usize>,
}

impl QueryBuffer {
    fn empty_for_query_type(query_type: u32) -> Self {
        Self {
            query_type,
            remaining_len: None,
            write_low: true,
            buffer: Vec::new(),
        }
    }

    fn write(&mut self, value: u32) -> bool {
        // NOTE: we have to match between 32 bit inner env and 64 bit outer env
        if let Some(remaining_len) = self.remaining_len.as_mut() {
            if self.write_low {
                self.buffer.push(value as usize);
                self.write_low = false;
            } else {
                let last = self.buffer.last_mut().unwrap();
                *last |= (value as usize) << 32;
                self.write_low = true;
            }
            *remaining_len -= 1;
            *remaining_len == 0
        } else {
            self.remaining_len = Some(value as usize);
            false
        }
    }
}

impl<'this, IOTypes: SystemIOTypesConfig, O: IOOracle<IOTypes>>
    ZkEEOracleWrapper<'this, IOTypes, O>
{
    pub fn new(oracle: O) -> Pin<Box<Self>> {
        let inner = Self {
            current_iterator: None,
            oracle,
            query_buffer: None,
            high_half: None,
            _marker: std::marker::PhantomPinned,
        };

        Box::pin(inner)
    }

    fn supported_query_ids() -> &'static [u32] {
        let supported = &[
            NextTxSizeWords::ID,
            NewTxContentIterator::ID,
            InitializeIOImplementerIterator::ID,
            ProofForStorageReadIterator::<EthereumIOTypesConfig>::ID,
            ProofForStorageWriteIterator::<EthereumIOTypesConfig>::ID,
            PreimageByteLenIterator::ID,
            InitialStorageSlotDataIterator::<EthereumIOTypesConfig>::ID,
            PreimageContentWordsIterator::ID,
            StartFrameFormalIterator::ID,
            EndFrameFormalIterator::ID,
            UARTAccessMarker::ID,
        ];

        debug_assert!(supported.is_sorted());

        supported
    }

    fn proceed_buffered_query(&mut self) {
        let buffer = self.query_buffer.take().expect("must exist");
        let query_id = buffer.query_type;
        debug_assert!(Self::supported_query_ids().contains(&query_id));
        let mut src_it = buffer.buffer.into_iter();
        let new_iterator: O::MarkerTiedIterator<'this> = match query_id {
            NextTxSizeWords::ID => {
                let params = <<NextTxSizeWords as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<NextTxSizeWords>(params)
                    .expect("must make an iterator");
                // extend lifetime
                unsafe { std::mem::transmute(it) }
            }
            UARTAccessMarker::ID => {
                // just our old plain uart
                todo!();
            }
            _ => {
                panic!()
            }
        };

        if new_iterator.len() > 0 {
            self.current_iterator = Some(new_iterator)
        }
    }

    fn read_impl(&mut self) -> u32 {
        if self.query_buffer.is_some() || self.current_iterator.is_some() {
            // take into account that CSRRW first reads, and then writes,
            // so we should ignore it
            return 0;
        }
        if let Some(high) = self.high_half.take() {
            return high;
        }
        let Some(current_iterator) = self.current_iterator.as_mut() else {
            panic!("trying to read, but data is not prepared");
        };
        let next = current_iterator.next().expect("must contain next element");
        if current_iterator.len() == 0 {
            // we are done
            self.current_iterator = None;
        }
        let high = (next >> 32) as u32;
        let low = next as u32;
        self.high_half = Some(high);

        low
    }

    fn write_impl(&mut self, value: u32) {
        if self.high_half.is_some() {
            // may have something from remains
            self.high_half = None;
        }
        if let Some(query_buffer) = self.query_buffer.as_mut() {
            let complete = query_buffer.write(value);
            if complete {
                // we can make an iterator
                self.proceed_buffered_query();
            }
        } else {
            assert!(Self::supported_query_ids().contains(&value));
            let new_buffer = QueryBuffer::empty_for_query_type(value);
            self.query_buffer = Some(new_buffer);
        }
    }
}

// now we hook an access
impl<'this, IOTypes: SystemIOTypesConfig, O: IOOracle<IOTypes>> NonDeterminismCSRSource
    for Pin<Box<ZkEEOracleWrapper<'this, IOTypes, O>>>
{
    fn read(&mut self) -> u32 {
        // Box<Pin<Self>> is not Unpin, so we will go unto project unchecked
        unsafe { Pin::get_unchecked_mut(self.as_mut()).read_impl() }
    }

    fn write(&mut self, value: u32) {
        // Box<Pin<Self>> is not Unpin, so we will go unto project unchecked
        unsafe { Pin::get_unchecked_mut(self.as_mut()).write_impl(value) }
    }
}
