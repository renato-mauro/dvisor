use std::{collections::HashMap, sync::{Arc, Mutex}};
use parquet::{errors::ParquetError, file::{reader::{ChunkReader, Length}, serialized_reader::SliceableCursor}};

use crate::{error::DvisorResult, urlrequest::URLRequest};

struct UrlDataProvider {
    url: String,
    table: HashMap<u64,Arc<Vec<u8>>>,
    cache_size: usize,
    request_count: usize,
    cache_hits: usize,
    request_size: usize
}

impl UrlDataProvider {
    pub fn new(url: &str) -> UrlDataProvider {
        UrlDataProvider { 
            url: url.to_string(), 
            table: HashMap::new(), 
            cache_size: 0,
            request_count: 0,
            cache_hits: 0,
            request_size: 0
        }
    }

    pub fn read(&mut self, start: u64, length: usize) -> DvisorResult<Arc<Vec<u8>>> {
        self.request_count+=1;
        self.request_size+=length;
        if let Some(data) = self.table.get(&start) {
            self.cache_hits += 1;
            Ok(data.clone())
        } else {
            let data = Arc::new(URLRequest::fetch_range(&self.url,start,length)?);
            self.cache_size += length;
            if self.cache_size >= 512*1024*1024 {
                self.cache_size = length;
                self.table.clear();
            }
            self.table.insert(start, data.clone());
            Ok(data)
        }
    }

    fn len(&self) -> DvisorResult<u64> {
        URLRequest::resource_len(&self.url)
    }


}

pub struct UrlChunkReader {
    length: u64,
    provider: Arc<Mutex<UrlDataProvider>>,
}

impl UrlChunkReader {
    pub fn new(url: &str) -> DvisorResult<UrlChunkReader> {
        let provider = UrlDataProvider::new(url);
        let length = provider.len()?;
        Ok(UrlChunkReader { length, provider: Arc::new(Mutex::new(provider)) })
    }
}

impl Length for UrlChunkReader {
    fn len(&self) -> u64 {
        self.length
    }
}

impl ChunkReader for UrlChunkReader {
    type T = SliceableCursor;
    fn get_read(&self, start: u64, length: usize) -> parquet::errors::Result<Self::T> {
        if let Ok(mut provider) = self.provider.lock() {
            Ok(SliceableCursor::new(provider.read(start, length)?))
        } else {
            Err(ParquetError::EOF("Error in ChunkReader - Mutex".to_string()))
        }
    }
}
