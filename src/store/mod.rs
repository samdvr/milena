use anyhow::Result;
use aws_sdk_s3::{
    error::{GetObjectError, NoSuchKey},
    types::{ByteStream, SdkError},
};
use lru::LruCache;
use tonic::async_trait;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    num::NonZeroUsize,
    path::Path,
    time::Duration,
};

use rocksdb::Options;
#[derive(Clone, Debug, PartialEq)]
pub struct Key(pub Vec<u8>);
#[derive(Clone, Debug, PartialEq)]
pub struct Value(pub Vec<u8>);

#[tonic::async_trait]
pub trait Store {
    async fn get(&mut self, bucket: &str, key: &Key) -> Result<Option<Value>>;
    async fn put(&mut self, bucket: &str, key: &Key, value: &Value) -> Result<()>;
    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<()>;
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
    async fn get(&mut self, bucket: &str, key: &Key) -> Result<Option<Value>> {
        let data = self.cache.get(&build_cache_key(bucket.as_bytes(), key).0);
        Ok(data.map(|x| Value(x.clone())))
    }

    async fn put(&mut self, bucket: &str, key: &Key, value: &Value) -> Result<()> {
        self.cache
            .put(build_cache_key(bucket.as_bytes(), key).0, value.clone().0);

        Ok(())
    }

    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<()> {
        self.cache
            .pop_entry(&build_cache_key(bucket.as_bytes(), key).0);
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
    async fn get(&mut self, bucket: &str, key: &Key) -> Result<Option<Value>> {
        let result = self
            .db
            .get(build_cache_key(bucket.as_bytes(), key).0)?
            .map(Value);

        Ok(result)
    }

    async fn put(&mut self, bucket: &str, key: &Key, value: &Value) -> Result<()> {
        self.db
            .put(build_cache_key(bucket.as_bytes(), key).0, &value.0)?;
        Ok(())
    }

    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<()> {
        self.db.delete(build_cache_key(bucket.as_bytes(), key).0)?;
        Ok(())
    }
}

pub struct S3Store {
    pub client: aws_sdk_s3::Client,
}

#[async_trait]
impl Store for S3Store {
    async fn get(&mut self, bucket: &str, key: &Key) -> Result<Option<Value>> {
        let data = self
            .client
            .get_object()
            .bucket(bucket)
            .key(std::str::from_utf8(build_cache_key(bucket.as_bytes(), key).0.as_slice()).unwrap())
            .send()
            .await;
        match data {
            Ok(v) => Ok(Some(Value(v.body.collect().await.unwrap().to_vec()))),
            Err(e) => {
                let error = e.into_service_error();

                Err(error.into())
            }
        }
    }

    async fn put(&mut self, bucket: &str, key: &Key, value: &Value) -> Result<()> {
        let _result = self
            .client
            .put_object()
            .bucket(bucket)
            .key(std::str::from_utf8(build_cache_key(bucket.as_bytes(), key).0.as_slice()).unwrap())
            .body(ByteStream::from(value.clone().0))
            .send()
            .await?;
        Ok(())
    }

    async fn delete(&mut self, bucket: &str, key: &Key) -> Result<()> {
        let _result = self
            .client
            .delete_object()
            .bucket(bucket)
            .key(std::str::from_utf8(build_cache_key(bucket.as_bytes(), key).0.as_slice()).unwrap())
            .send()
            .await?;

        Ok(())
    }
}

fn build_cache_key(bucket: &[u8], key: &Key) -> Key {
    let shard_key = ((calculate_hash(&key.0) % 256) + 1).to_string();

    let mut key_vec = vec![];
    key_vec.extend(shard_key.as_bytes());
    key_vec.extend("/".as_bytes());

    let mut key_to_md5 = key_vec.clone();
    key_to_md5.extend(&key.0);
    key_to_md5.extend(bucket);

    let digest = format!("{:x}", md5::compute(&key_to_md5))
        .as_bytes()
        .to_vec();
    key_vec.extend(digest);
    /*
    println!(
        "key: {:?}",
        std::str::from_utf8(key_vec.clone().as_slice()).unwrap()
    );
    */

    Key(key_vec)
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
        "254f1c075eea17b845c836c9e1ebee1d0cd"
    );
}
#[tokio::test]
async fn test_lru_store_methods() {
    let mut store = LRUStore::new(100);
    let bucket = "bucket";
    let key = Key("key".as_bytes().to_vec());
    let value = Value("value".as_bytes().to_vec());

    // Test put
    store.put(bucket, &key, &value).await.unwrap();
    assert_eq!(store.cache.len(), 1);

    // Test get
    let result = store.get(bucket, &key).await.unwrap();
    assert_eq!(result.unwrap(), value.clone());

    // Test delete
    store.delete(bucket, &key).await.unwrap();
    assert_eq!(store.cache.len(), 0);
}
