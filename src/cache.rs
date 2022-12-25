use std::error::Error;

use crate::store::Store;

struct Cache<I: Store, O: Store, C: Store> {
    in_memory_store: I,
    on_disk_store: O,
    cloud_storage: C,
}

impl<I, O, C> Cache<I, O, C>
where
    I: Store + Copy,
    O: Store + Copy,
    C: Store + Copy,
{
    async fn get(
        &mut self,
        bucket: &str,
        key: &[u8],
        from_cache: Option<bool>,
    ) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let read_from_cache = from_cache.unwrap_or(true);
        if read_from_cache {
            let in_memory_data = &self.in_memory_store.get(bucket, key).await;
            match in_memory_data {
                Ok(Some(data)) => Ok(Some(data.clone())),
                Ok(None) => match &self.on_disk_store.get(bucket, key).await {
                    Ok(Some(v)) => {
                        self.in_memory_store.put(bucket, key, v).await?;
                        Ok(Some(v.clone()))
                    }
                    Ok(None) => {
                        let cloud_data = self.cloud_storage.get(bucket, key).await;
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
                    Err(_e) => self.cloud_storage.get(bucket, key).await, //todo fix this
                },
                Err(_e) => self.cloud_storage.get(bucket, key).await,
            }
        } else {
            self.cloud_storage.get(bucket, key).await
        }
    }

    async fn put(&mut self, bucket: &str, key: &[u8], value: &[u8]) -> Result<(), Box<dyn Error>> {
        self.cloud_storage.put(bucket, key, value).await?;
        self.on_disk_store.put(bucket, key, value).await?;
        self.in_memory_store.put(bucket, key, value).await
    }

    async fn delete(&mut self, bucket: &str, key: &[u8]) -> Result<(), Box<dyn Error>> {
        self.cloud_storage.delete(bucket, key).await?;
        self.on_disk_store.delete(bucket, key).await?;
        self.in_memory_store.delete(bucket, key).await
    }
}
