use sqlx::PgPool;

use crate::error::AppError;

use super::blob::BlobRecord;
use super::s3_ops::S3Ops;

const PRESIGNED_URL_EXPIRY_SECS: u32 = 3600;

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

    let patterns: Vec<String> = uuids.iter().map(|u| format!("variants/{}/%", u)).collect();

    let variant_keys: Vec<String> = if patterns.is_empty() {
        Vec::new()
    } else {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT key FROM active_storage_blobs WHERE key LIKE ANY($1)"
        )
        .bind(&patterns)
        .fetch_all(db)
        .await?;
        rows.into_iter().map(|v| v.0).collect()
    };

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
