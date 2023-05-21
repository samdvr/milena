use std::time::Duration;

use anyhow::Result;
use aws_sdk_s3::Client;
use rocksdb::Options;
use tonic::async_trait;

use crate::store::{DiskStore, Key, LRUStore, S3Store, Store, Value};

pub struct Operation<I, O, C> {
    in_memory_store: I,
    on_disk_store: O,
    cloud_store: C,
}

impl<I: Store, O: Store, C: Store> Operation<I, O, C> {
    pub fn simple_new(
        in_memory_lru_capacity: u64,
        disk_store_ttl: Duration,
        client: Client,
    ) -> Operation<LRUStore, DiskStore, S3Store> {
        let in_memory_store = LRUStore::new(in_memory_lru_capacity);
        let mut ops = Options::default();
        ops.create_if_missing(true);
        let on_disk_store = DiskStore::new(&ops, disk_store_ttl, "./db");
        let cloud_store = S3Store { client };

        Operation {
            in_memory_store,
            on_disk_store,
            cloud_store,
        }
    }
    pub async fn get(&mut self, bucket: &str, key: &Key) -> Result<Option<Value>> {
        // Check in-memory store first
        if let Some(data) = self.in_memory_store.get(bucket, key).await? {
            print!("here!!! 37");
            return Ok(Some(data));
        }

        // Check on-disk store next
        if let Some(data) = self.on_disk_store.get(bucket, key).await? {
            // Store data in in-memory store before returning it
            self.in_memory_store.put(bucket, key, &data).await?;
            print!("here!!! 45");
            return Ok(Some(data));
        }

        // Check cloud store if data is not found in cache
        if let Some(data) = self.cloud_store.get(bucket, key).await? {
            // Store data in in-memory and on-disk stores before returning it
            self.in_memory_store.put(bucket, key, &data).await?;
            self.on_disk_store.put(bucket, key, &data).await?;
            print!("here!!! 54");
            return Ok(Some(data));
        }

        print!("here!!! 58");

        Ok(None)
    }

    pub async fn put(&mut self, bucket: &str, key: &Key, value: &Value) -> Result<()> {
        self.cloud_store.put(bucket, key, value).await?;
        self.on_disk_store.put(bucket, key, value).await?;
        self.in_memory_store.put(bucket, key, value).await
    }

    pub async fn delete(&mut self, bucket: &str, key: &Key) -> Result<()> {
        self.cloud_store.delete(bucket, key).await?;
        self.on_disk_store.delete(bucket, key).await?;
        self.in_memory_store.delete(bucket, key).await
    }
}

mod tests {

    use super::*;
    use std::collections::HashMap;
    use tonic::async_trait;

    pub struct MockStore {
        map: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl MockStore {
        pub fn new() -> Self {
            Self {
                map: HashMap::new(),
            }
        }
    }
    #[async_trait]
    impl Store for MockStore {
        async fn get(&mut self, _bucket: &str, key: &Key) -> Result<Option<Value>> {
            Ok(self.map.get(&key.0).cloned().map(Value))
        }

        async fn put(&mut self, _bucket: &str, key: &Key, value: &Value) -> Result<()> {
            self.map.insert(key.0.clone(), value.0.clone());
            Ok(())
        }

        async fn delete(&mut self, _bucket: &str, key: &Key) -> Result<()> {
            self.map.remove(&key.0);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_get() -> Result<()> {
        let mut operation = Operation {
            in_memory_store: MockStore::new(),
            on_disk_store: MockStore::new(),
            cloud_store: MockStore::new(),
        };

        let bucket = "bucket";
        let key = Key(vec![1, 2, 3]);
        let value = Value(vec![4, 5, 6]);

        // Initially, get should return None.
        assert!(operation.get(bucket, &key).await?.is_none());

        // After putting a value, get should return the value.
        operation.put(bucket, &key, &value).await?;
        assert_eq!(operation.get(bucket, &key).await?, Some(value.clone()));

        Ok(())
    }

    #[tokio::test]
    async fn test_put() -> Result<()> {
        let mut operation = Operation {
            in_memory_store: MockStore::new(),
            on_disk_store: MockStore::new(),
            cloud_store: MockStore::new(),
        };

        let bucket = "bucket";
        let key = Key(vec![1, 2, 3]);
        let value = Value(vec![4, 5, 6]);

        operation.put(bucket, &key, &value).await?;
        assert_eq!(operation.get(bucket, &key).await?, Some(value));

        Ok(())
    }

    #[tokio::test]
    async fn test_delete() -> Result<()> {
        let mut operation = Operation {
            in_memory_store: MockStore::new(),
            on_disk_store: MockStore::new(),
            cloud_store: MockStore::new(),
        };

        let bucket = "bucket";
        let key = Key(vec![1, 2, 3]);
        let value = Value(vec![4, 5, 6]);

        operation.put(bucket, &key, &value).await?;
        operation.delete(bucket, &key).await?;
        assert!(operation.get(bucket, &key).await?.is_none());

        Ok(())
    }
}
