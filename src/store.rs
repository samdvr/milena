use aws_sdk_s3::types::ByteStream;

use lru::LruCache;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    path::Path,
    time::Duration,
};

use rocksdb::Options;
#[derive(Clone)]
pub struct Key(pub Vec<u8>);
#[derive(Clone)]
pub struct Value(pub Vec<u8>);

#[tonic::async_trait]
pub trait Store {
    async fn get(
        &mut self,
        bucket: &str,
        key: &Key,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>>;
    async fn put(
        &mut self,
        bucket: &str,
        key: &Key,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct LRUStore {
    cache: LruCache<Vec<u8>, Vec<u8>>,
}

impl LRUStore {
    pub fn new(capacity: u64) -> Self {
        let cache = LruCache::new(NonZeroUsize::new(capacity.try_into().unwrap()).unwrap());
        LRUStore { cache }
    }
}

#[tonic::async_trait]
impl Store for LRUStore {
    async fn get(
        &mut self,
        bucket: &str,
        key: &Key,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key.0.clone());
        let data = self.cache.get(&bucket_with_key);
        Ok(data.map(|x| Value(x.clone())))
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Key,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key.clone().0);
        self.cache.put(bucket_with_key, value.clone().0);

        Ok(())
    }

    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<(), Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key.clone().0);
        self.cache.pop_entry(&bucket_with_key);
        Ok(())
    }
}

pub struct DiskStore {
    db: rocksdb::DB,
}

impl DiskStore {
    pub fn new<P: AsRef<Path>>(opts: &Options, ttl: Duration, path: P) -> Self {
        let db = rocksdb::DB::open_with_ttl(opts, path, ttl)
            .expect("could not open rocksdb for path given");
        DiskStore { db }
    }
}
#[tonic::async_trait]
impl Store for DiskStore {
    async fn get(
        &mut self,
        bucket: &str,
        key: &Key,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let result = self.db.get(build_cache_key(bucket.as_bytes(), &key).0);

        match result {
            Ok(v) => Ok(v.map(|x| Value(x))),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Key,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .db
            .put(build_cache_key(bucket.as_bytes(), key).0, &value.0);
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.db.delete(build_cache_key(bucket.as_bytes(), key).0);
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

pub struct S3Store {
    pub client: aws_sdk_s3::Client,
    pub throttle: Duration,
}

#[tonic::async_trait]
impl Store for S3Store {
    async fn get(
        &mut self,
        bucket: &str,
        key: &Key,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        let data = self
            .client
            .get_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(bucket.as_bytes(), key).0.as_slice(),
            )?)
            .send()
            .await?
            .body
            .collect()
            .await
            .unwrap()
            .to_vec();
        Ok(Some(Value(data)))
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Key,
        value: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .client
            .put_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(bucket.as_bytes(), key).0.as_slice(),
            )?)
            .body(ByteStream::from(value.clone().0))
            .send()
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .client
            .delete_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(bucket.as_bytes(), key).0.as_slice(),
            )?)
            .send()
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

fn build_cache_key(bucket: &[u8], key: &Key) -> Key {
    let shard_key = ((calculate_hash(&key.0) % 256) + 1).to_string();
    let mut key_vec = bucket.to_owned();
    key_vec.extend("/".as_bytes().to_vec());
    key_vec.extend(shard_key.as_bytes().to_vec());
    key_vec.extend("/".as_bytes().to_vec());

    let mut key_to_md5 = key_vec.clone();
    for i in &key.0 {
        key_to_md5.push(*i);
    }

    let digest = format!("{:x}", md5::compute(&key_to_md5))
        .as_bytes()
        .to_vec();
    key_vec.extend(digest);
    Key(key_vec.to_vec())
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

#[test]
fn test_build_cache() {
    let a = "topic".as_bytes().to_vec();
    let b = "some_key".as_bytes().to_vec();
    let result = build_cache_key(&a, &Key(b));

    assert_eq!(
        String::from_utf8_lossy(result.0.as_slice()),
        "topic/254/5266607d733dccfade57904238347f03"
    );
}
