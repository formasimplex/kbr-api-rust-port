use s3::bucket::Bucket as S3Bucket;
use s3::error::S3Error;

use crate::error::AppError;

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
