use std::{error::Error, time::Duration};

use aws_sdk_s3::{types::ByteStream, Client};
use rocksdb::Options;

use crate::store::{DiskStore, Key, LRUStore, S3Store, Store, Value};

struct Cache<I, O, C> {
    in_memory_store: Box<I>,
    on_disk_store: Box<O>,
    cloud_store: Box<C>,
}

impl<I: Store + Copy, O: Store + Copy, C: Store + Copy> Cache<I, O, C> {
    fn simple_new<'a>(
        in_memory_lru_capacity: u64,
        disk_store_ttl: Duration,
        client: Client,
        throttle: Duration,
    ) -> Cache<LRUStore, DiskStore, S3Store> {
        let mut in_memory_store = Box::new(LRUStore::new(in_memory_lru_capacity));
        let mut on_disk_store =
            Box::new(DiskStore::new(&Options::default(), disk_store_ttl, "./db"));
        let mut cloud_store = Box::new(S3Store { client, throttle });

        Cache {
            in_memory_store,
            on_disk_store,
            cloud_store,
        }
    }
    async fn get(
        &mut self,
        bucket: &str,
        key: &Key,
        from_cache: Option<bool>,
    ) -> Result<Option<Value>, Box<dyn Error>> {
        let read_from_cache = from_cache.unwrap_or(true);

        if read_from_cache {
            // Check in-memory store first
            match self.in_memory_store.get(bucket, key).await {
                Ok(Some(data)) => return Ok(Some(data)),
                Ok(None) => {}
                Err(e) => return Err(e),
            }

            // Check on-disk store next
            match self.on_disk_store.get(bucket, key).await {
                Ok(Some(data)) => {
                    // Store data in in-memory store before returning it
                    self.in_memory_store.put(bucket, key, &data).await;
                    return Ok(Some(data));
                }
                Ok(None) => {}
                Err(e) => return Err(e),
            }

            // Check cloud store if data is not found in cache
            match self.cloud_store.get(bucket, key).await {
                Ok(Some(data)) => {
                    // Store data in in-memory and on-disk stores before returning it
                    self.in_memory_store.put(bucket, key, &data).await?;
                    self.on_disk_store.put(bucket, key, &data).await?;
                    return Ok(Some(data));
                }
                Ok(None) => return Ok(None),
                Err(e) => return Err(e),
            }
        } else {
            // Return data from cloud store without checking cache
            self.cloud_store.get(bucket, key).await
        }
    }

    async fn put(&mut self, bucket: &str, key: &Key, value: &Value) -> Result<(), Box<dyn Error>> {
        self.cloud_store.put(bucket, key, value).await?;
        self.on_disk_store.put(bucket, key, value).await?;
        self.in_memory_store.put(bucket, key, value).await
    }

    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<(), Box<dyn Error>> {
        self.cloud_store.delete(bucket, key).await?;
        self.on_disk_store.delete(bucket, key).await?;
        self.in_memory_store.delete(bucket, key).await
    }

    async fn put_in_cache(
        &mut self,
        bucket: &str,
        key: &Key,
        value: &Value,
    ) -> Result<(), Box<dyn Error>> {
        let b = self.in_memory_store.put(bucket, key, value).await?;
        self.on_disk_store.put(bucket, key, value).await
    }
}
