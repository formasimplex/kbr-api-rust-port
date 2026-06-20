use rs_vips::voption::Setter;
use s3::bucket::Bucket as S3Bucket;
use s3::creds::Credentials;
use s3::error::S3Error;
use s3::region::Region;
use sha2::Digest;
use sqlx::PgPool;

use crate::error::AppError;

pub struct S3Config {
    pub access_key: String,
    pub secret_key: String,
    pub endpoint: String,
    pub bucket_name: String,
    pub region: String,
}

impl S3Config {
    pub fn from_env() -> Result<Self, AppError> {
        Ok(Self {
            access_key: std::env::var("S3_ACCESS_KEY")
                .map_err(|_| AppError::Storage("S3_ACCESS_KEY not set".to_string()))?,
            secret_key: std::env::var("S3_SECRET_KEY")
                .map_err(|_| AppError::Storage("S3_SECRET_KEY not set".to_string()))?,
            endpoint: std::env::var("S3_ENDPOINT")
                .map_err(|_| AppError::Storage("S3_ENDPOINT not set".to_string()))?,
            bucket_name: std::env::var("S3_BUCKET")
                .map_err(|_| AppError::Storage("S3_BUCKET not set".to_string()))?,
            region: std::env::var("S3_REGION")
                .unwrap_or_else(|_| "us-southeast-1".to_string()),
        })
    }
}

pub fn create_s3_bucket(config: &S3Config) -> Result<Box<S3Bucket>, AppError> {
    let creds = Credentials::new(Some(&config.access_key), Some(&config.secret_key), None, None, None)
        .map_err(|e| AppError::Storage(format!("Invalid S3 credentials: {}", e)))?;

    let region = Region::Custom {
        region: config.region.clone(),
        endpoint: config.endpoint.clone(),
    };

    let bucket = S3Bucket::new(&config.bucket_name, region, creds)
        .map_err(|e| AppError::Storage(format!("Failed to create S3 bucket: {}", e)))?;

    Ok(bucket.with_path_style())
}

/// Trait abstracting S3 operations for testability.
#[async_trait::async_trait]
pub trait S3Ops: Send + Sync {
    async fn put_object(&self, key: &str, data: &[u8], content_type: &str) -> Result<(), AppError>;
    async fn presign_get(&self, key: &str, expiry_secs: u32) -> Result<String, AppError>;
}

#[async_trait::async_trait]
impl S3Ops for S3Bucket {
    async fn put_object(&self, key: &str, data: &[u8], content_type: &str) -> Result<(), AppError> {
        let _ = self
            .put_object_with_content_type(key, data, content_type)
            .await
            .map_err(|e| AppError::Storage(format!("S3 upload failed: {}", e)))?;
        Ok(())
    }

    async fn presign_get(&self, key: &str, expiry_secs: u32) -> Result<String, AppError> {
        self.presign_get(key, expiry_secs, None)
            .await
            .map_err(|e: S3Error| AppError::Storage(format!("Failed to generate presigned URL: {}", e)))
    }
}

#[async_trait::async_trait]
impl S3Ops for Box<S3Bucket> {
    async fn put_object(&self, key: &str, data: &[u8], content_type: &str) -> Result<(), AppError> {
        let _ = self
            .as_ref()
            .put_object_with_content_type(key, data, content_type)
            .await
            .map_err(|e| AppError::Storage(format!("S3 upload failed: {}", e)))?;
        Ok(())
    }

    async fn presign_get(&self, key: &str, expiry_secs: u32) -> Result<String, AppError> {
        self.as_ref()
            .presign_get(key, expiry_secs, None)
            .await
            .map_err(|e: S3Error| AppError::Storage(format!("Failed to generate presigned URL: {}", e)))
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BlobRecord {
    pub id: i64,
    pub key: String,
    pub filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub variant_key: Option<String>,
}

const PRESIGNED_URL_EXPIRY_SECS: u32 = 3600;

pub async fn upload_file(
    s3: &dyn S3Ops,
    db: &PgPool,
    key: &str,
    data: &[u8],
    filename: &str,
    content_type: &str,
) -> Result<i64, AppError> {
    s3.put_object(key, data, content_type).await?;

    let metadata = serde_json::json!({
        "analyzed": true
    });

    let checksum = sha256::digest(data);

    let blob_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO active_storage_blobs
           (key, filename, content_type, metadata, byte_size, checksum, created_at, service_name)
           VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7)
           RETURNING id"#
    )
    .bind(key)
    .bind(filename)
    .bind(content_type)
    .bind(metadata.to_string())
    .bind(data.len() as i64)
    .bind(checksum)
    .bind("kbr")
    .fetch_one(db)
    .await
    .map_err(AppError::from)?;

    Ok(blob_id)
}

