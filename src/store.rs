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

#[tonic::async_trait]
pub trait Store {
    async fn get(
        self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<ByteStream>, Box<dyn std::error::Error>>;
    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: ByteStream,
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn delete(&self, bucket: &str, key: &Vec<u8>) -> Result<(), Box<dyn std::error::Error>>;
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
        self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<ByteStream>, Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key);
        let value = self
            .cache
            .get(&bucket_with_key)
            .map(|x| ByteStream::from_static(x.as_slice()));
        Ok(value)
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: ByteStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key);
        self.cache
            .put(bucket_with_key, value.collect().await.unwrap().to_vec());

        Ok(())
    }

    async fn delete(&self, bucket: &str, key: &Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key);
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
        self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<ByteStream>, Box<dyn std::error::Error>> {
        let result = self.db.get(build_cache_key(bucket.as_bytes(), key));

        match result {
            Ok(v) => Ok(v.map(|x| ByteStream::from(x))),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: ByteStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.db.put(
            build_cache_key(bucket.as_bytes(), key),
            value.collect().await.unwrap().to_vec(),
        );
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn delete(&self, bucket: &str, key: &Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.db.delete(build_cache_key(bucket.as_bytes(), key));
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
        self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<ByteStream>, Box<dyn std::error::Error>> {
        Ok(Some(
            self.client
                .get_object()
                .bucket(bucket)
                .key(std::str::from_utf8(
                    build_cache_key(bucket.as_bytes(), key).as_slice(),
                )?)
                .send()
                .await?
                .body,
        ))
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: ByteStream,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .client
            .put_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(bucket.as_bytes(), key).as_slice(),
            )?)
            .body(value)
            .send()
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn delete(&self, bucket: &str, key: &Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .client
            .delete_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(bucket.as_bytes(), key).as_slice(),
            )?)
            .send()
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

fn build_cache_key(bucket: &[u8], key: &[u8]) -> Vec<u8> {
    let shard_key = ((calculate_hash(&key) % 256) + 1).to_string();
    let mut key_vec = bucket.to_owned();
    key_vec.extend("/".as_bytes().to_vec());
    key_vec.extend(shard_key.as_bytes().to_vec());
    key_vec.extend("/".as_bytes().to_vec());

    let mut key_to_md5 = key_vec.clone();
    for i in key {
        key_to_md5.push(*i);
    }

    let digest = format!("{:x}", md5::compute(&key_to_md5))
        .as_bytes()
        .to_vec();
    key_vec.extend(digest);
    key_vec.to_vec()
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
    let result = build_cache_key(&a, &b);

    assert_eq!(
        String::from_utf8_lossy(result.as_slice()),
        "topic/254/5266607d733dccfade57904238347f03"
    );
}
