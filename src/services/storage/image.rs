use rs_vips::voption::Setter;
use rs_vips::VipsImage;
use sha2::Digest;
use sqlx::PgPool;

use crate::error::AppError;

use super::blob::upload_file;
use super::s3_ops::S3Ops;

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

    let img = VipsImage::new_from_buffer(data, "")
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

pub fn compute_variation_digest(width: i32, height: i32) -> String {
    let variation = format!("resize_to_limit:{width}x{height}#resize_to_limit:{width}x{height}");
    let hash = sha2::Sha256::digest(variation.as_bytes());
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

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