pub async fn attach_blob(
    db: &PgPool,
    record_type: &str,
    record_id: i64,
    name: &str,
    blob_id: i64,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO active_storage_attachments
           (name, record_type, record_id, blob_id, created_at)
           VALUES ($1, $2, $3, $4, NOW())
           ON CONFLICT (record_type, record_id, name, blob_id) DO NOTHING"#
    )
    .bind(name)
    .bind(record_type)
    .bind(record_id)
    .bind(blob_id)
    .execute(db)
    .await
    .map_err(AppError::from)?;

    Ok(())
}

/// Uploads a file to S3, records the blob, and attaches it to a record.
/// Returns the blob_id.
pub async fn upload_and_attach(
    s3: &dyn S3Ops,
    db: &PgPool,
    record_type: &str,
    record_id: i64,
    attachment_name: &str,
    data: &[u8],
    filename: &str,
    content_type: &str,
) -> Result<i64, AppError> {
    let key = generate_blob_key(filename);
    let blob_id = upload_file(s3, db, &key, data, filename, content_type).await?;
    attach_blob(db, record_type, record_id, attachment_name, blob_id).await?;
    Ok(blob_id)
}

/// Uploads a file to S3 with a pre-generated key, records the blob, and attaches it.
/// Returns `(blob_id, key)`.
pub async fn upload_and_attach_with_key(
    s3: &dyn S3Ops,
    db: &PgPool,
    record_type: &str,
    record_id: i64,
    attachment_name: &str,
    data: &[u8],
    filename: &str,
    content_type: &str,
) -> Result<(i64, String), AppError> {
    let key = generate_blob_key(filename);
    let blob_id = upload_file(s3, db, &key, data, filename, content_type).await?;
    attach_blob(db, record_type, record_id, attachment_name, blob_id).await?;
    Ok((blob_id, key))
}

fn generate_blob_key(filename: &str) -> String {
    let uuid_key = uuid::Uuid::new_v4().to_string();
    format!("blobs/{}/{}", uuid_key, filename)
}

