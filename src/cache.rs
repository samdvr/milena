use std::{error::Error, time::Duration};

use aws_sdk_s3::{types::ByteStream, Client};
use rocksdb::Options;

use crate::store::{DiskStore, LRUStore, S3Store, Store};

struct Cache<I, O, C> {
    in_memory_store: I,
    on_disk_store: O,
    cloud_store: C,
}

impl<I: Store, O: Store, C: Store> Cache<I, O, C> {
    fn simple_new(
        in_memory_lru_capacity: u64,
        disk_store_ttl: Duration,
        client: Client,
        throttle: Duration,
    ) -> Cache<LRUStore, DiskStore, S3Store> {
        let mut in_memory_store = LRUStore::new(in_memory_lru_capacity);
        let mut on_disk_store = DiskStore::new(&Options::default(), disk_store_ttl, "./db");
        let mut cloud_store = S3Store { client, throttle };

        Cache {
            in_memory_store,
            on_disk_store,
            cloud_store,
        }
    }
    async fn get(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        from_cache: Option<bool>,
    ) -> Result<Option<ByteStream>, Box<dyn Error>> {
        let read_from_cache = from_cache.unwrap_or(true);

        if read_from_cache {
            match self.in_memory_store.get(bucket, key).await {
                Ok(Some(data)) => Ok(Some(data)),
                Ok(None) => match self.on_disk_store.get(bucket, key).await {
                    Ok(Some(v)) => {
                        self.in_memory_store.put(bucket, key, &v);
                        Ok(Some(v))
                    }
                    Ok(None) => {
                        let cloud_data = self.cloud_store.get(bucket, key).await;
                        match cloud_data {
                            Ok(Some(v)) => {
                                self.in_memory_store.put(bucket, key, &v).await?;
                                self.on_disk_store.put(bucket, key, &v).await?;

                                Ok(Some(v))
                            }
                            Ok(None) => Ok(None),
                            Err(e) => Err(e),
                        }
                    }
                    Err(_e) => self.cloud_store.get(bucket, key).await, //todo fix this
                },
                Err(_e) => self.cloud_store.get(bucket, key).await,
            }
        } else {
            self.cloud_store.get(bucket, key).await
        }
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: &ByteStream,
    ) -> Result<(), Box<dyn Error>> {
        self.cloud_store.put(bucket, key, value).await?;
        self.on_disk_store.put(bucket, key, value).await?;
        self.in_memory_store.put(bucket, key, value).await
    }

    async fn delete(&mut self, bucket: &str, key: &Vec<u8>) -> Result<(), Box<dyn Error>> {
        self.cloud_store.delete(bucket, key).await?;
        self.on_disk_store.delete(bucket, key).await?;
        self.in_memory_store.delete(bucket, key).await
    }

    async fn put_in_cache(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: &ByteStream,
    ) -> Result<(), Box<dyn Error>> {
        let b = self.in_memory_store.put(bucket, key, value).await?;
        self.on_disk_store.put(bucket, key, value).await
    }
}
