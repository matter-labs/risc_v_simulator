// Hook zk_ee IOOracle to be NonDeterminismCSRSource

use ringbuffer::RingBuffer;
use std::collections::BTreeMap;
use zk_ee::system::kv_markers::UsizeDeserializable;
use zk_ee::system::system_io_oracle::*;
use zk_ee::system::types_config::*;

use super::memory::MemorySource;
use super::non_determinism::NonDeterminismCSRSource;

pub struct ZkEENonDeterminismSource<M: MemorySource> {
    query_buffer: Option<QueryBuffer>,
    current_iterator: Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>>,
    iterator_len_to_indicate: Option<u32>,
    high_half: Option<u32>,
    is_connected_to_external_oracle: bool,
    last_values_buffer: ringbuffer::ConstGenericRingBuffer<u32, 8>,
    processors: Vec<Box<dyn OracleQueryProcessor<M> + 'static>>,
    ranges: BTreeMap<u32, usize>,
}

impl<M: MemorySource> Default for ZkEENonDeterminismSource<M> {
    fn default() -> Self {
        Self {
            query_buffer: None,
            current_iterator: None,
            iterator_len_to_indicate: None,
            high_half: None,
            is_connected_to_external_oracle: false,
            last_values_buffer: ringbuffer::ConstGenericRingBuffer::new(),
            processors: Vec::new(),
            ranges: BTreeMap::new(),
        }
    }
}

impl<M: MemorySource> ZkEENonDeterminismSource<M> {
    pub fn add_external_processor<P: OracleQueryProcessor<M> + 'static>(&mut self, processor: P) {
        let query_ids = processor.supported_query_ids();
        let processor_id = self.processors.len();
        for id in query_ids.into_iter() {
            let existing = self.ranges.insert(id, processor_id);
            assert!(
                existing.is_none(),
                "more than one processor for query id 0x{:08x}",
                id
            );
        }
        self.processors.push(Box::new(processor));
        self.is_connected_to_external_oracle = true;
    }

    fn process_buffered_query(&mut self, memory: &M) {
        assert!(self.current_iterator.is_none());

        let buffer = self.query_buffer.take().expect("must exist");
        let query_id = buffer.query_type;
        if query_id == DisconnectOracleFormalIterator::ID {
            self.is_connected_to_external_oracle = false;
            return;
        } else {
            let buffer = buffer.buffer;
            let Some(processor_id) = self.ranges.get(&query_id).copied() else {
                panic!("Can not proceed query with ID = 0x{:08x}", query_id);
            };
            let processor = &mut self.processors[processor_id];
            let new_iterator = processor.process_buffered_query(query_id, buffer, memory);

            if let Some(new_iterator) = new_iterator {
                let result_len = new_iterator.len() * 2; // NOTE for mismatch of 32/64 bit archs
                self.iterator_len_to_indicate = Some(result_len as u32);
                if result_len > 0 {
                    self.current_iterator = Some(new_iterator);
                }
            } else {
                self.iterator_len_to_indicate = Some(0);
            }
        }
    }

    fn read_impl(&mut self) -> u32 {
        // We mocked reads, so it's filtered out before
        if self.is_connected_to_external_oracle == false {
            return 0;
        }

        if let Some(iterator_len_to_indicate) = self.iterator_len_to_indicate.take() {
            return iterator_len_to_indicate;
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

    fn write_impl(&mut self, memory: &M, value: u32) {
        self.last_values_buffer.push(value);

        // may have something from remains
        if self.current_iterator.is_some() {
            self.current_iterator = None;
        }
        if self.iterator_len_to_indicate.is_some() {
            self.iterator_len_to_indicate = None;
        }
        if self.high_half.is_some() {
            self.high_half = None;
        }

        if let Some(query_buffer) = self.query_buffer.as_mut() {
            let complete = query_buffer.write(value);
            if complete {
                self.process_buffered_query(memory);
            }
        } else {
            if self.is_connected_to_external_oracle == false {
                if value != UARTAccessMarker::ID {
                    // we are not interested in general to start another query
                    return;
                }
            }
            let new_buffer = QueryBuffer::empty_for_query_type(value);
            self.query_buffer = Some(new_buffer);
        }
    }

    pub fn get_possible_program_output(&self) -> [u32; 8] {
        let mut result = [0u32; 8];
        for (dst, src) in result.iter_mut().zip(self.last_values_buffer.iter()) {
            *dst = *src;
        }

        result
    }
}

pub trait OracleQueryProcessor<M: MemorySource> {
    fn supported_query_ids(&self) -> Vec<u32>;
    fn supports_query_id(&self, query_id: u32) -> bool {
        self.supported_query_ids().contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &M,
    ) -> Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>>;
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
            // println!("Writing word 0x{:08x} for query ID = 0x{:08x}", value, self.query_type);
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
            // println!("Expecting {} words for query ID = 0x{:08x}", value, self.query_type);
            self.remaining_len = Some(value as usize);
            if value == 0 {
                // nothing else to expect
                true
            } else {
                false
            }
        }
    }
}

pub struct BasicZkEEOracleWrapper<IOTypes: SystemIOTypesConfig, O: IOOracle<IOTypes>>
where
    for<'a> O::MarkerTiedIterator<'a>: 'static,
{
    oracle: O,
    is_connected_to_external_oracle: bool,
    _marker: std::marker::PhantomData<IOTypes>,
}

