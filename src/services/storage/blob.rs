use s3::bucket::Bucket as S3Bucket;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::error::AppError;

use super::s3_ops::S3Ops;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BlobRecord {
    pub id: i64,
    pub key: String,
    pub filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub variant_key: Option<String>,
}

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

    let checksum = hex::encode(Sha256::digest(data));

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

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

        let blob_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM active_storage_blobs WHERE id = $1")
            .bind(blob_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to query blob");
        assert_eq!(blob_count, 1);

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

        assert!(key.starts_with("blobs/"), "key should start with blobs/");
        assert!(key.ends_with("/photo.png"), "key should end with /photo.png");

        let parts: Vec<&str> = key.split('/').collect();
        assert_eq!(parts.len(), 3, "key should have 3 parts: blobs, uuid, filename");
        assert_eq!(parts[0], "blobs");
        assert_eq!(parts[2], "photo.png");

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
}
