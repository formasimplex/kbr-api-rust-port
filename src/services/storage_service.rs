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
    s3: &S3Bucket,
    db: &PgPool,
    key: &str,
    data: &[u8],
    filename: &str,
    content_type: &str,
) -> Result<i64, AppError> {
    s3.put_object_with_content_type(key, data, content_type)
        .await
        .map_err(|e| AppError::Storage(format!("S3 upload failed: {}", e)))?;

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
    .map_err(|e| {
        let _ = s3.delete_object(key);
        e
    })
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

pub async fn generate_presigned_url(
    s3: &S3Bucket,
    key: &str,
) -> Result<String, AppError> {
    s3.presign_get(key, PRESIGNED_URL_EXPIRY_SECS, None)
        .await
        .map_err(|e: S3Error| AppError::Storage(format!("Failed to generate presigned URL: {}", e)))
}

pub async fn get_image_urls(
    s3: &S3Bucket,
    db: &PgPool,
    record_type: &str,
    record_id: i64,
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let blobs: Vec<BlobRecord> = sqlx::query_as(
        r#"
        SELECT
            b.id,
            b.key,
            b.filename,
            b.content_type,
            b.byte_size,
            vr.variation_digest AS variant_key
        FROM active_storage_attachments a
        JOIN active_storage_blobs b ON b.id = a.blob_id
        LEFT JOIN active_storage_variant_records vr
            ON vr.blob_id = b.id
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

    let mut urls: Vec<String> = Vec::new();
    let mut thumbnail_urls: Vec<String> = Vec::new();

    for blob in &blobs {
        let url = generate_presigned_url(s3, &blob.key).await?;
        if blob.variant_key.is_some() {
            thumbnail_urls.push(url);
        } else {
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
