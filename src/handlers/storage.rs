//! Storage handlers
//!
//! Provides endpoints for uploading, listing, and deleting images via
//! S3 storage. All endpoints require artist role or above.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `upload` | POST | `/v1/storage/upload` | artist+ | Upload an image to S3 storage |
//! | `get_images` | GET | `/v1/storage/images/{record_type}/{record_id}` | public | List images for a specific record type and ID |
//! | `delete_image` | DELETE | `/v1/storage/blob/{blob_id}` | artist+ | Delete an image blob from S3 storage |

use actix_multipart::Multipart;
use futures_util::StreamExt;
use rs_vips::voption::Setter;

use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::services::storage_service::{
    self, generate_presigned_url,
};

const MAX_UPLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB

/// Response for a successful image upload.
#[derive(Debug, serde::Serialize)]
pub struct UploadResponse {
    pub id: i64,
    pub key: String,
    pub filename: String,
    pub content_type: String,
    pub byte_size: i64,
    pub url: String,
    pub thumbnail_url: String,
}

/// Upload an image to S3 storage.
///
/// Requires artist role or above. Accepts multipart form data with
/// record_type, record_id, name (attachment name), and file fields.
/// Generates original, medium (512px), and thumbnail (100px) variants.
///
/// # Response
///
/// `200 OK` — `UploadResponse` with URLs for the uploaded image
/// `403 Forbidden` — insufficient role
/// `422 Unprocessable` — missing fields, invalid file type, or file too large (>10MB)
pub async fn upload(
    state: web::Data<AppState>,
    user: CurrentUser,
    mut payload: Multipart,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let mut record_type: Option<String> = None;
    let mut record_id: Option<i64> = None;
    let mut attachment_name: Option<String> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;

    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|e| AppError::Storage(format!("Multipart error: {}", e)))?;

        let field_name = field.name()
            .ok_or_else(|| AppError::Validation("Field name is required".to_string()))?
            .to_string();

        let data = field
            .bytes(MAX_UPLOAD_SIZE)
            .await
            .map_err(|e| AppError::Storage(format!("Failed to read field: {}", e)))?
            .map_err(|e| AppError::Storage(format!("Failed to read field: {}", e)))?
            .to_vec();

        match field_name.as_str() {
            "record_type" => {
                record_type = Some(String::from_utf8(data)
                    .map_err(|_| AppError::Validation("Invalid record_type".to_string()))?);
            }
            "record_id" => {
                let s = String::from_utf8(data)
                    .map_err(|_| AppError::Validation("Invalid record_id".to_string()))?;
                record_id = Some(s.trim().parse()
                    .map_err(|_| AppError::Validation("record_id must be an integer".to_string()))?);
            }
            "name" => {
                attachment_name = Some(String::from_utf8(data)
                    .map_err(|_| AppError::Validation("Invalid name".to_string()))?);
            }
            "file" => {
                if data.len() > MAX_UPLOAD_SIZE {
                    return Err(AppError::Validation(
                        format!("File too large. Maximum size is {}MB", MAX_UPLOAD_SIZE / 1024 / 1024)
                    ));
                }

                let ct = field.content_type()
                    .ok_or_else(|| AppError::Validation("Content type is required".to_string()))?;

                if !ct.to_string().starts_with("image/") {
                    return Err(AppError::Validation(
                        "Only image files are allowed".to_string()
                    ));
                }

                filename = field.content_disposition()
                    .and_then(|cd| cd.get_filename().map(|n| n.to_string()))
                    .or_else(|| Some("upload".to_string()));

                content_type = Some(ct.to_string());
                file_data = Some(data);
            }
            _ => {}
        }
    }

    let record_type = record_type.ok_or_else(|| {
        AppError::Validation("record_type is required".to_string())
    })?;
    let record_id = record_id.ok_or_else(|| {
        AppError::Validation("record_id is required".to_string())
    })?;
    let attachment_name = attachment_name.ok_or_else(|| {
        AppError::Validation("name (attachment name) is required".to_string())
    })?;
    let file_data = file_data.ok_or_else(|| {
        AppError::Validation("file is required".to_string())
    })?;
    let filename = filename.ok_or_else(|| {
        AppError::Validation("filename is required".to_string())
    })?;
    let content_type = content_type.ok_or_else(|| {
        AppError::Validation("content_type is required".to_string())
    })?;

    let uuid_key = uuid::Uuid::new_v4().to_string();
    let original_key = format!("blobs/{}/{}", uuid_key, filename);

    let original_blob_id = storage_service::upload_file(
        &state.s3,
        &state.db,
        &original_key,
        &file_data,
        &filename,
        &content_type,
    )
    .await?;

    storage_service::attach_blob(
        &state.db,
        &record_type,
        record_id,
        &attachment_name,
        original_blob_id,
    )
    .await?;

    let _medium_url = process_and_upload_variant(
        &state.s3,
        &state.db,
        &file_data,
        &uuid_key,
        &filename,
        original_blob_id,
        512,
    )
    .await?;

    let thumbnail_url = process_and_upload_variant(
        &state.s3,
        &state.db,
        &file_data,
        &uuid_key,
        &filename,
        original_blob_id,
        100,
    )
    .await?;

    let original_url = generate_presigned_url(&state.s3, &original_key).await?;

    Ok(HttpResponse::Ok().json(UploadResponse {
        id: original_blob_id,
        key: original_key,
        filename,
        content_type,
        byte_size: file_data.len() as i64,
        url: original_url,
        thumbnail_url,
    }))
}

