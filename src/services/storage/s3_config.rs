use s3::bucket::Bucket as S3Bucket;
use s3::creds::Credentials;
use s3::region::Region;

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
