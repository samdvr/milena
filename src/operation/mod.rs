use std::{error::Error, time::Duration};

use aws_sdk_s3::Client;
use rocksdb::Options;

use crate::store::{DiskStore, Key, LRUStore, S3Store, Store, Value};

pub struct Operation<I, O, C> {
    in_memory_store: I,
    on_disk_store: O,
    cloud_store: C,
}

impl<I: Store, O: Store, C: Store> Operation<I, O, C> {
    pub fn simple_new<'a>(
        in_memory_lru_capacity: u64,
        disk_store_ttl: Duration,
        client: Client,
    ) -> Operation<LRUStore, DiskStore, S3Store> {
        let mut in_memory_store = LRUStore::new(in_memory_lru_capacity);
        let mut on_disk_store = DiskStore::new(&Options::default(), disk_store_ttl, "./db");
        let mut cloud_store = S3Store { client };

        Operation {
            in_memory_store,
            on_disk_store,
            cloud_store,
        }
    }
    pub async fn get(&mut self, bucket: &str, key: &Key) -> Result<Option<Value>, String> {
        // Check in-memory store first
        if let Some(data) = self.in_memory_store.get(bucket, key).await? {
            return Ok(Some(data));
        }

        // Check on-disk store next
        if let Some(data) = self.on_disk_store.get(bucket, key).await? {
            // Store data in in-memory store before returning it
            self.in_memory_store.put(bucket, key, &data).await?;
            return Ok(Some(data));
        }

        // Check cloud store if data is not found in cache
        if let Some(data) = self.cloud_store.get(bucket, key).await? {
            // Store data in in-memory and on-disk stores before returning it
            self.in_memory_store.put(bucket, key, &data).await?;
            self.on_disk_store.put(bucket, key, &data).await?;
            return Ok(Some(data));
        }

        Ok(None)
    }

    pub async fn put(&mut self, bucket: &str, key: &Key, value: &Value) -> Result<(), String> {
        self.cloud_store.put(bucket, key, value).await?;
        self.on_disk_store.put(bucket, key, value).await?;
        self.in_memory_store.put(bucket, key, value).await
    }

    pub async fn delete(&mut self, bucket: &str, key: &Key) -> Result<(), String> {
        self.cloud_store.delete(bucket, key).await?;
        self.on_disk_store.delete(bucket, key).await?;
        self.in_memory_store.delete(bucket, key).await
    }
}