/// Processes image variants (resize + WebP encode), uploads each to S3,
/// and records them in `active_storage_variant_records`.
/// Returns the presigned URLs for each variant in the same order as `sizes`.
pub async fn create_variants(
    s3: &dyn S3Ops,
    db: &PgPool,
    original_blob_id: i64,
    data: &[u8],
    uuid_key: &str,
    original_filename: &str,
    sizes: &[i32],
) -> Result<Vec<String>, AppError> {
    let extension = std::path::Path::new(original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let variant_filename = format!(
        "{}.webp",
        original_filename.strip_suffix(extension).unwrap_or(original_filename)
    );

    let img = rs_vips::VipsImage::new_from_buffer(data, "")
        .map_err(|e| AppError::Storage(format!("Failed to load image: {}", e)))?;

    let mut urls = Vec::new();

    for &max_width in sizes {
        let resized = img
            .thumbnail_image(max_width)
            .map_err(|e| AppError::Storage(format!("Failed to resize image: {}", e)))?;

        let webp_data = resized
            .webpsave_buffer_with_opts(rs_vips::voption::VOption::new().set("Q", 80i32))
            .map_err(|e| AppError::Storage(format!("Failed to encode WebP: {}", e)))?;

        let variant_key = format!("variants/{}/{}/{}", uuid_key, max_width, variant_filename);

        let _variant_blob_id =
            upload_file(s3, db, &variant_key, &webp_data, &variant_filename, "image/webp").await?;

        let variation_digest = compute_variation_digest(max_width, max_width);

        let _ = sqlx::query(
            r#"INSERT INTO active_storage_variant_records (blob_id, variation_digest)
               VALUES ($1, $2)
               ON CONFLICT (blob_id, variation_digest) DO NOTHING"#,
        )
        .bind(original_blob_id)
        .bind(variation_digest)
        .execute(db)
        .await;

        urls.push(variant_key);
    }

    Ok(urls)
}

pub async fn generate_presigned_url(
    s3: &dyn S3Ops,
    key: &str,
) -> Result<String, AppError> {
    s3.presign_get(key, PRESIGNED_URL_EXPIRY_SECS).await
}

pub async fn get_image_urls(
    s3: &dyn S3Ops,
    db: &PgPool,
    record_type: &str,
    record_id: i64,
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let originals: Vec<BlobRecord> = sqlx::query_as(
        r#"
        SELECT
            b.id,
            b.key,
            b.filename,
            b.content_type,
            b.byte_size,
            NULL::TEXT AS variant_key
        FROM active_storage_attachments a
        JOIN active_storage_blobs b ON b.id = a.blob_id
        WHERE a.record_type = $1
          AND a.record_id = $2
          AND a.name = 'images'
        ORDER BY a.created_at
        "#
    )
    .bind(record_type)
    .bind(record_id)
    .fetch_all(db)
    .await?;

    let uuids: Vec<String> = originals
        .iter()
        .filter_map(|b| {
            b.key
                .split('/')
                .enumerate()
                .find(|(i, segment)| *i >= 1 && segment.len() == 36)
                .map(|(_, segment)| segment.to_string())
        })
        .collect();

    let mut variant_keys: Vec<String> = Vec::new();
    for uuid in &uuids {
        let pattern = format!("variants/{}/%", uuid);
        let variants: Vec<(String,)> = sqlx::query_as(
            "SELECT key FROM active_storage_blobs WHERE key LIKE $1"
        )
        .bind(&pattern)
        .fetch_all(db)
        .await?;
        variant_keys.extend(variants.into_iter().map(|v| v.0));
    }

    let mut urls: Vec<String> = Vec::new();
    let mut thumbnail_urls: Vec<String> = Vec::new();

    for vkey in variant_keys {
        let url = generate_presigned_url(s3, &vkey).await?;
        let size_str = vkey
            .split('/')
            .nth(2)
            .unwrap_or("");
        match size_str {
            "100" => thumbnail_urls.push(url),
            _ => urls.push(url),
        }
    }

    if urls.is_empty() && thumbnail_urls.is_empty() {
        for blob in &originals {
            let url = generate_presigned_url(s3, &blob.key).await?;
            urls.push(url);
        }
    }

    Ok((urls, thumbnail_urls))
}

pub async fn delete_blob(
    s3: &S3Bucket,
    db: &PgPool,
    blob_id: i64,
) -> Result<(), AppError> {
    let blob: Option<(String,)> = sqlx::query_as(
        r#"SELECT key FROM active_storage_blobs WHERE id = $1"#
    )
    .bind(blob_id)
    .fetch_optional(db)
    .await?;

    if let Some((key,)) = blob {
        let _ = s3.delete_object(&key).await;

        sqlx::query(
            r#"DELETE FROM active_storage_attachments WHERE blob_id = $1"#
        )
        .bind(blob_id)
        .execute(db)
        .await?;

        sqlx::query(
            r#"DELETE FROM active_storage_blobs WHERE id = $1"#
        )
        .bind(blob_id)
        .execute(db)
        .await?;
    }

    Ok(())
}

pub fn compute_variation_digest(width: i32, height: i32) -> String {
    let variation = format!("resize_to_limit:{width}x{height}#resize_to_limit:{width}x{height}");
    let hash = sha2::Sha256::digest(variation.as_bytes());
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Mock S3 that records calls and always succeeds.
    struct MockS3 {
        pub put_called: Arc<AtomicBool>,
        pub put_count: Arc<AtomicUsize>,
        pub put_keys: Arc<std::sync::Mutex<Vec<String>>>,
        pub presign_called: Arc<AtomicBool>,
    }

    impl MockS3 {
        fn new() -> Self {
            Self {
                put_called: Arc::new(AtomicBool::new(false)),
                put_count: Arc::new(AtomicUsize::new(0)),
                put_keys: Arc::new(std::sync::Mutex::new(Vec::new())),
                presign_called: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    #[async_trait::async_trait]
    impl S3Ops for MockS3 {
        async fn put_object(&self, key: &str, _data: &[u8], _content_type: &str) -> Result<(), AppError> {
            self.put_called.store(true, Ordering::SeqCst);
            self.put_count.fetch_add(1, Ordering::SeqCst);
            self.put_keys.lock().unwrap().push(key.to_string());
            Ok(())
        }

        async fn presign_get(&self, _key: &str, _expiry_secs: u32) -> Result<String, AppError> {
            self.presign_called.store(true, Ordering::SeqCst);
            Ok("https://mock.example.com/url".to_string())
        }
    }

    /// Mock S3 that fails on put_object.
    struct FailingS3;

    #[async_trait::async_trait]
    impl S3Ops for FailingS3 {
        async fn put_object(&self, _key: &str, _data: &[u8], _content_type: &str) -> Result<(), AppError> {
            Err(AppError::Storage("mock S3 failure".to_string()))
        }

        async fn presign_get(&self, _key: &str, _expiry_secs: u32) -> Result<String, AppError> {
            Ok("https://mock.example.com/url".to_string())
        }
    }

    async fn get_test_pool() -> PgPool {
        let url = crate::test_utils::test_db_url();
        sqlx::pool::PoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upload_and_attach_creates_blob_and_attachment() {
        crate::test_utils::set_test_env();
        let pool = get_test_pool().await;
        let guard = crate::test_utils::TestGuard::new(&pool).await;

        let s3 = MockS3::new();
        let put_called = s3.put_called.clone();
        let data = b"fake image data";

        let blob_id = upload_and_attach(
            &s3,
            &pool,
            "Artist",
            42,
            "images",
            data,
            "test.jpg",
            "image/jpeg",
        )
        .await
        .expect("upload_and_attach should succeed");

        assert!(put_called.load(Ordering::SeqCst), "S3 put_object should be called");
        assert!(blob_id > 0, "blob_id should be positive");

        // Verify blob exists in DB
        let blob_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM active_storage_blobs WHERE id = $1")
            .bind(blob_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to query blob");
        assert_eq!(blob_count, 1);

        // Verify attachment exists in DB
        let attach_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM active_storage_attachments WHERE blob_id = $1 AND record_type = $2 AND record_id = $3 AND name = $4"
        )
        .bind(blob_id)
        .bind("Artist")
        .bind(42i64)
        .bind("images")
        .fetch_one(&pool)
        .await
        .expect("Failed to query attachment");
        assert_eq!(attach_count, 1);

        guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upload_and_attach_generates_valid_key_format() {
        crate::test_utils::set_test_env();
        let pool = get_test_pool().await;
        let guard = crate::test_utils::TestGuard::new(&pool).await;

        let s3 = MockS3::new();
        let put_keys = s3.put_keys.clone();
        let data = b"fake image data";

        upload_and_attach(
            &s3,
            &pool,
            "Artist",
            42,
            "images",
            data,
            "photo.png",
            "image/png",
        )
        .await
        .expect("upload_and_attach should succeed");

        let keys = put_keys.lock().unwrap();
        assert_eq!(keys.len(), 1);
        let key = &keys[0];

        // Key format: blobs/{uuid}/photo.png
        assert!(key.starts_with("blobs/"), "key should start with blobs/");
        assert!(key.ends_with("/photo.png"), "key should end with /photo.png");

        let parts: Vec<&str> = key.split('/').collect();
        assert_eq!(parts.len(), 3, "key should have 3 parts: blobs, uuid, filename");
        assert_eq!(parts[0], "blobs");
        assert_eq!(parts[2], "photo.png");

        // UUID segment should be 36 chars
        assert_eq!(parts[1].len(), 36, "UUID segment should be 36 characters");

        guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upload_and_attach_returns_blob_id() {
        crate::test_utils::set_test_env();
        let pool = get_test_pool().await;
        let guard = crate::test_utils::TestGuard::new(&pool).await;

        let s3 = MockS3::new();
        let data = b"fake image data";

        let blob_id = upload_and_attach(
            &s3,
            &pool,
            "KbrEvent",
            99,
            "images",
            data,
            "event.jpg",
            "image/jpeg",
        )
        .await
        .expect("upload_and_attach should succeed");

        assert!(blob_id > 0);

        guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upload_and_attach_fails_on_upload_error() {
        crate::test_utils::set_test_env();
        let pool = get_test_pool().await;
        let guard = crate::test_utils::TestGuard::new(&pool).await;

        let s3 = FailingS3;
        let data = b"fake image data";

        let result = upload_and_attach(
            &s3,
            &pool,
            "Artist",
            42,
            "images",
            data,
            "test.jpg",
            "image/jpeg",
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_str = format!("{}", err);
        assert!(
            err_str.contains("mock S3 failure"),
            "error should propagate from S3: {}",
            err_str
        );

        guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upload_and_attach_records_correct_metadata() {
        crate::test_utils::set_test_env();
        let pool = get_test_pool().await;
        let guard = crate::test_utils::TestGuard::new(&pool).await;

        let s3 = MockS3::new();
        let data = b"fake image data";

        let blob_id = upload_and_attach(
            &s3,
            &pool,
            "Artist",
            42,
            "images",
            data,
            "test.jpg",
            "image/jpeg",
        )
        .await
        .expect("upload_and_attach should succeed");

        let blob: BlobRecord = sqlx::query_as(
            "SELECT id, key, filename, content_type, byte_size, NULL::TEXT AS variant_key
             FROM active_storage_blobs WHERE id = $1"
        )
        .bind(blob_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to query blob");

        assert_eq!(blob.filename, "test.jpg");
        assert_eq!(blob.content_type, "image/jpeg");
        assert_eq!(blob.byte_size, data.len() as i64);

        guard.cleanup().await;
    }

    #[test]
    fn variation_digest_is_deterministic() {
        let d1 = compute_variation_digest(512, 512);
        let d2 = compute_variation_digest(512, 512);
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }

    #[test]
    fn variation_digest_differs_for_different_sizes() {
        let d1 = compute_variation_digest(512, 512);
        let d2 = compute_variation_digest(100, 100);
        assert_ne!(d1, d2);
    }
}