async fn process_and_upload_variant(
    s3: &s3::bucket::Bucket,
    db: &sqlx::PgPool,
    data: &[u8],
    uuid_key: &str,
    original_filename: &str,
    original_blob_id: i64,
    max_width: i32,
) -> Result<String, AppError> {
    let img = rs_vips::VipsImage::new_from_buffer(data, "")
        .map_err(|e| AppError::Storage(format!("Failed to load image: {}", e)))?;

    let resized = img.thumbnail_image(max_width)
        .map_err(|e| AppError::Storage(format!("Failed to resize image: {}", e)))?;

    let webp_data = resized.webpsave_buffer_with_opts(
        rs_vips::voption::VOption::new()
            .set("Q", 80i32),
    )
    .map_err(|e| AppError::Storage(format!("Failed to encode WebP: {}", e)))?;

    let extension = std::path::Path::new(original_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let variant_filename = format!(
        "{}.webp",
        original_filename.strip_suffix(extension).unwrap_or(original_filename)
    );

    let variant_key = format!("variants/{}/{}/{}", uuid_key, max_width, variant_filename);

    let _variant_blob_id = storage_service::upload_file(
        s3,
        db,
        &variant_key,
        &webp_data,
        &variant_filename,
        "image/webp",
    )
    .await?;

    let variation_digest = storage_service::compute_variation_digest(max_width, max_width);

    // blob_id references the ORIGINAL blob, matching Rails ActiveStorage semantics
    let _ = sqlx::query(
        r#"INSERT INTO active_storage_variant_records (blob_id, variation_digest)
           VALUES ($1, $2)
           ON CONFLICT (blob_id, variation_digest) DO NOTHING"#
    )
    .bind(original_blob_id)
    .bind(variation_digest)
    .execute(db)
    .await;

    generate_presigned_url(s3, &variant_key).await
}

/// List images for a specific record type and ID.
///
/// Returns presigned URLs for original images and thumbnails.
/// No authentication required.
///
/// # Response
///
/// `200 OK` — JSON object with `image_urls` and `image_thumbnail_urls` arrays
pub async fn get_images(
    state: web::Data<AppState>,
    path: web::Path<(String, i64)>,
) -> Result<HttpResponse, AppError> {
    let (record_type, record_id) = path.into_inner();

    let (image_urls, thumbnail_urls) = storage_service::get_image_urls(
        &state.s3,
        &state.db,
        &record_type,
        record_id,
    )
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "image_urls": image_urls,
        "image_thumbnail_urls": thumbnail_urls,
    })))
}

/// Delete an image blob from S3 storage.
///
/// Requires artist role or above. Removes the blob and its variants
/// from S3 and cleans up database records.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `403 Forbidden` — insufficient role
pub async fn delete_image(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let blob_id = path.into_inner();

    storage_service::delete_blob(&state.s3, &state.db, blob_id).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Blob #{} deleted", blob_id)
    })))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/storage/upload", web::post().to(upload))
        .route("/storage/images/{record_type}/{record_id}", web::get().to(get_images))
        .route("/storage/blob/{blob_id}", web::delete().to(delete_image));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{artist_token, user_token};
    use actix_web::test;

    #[tokio::test(flavor = "current_thread")]
    async fn upload_forbidden_for_non_artist() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let mut builder = actix_web::test::TestRequest::post()
            .uri("/storage/upload")
            .insert_header(("Authorization", format!("Bearer {}", user_token(2))));

        let body = multipart_body();
        builder = builder.set_payload(body);

        let req = builder.to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upload_missing_file_field() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let body = no_file_body();

        let req = actix_web::test::TestRequest::post()
            .uri("/storage/upload")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(1))))
            .insert_header(("Content-Type", "multipart/form-data; boundary=boundary456"))
            .set_payload(body)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    fn multipart_body() -> Vec<u8> {
        let boundary = "boundary123";
        let mut body = Vec::new();

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"record_type\"\r\n\r\nArtist\r\n",
            boundary
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"record_id\"\r\n\r\n1\r\n",
            boundary
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nimages\r\n",
            boundary
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n",
            boundary
        ).as_bytes());
        body.extend_from_slice(b"\x89PNG\r\n\x1a\nfake image data");

        body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

        body
    }

    fn no_file_body() -> Vec<u8> {
        let boundary = "boundary456";
        let mut body = Vec::new();

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"record_type\"\r\n\r\nArtist\r\n",
            boundary
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"record_id\"\r\n\r\n1\r\n",
            boundary
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\nimages\r\n",
            boundary
        ).as_bytes());

        body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

        body
    }

    #[tokio::test(flavor = "current_thread")]
    async fn variation_digest_is_deterministic() {
        let d1 = storage_service::compute_variation_digest(512, 512);
        let d2 = storage_service::compute_variation_digest(512, 512);
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn variation_digest_differs_for_different_sizes() {
        let d1 = storage_service::compute_variation_digest(512, 512);
        let d2 = storage_service::compute_variation_digest(100, 100);
        assert_ne!(d1, d2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upload_response_serializes() {
        let resp = UploadResponse {
            id: 1,
            key: "blobs/test.jpg".to_string(),
            filename: "test.jpg".to_string(),
            content_type: "image/jpeg".to_string(),
            byte_size: 1024,
            url: "https://example.com/original".to_string(),
            thumbnail_url: "https://example.com/thumb".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["id"], 1);
        assert_eq!(json["filename"], "test.jpg");
        assert_eq!(json["byte_size"], 1024);
    }

 }
