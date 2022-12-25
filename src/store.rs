use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    path::Path,
};

use lru::LruCache;

use rocksdb::Options;

#[tonic::async_trait]
pub trait Store {
    async fn get(
        self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>>;
    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn delete(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

struct LRUStore {
    cache: LruCache<Vec<u8>, Vec<u8>>,
}

impl LRUStore {
    fn new(capacity: u64) -> Self {
        let cache = LruCache::new(NonZeroUsize::new(capacity.try_into().unwrap()).unwrap());
        LRUStore { cache }
    }
}

#[tonic::async_trait]
impl Store for LRUStore {
    async fn get(
        mut self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key);
        let value = self.cache.get(&bucket_with_key);
        Ok(value.cloned())
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key);
        self.cache.put(bucket_with_key, value.clone());
        Ok(())
    }

    async fn delete(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bucket_with_key = bucket.as_bytes().to_vec();
        bucket_with_key.extend(b"/");
        bucket_with_key.extend(key);
        self.cache.pop_entry(&bucket_with_key);
        Ok(())
    }
}

struct DiskStore {
    db: rocksdb::DB,
}

impl DiskStore {
    fn new<P: AsRef<Path>>(opts: &Options, path: P) -> Self {
        let db = rocksdb::DB::open(opts, path).expect("could not open rocksdb for path given");
        DiskStore { db }
    }
}
#[tonic::async_trait]
impl Store for DiskStore {
    async fn get(
        self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let result = self
            .db
            .get(build_cache_key(&bucket.as_bytes().to_vec(), key));

        match result {
            Ok(v) => Ok(v),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .db
            .put(build_cache_key(&bucket.as_bytes().to_vec(), key), value);
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn delete(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .db
            .delete(build_cache_key(&bucket.as_bytes().to_vec(), key));
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

struct S3Store {
    client: aws_sdk_s3::Client,
}

#[tonic::async_trait]
impl Store for S3Store {
    async fn get(
        self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let result = self
            .client
            .get_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(&bucket.as_bytes().to_vec(), key).as_slice(),
            )?)
            .send()
            .await?
            .body
            .collect()
            .await;

        match result {
            Ok(v) => Ok(Some(v.to_vec())),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn put(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
        value: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .client
            .put_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(&bucket.as_bytes().to_vec(), key).as_slice(),
            )?)
            .body(value.clone().into())
            .send()
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn delete(
        &mut self,
        bucket: &str,
        key: &Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let result = self
            .client
            .delete_object()
            .bucket(bucket)
            .key(std::str::from_utf8(
                build_cache_key(&bucket.as_bytes().to_vec(), key).as_slice(),
            )?)
            .send()
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(Box::new(e)),
        }
    }
}

fn build_cache_key(bucket: &Vec<u8>, key: &Vec<u8>) -> Vec<u8> {
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