impl<IOTypes: SystemIOTypesConfig, O: IOOracle<IOTypes>> BasicZkEEOracleWrapper<IOTypes, O>
where
    for<'a> O::MarkerTiedIterator<'a>: 'static,
{
    pub fn new(oracle: O) -> Self {
        Self {
            oracle,
            is_connected_to_external_oracle: true,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<IOTypes: SystemIOTypesConfig, O: IOOracle<IOTypes>, M: MemorySource> OracleQueryProcessor<M>
    for BasicZkEEOracleWrapper<IOTypes, O>
where
    for<'a> O::MarkerTiedIterator<'a>: 'static,
{
    fn supported_query_ids(&self) -> Vec<u32> {
        let supported = &[
            NextTxSize::ID,
            NewTxContentIterator::ID,
            InitializeIOImplementerIterator::ID,
            ProofForStorageReadIterator::<EthereumIOTypesConfig>::ID,
            ProofForStorageWriteIterator::<EthereumIOTypesConfig>::ID,
            PreimageByteLenIterator::ID,
            InitialStorageSlotDataIterator::<EthereumIOTypesConfig>::ID,
            PreimageContentWordsIterator::ID,
            StartFrameFormalIterator::ID,
            EndFrameFormalIterator::ID,
            DisconnectOracleFormalIterator::ID,
            ProofForIndexIterator::ID,
            NeighboursIndexesIterator::ID,
            ExactIndexIterator::ID,
            UARTAccessMarker::ID,
        ];

        debug_assert!(supported.is_sorted());

        supported.to_vec()
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &M,
    ) -> Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>> {
        debug_assert!(<Self as OracleQueryProcessor<M>>::supports_query_id(
            self, query_id
        ));

        let new_iterator: O::MarkerTiedIterator<'_> = match query_id {
            NextTxSize::ID => {
                let mut src_it = query.into_iter();
                let params = <<NextTxSize as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<NextTxSize>(params)
                    .expect("must make an iterator");

                it
            }
            NewTxContentIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<NewTxContentIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<NewTxContentIterator>(params)
                    .expect("must make an iterator");
                it
            }
            InitializeIOImplementerIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<InitializeIOImplementerIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<InitializeIOImplementerIterator>(params)
                    .expect("must make an iterator");
                it
            }
            ProofForStorageReadIterator::<EthereumIOTypesConfig>::ID => {
                let mut src_it = query.into_iter();
                let params = <<ProofForStorageReadIterator<EthereumIOTypesConfig> as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<ProofForStorageReadIterator<EthereumIOTypesConfig>>(params)
                    .expect("must make an iterator");
                it
            }
            ProofForStorageWriteIterator::<EthereumIOTypesConfig>::ID => {
                let mut src_it = query.into_iter();
                let params = <<ProofForStorageWriteIterator<EthereumIOTypesConfig> as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<ProofForStorageWriteIterator<EthereumIOTypesConfig>>(params)
                    .expect("must make an iterator");
                it
            }
            PreimageByteLenIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<PreimageByteLenIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<PreimageByteLenIterator>(params)
                    .expect("must make an iterator");
                it
            }
            InitialStorageSlotDataIterator::<EthereumIOTypesConfig>::ID => {
                let mut src_it = query.into_iter();
                let params = <<InitialStorageSlotDataIterator::<EthereumIOTypesConfig> as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<InitialStorageSlotDataIterator<EthereumIOTypesConfig>>(params)
                    .expect("must make an iterator");
                it
            }
            PreimageContentWordsIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<PreimageContentWordsIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<PreimageContentWordsIterator>(params)
                    .expect("must make an iterator");
                it
            }
            StartFrameFormalIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<StartFrameFormalIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .make_iterator::<StartFrameFormalIterator>(params)
                    .expect("must make an iterator");
                it
            }
            EndFrameFormalIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<EndFrameFormalIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .make_iterator::<EndFrameFormalIterator>(params)
                    .expect("must make an iterator");
                it
            }
            DisconnectOracleFormalIterator::ID => {
                self.is_connected_to_external_oracle = false;
                return None;
            }
            ProofForIndexIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<ProofForIndexIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .make_iterator::<ProofForIndexIterator>(params)
                    .expect("must make an iterator");
                it
            }
            NeighboursIndexesIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<NeighboursIndexesIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .make_iterator::<NeighboursIndexesIterator>(params)
                    .expect("must make an iterator");
                it
            }
            ExactIndexIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<ExactIndexIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .make_iterator::<ExactIndexIterator>(params)
                    .expect("must make an iterator");
                it
            }
            UARTAccessMarker::ID => {
                // just our old plain uart
                let output = query;
                let u32_vec: Vec<u32> = output
                    .into_iter()
                    .flat_map(|el| [el as u32, (el >> 32) as u32])
                    .collect();
                assert!(u32_vec.len() > 0);
                let len = u32_vec[0] as usize;
                let mut string_bytes: Vec<u8> = u32_vec[1..]
                    .iter()
                    .flat_map(|el| el.to_le_bytes())
                    .collect();
                assert!(string_bytes.len() >= len);
                string_bytes.truncate(len);
                let string = String::from_utf8(string_bytes).unwrap();
                println!("UART: {}", string);

                return None;
            }
            a @ _ => {
                panic!("Can not proceed query with ID = 0x{:08x}", a);
            }
        };

        if new_iterator.len() > 0 {
            Some(Box::new(new_iterator))
        } else {
            None
        }
    }
}

// now we hook an access
impl<M: MemorySource> NonDeterminismCSRSource<M> for ZkEENonDeterminismSource<M> {
    fn read(&mut self) -> u32 {
        self.read_impl()
    }

    fn write_with_memory_access(&mut self, memory: &M, value: u32) {
        self.write_impl(memory, value)
    }
}
