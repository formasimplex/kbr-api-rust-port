use std::fs::OpenOptions;
use std::io::Write;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use actix_web::{web, App, HttpServer, HttpResponse, HttpRequest};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const MAX_ERROR_LINES: usize = 20;

async fn handle_webhook(
    req: HttpRequest,
    payload: web::Bytes,
    build_lock: web::Data<Mutex<bool>>,
) -> HttpResponse {
    let event_type = req
        .headers()
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if event_type != "push" {
        return HttpResponse::Ok().finish();
    }

    let signature_header = req
        .headers()
        .get("X-Hub-Signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !verify_signature(payload.as_ref(), signature_header) {
        tracing::warn!("Webhook signature verification failed");
        return HttpResponse::Unauthorized().finish();
    }

    let parsed = match serde_json::from_slice::<Value>(&payload) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to parse webhook payload: {}", e);
            return HttpResponse::Ok().finish();
        }
    };

    let is_main = parsed
        .get("ref")
        .and_then(|v| v.as_str())
        .map(|r| r == "refs/heads/main")
        .unwrap_or(false);

    if !is_main {
        return HttpResponse::Ok().finish();
    }

    tracing::info!("Main branch push detected, triggering deploy");

    let lock = build_lock.into_inner();
    tokio::task::spawn(async move {
        deploy(lock).await;
    });

    HttpResponse::Ok().finish()
}

fn verify_signature(payload: &[u8], expected: &str) -> bool {
    let secret = match std::env::var("GITHUB_WEBHOOK_SECRET") {
        Ok(s) => s,
        Err(_) => {
            tracing::error!("GITHUB_WEBHOOK_SECRET not set");
            return false;
        }
    };

    let expected = match expected.strip_prefix("sha256=") {
        Some(stripped) => stripped,
        None => return false,
    };

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(payload);

    let computed = hex::encode(mac.finalize().into_bytes());
    constant_time_eq::constant_time_eq(computed.as_bytes(), expected.as_bytes())
}

async fn deploy(lock: Arc<Mutex<bool>>) {
    let log_start = Instant::now();

    {
        let mut locked = lock.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned, recovering: {}", e);
            e.into_inner()
        });
        if *locked {
            tracing::info!("Build already in progress, skipping");
            return;
        }
        *locked = true;
    }

    let result = do_deploy().await;

    {
        let mut locked = lock.lock().unwrap_or_else(|e| {
            tracing::error!("Mutex poisoned, recovering: {}", e);
            e.into_inner()
        });
        *locked = false;
    }

    let elapsed = log_start.elapsed();
    write_deploy_log(&result, &elapsed);
}

async fn do_deploy() -> DeployResult {
    let pull_start = Instant::now();
    let pull_output = run_command("git", &["pull", "origin", "main"]).await;
    let pull_elapsed = pull_start.elapsed();

    if !pull_output.success {
        tracing::error!("Git pull failed after {:?}", pull_elapsed);
        return DeployResult {
            success: false,
            pull_elapsed,
            build_elapsed: std::time::Duration::ZERO,
            restart_elapsed: std::time::Duration::ZERO,
            error_lines: {
                let all = format!("{}{}", pull_output.stdout, pull_output.stderr);
                all.lines()
                    .filter(|l| !l.is_empty())
                    .rev()
                    .take(MAX_ERROR_LINES)
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .into_iter()
                    .rev()
                    .collect()
            },
        };
    }

    tracing::info!("Git pull succeeded in {:?}", pull_elapsed);

    let build_start = Instant::now();

    let build_output = run_command("cargo", &["build", "--release"]).await;
    let build_elapsed = build_start.elapsed();

    if !build_output.success {
        tracing::error!("Build failed after {:?}", build_elapsed);
        return DeployResult {
            success: false,
            pull_elapsed,
            build_elapsed,
            restart_elapsed: std::time::Duration::ZERO,
            error_lines: {
                let all = format!("{}{}", build_output.stdout, build_output.stderr);
                all.lines()
                    .filter(|l| !l.is_empty())
                    .rev()
                    .take(MAX_ERROR_LINES)
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
                    .into_iter()
                    .rev()
                    .collect()
            },
        };
    }

    tracing::info!("Build succeeded in {:?}", build_elapsed);

    let restart_start = Instant::now();
    let restart_output = run_command("sudo", &["/usr/bin/systemctl", "restart", "kbr-api-rust"]).await;
    let restart_elapsed = restart_start.elapsed();

    if !restart_output.success {
        tracing::error!("Service restart failed after {:?}", restart_elapsed);
        return DeployResult {
            success: false,
            pull_elapsed,
            build_elapsed,
            restart_elapsed,
            error_lines: restart_output.stderr.lines()
                .filter(|l| !l.is_empty())
                .take(MAX_ERROR_LINES)
                .map(|s| s.to_string())
                .collect(),
        };
    }

    tracing::info!("Service restarted in {:?}", restart_elapsed);

    DeployResult {
        success: true,
        pull_elapsed,
        build_elapsed,
        restart_elapsed,
        error_lines: Vec::new(),
    }
}

async fn run_command(cmd: &str, args: &[&str]) -> CommandOutput {
    let child = match tokio::process::Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to spawn {}: {}", cmd, e);
            return CommandOutput {
                success: false,
                stdout: format!("Failed to spawn {}: {}", cmd, e),
                stderr: String::new(),
            };
        }
    };

    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("Failed to wait for {}: {}", cmd, e);
            return CommandOutput {
                success: false,
                stdout: format!("Failed to wait for {}: {}", cmd, e),
                stderr: String::new(),
            };
        }
    };

    CommandOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

fn write_deploy_log(result: &DeployResult, total_elapsed: &std::time::Duration) {
    let now = chrono::Utc::now();
    let status = if result.success { "SUCCESS" } else { "FAILED" };

    let mut lines = vec![
        format!("\n=== {} {} ===", now.format("%Y-%m-%d %H:%M:%S UTC"), status),
        format!("Total elapsed: {:?}", total_elapsed),
        format!("Pull: {:?}", result.pull_elapsed),
        format!("Build: {:?}", result.build_elapsed),
        format!("Restart: {:?}", result.restart_elapsed),
    ];

    if !result.error_lines.is_empty() {
        lines.push("Last error lines:".to_string());
        for line in &result.error_lines {
            lines.push(format!("  {}", line));
        }
    }

    lines.push("---".to_string());

    let log_content = lines.join("\n") + "\n";

    if let Ok(log_path) = std::env::var("DEPLOY_LOG_PATH") {
        append_to_file(&log_path, &log_content);
    } else {
        append_to_file("deploy.log", &log_content);
    }
}

fn append_to_file(path: &str, content: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        if let Err(e) = file.write_all(content.as_bytes()) {
            tracing::error!("Failed to write to {}: {}", path, e);
        }
    } else {
        tracing::error!("Failed to open {} for writing", path);
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let env_file = std::env::var("ENV_FILE_PATH").unwrap_or_else(|_| ".env".to_string());
    if dotenvy::from_path(&env_file).is_err() {
        eprintln!("Warning: Failed to load env file: {}", env_file);
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("deploy_webhook=info".parse().unwrap()),
        )
        .init();

    let secret = std::env::var("GITHUB_WEBHOOK_SECRET");
    if secret.is_err() {
        tracing::error!("GITHUB_WEBHOOK_SECRET must be set in .env");
        std::process::exit(1);
    }

       let build_lock = web::Data::new(Mutex::new(false));

    let addr: std::net::SocketAddr = std::env::var("WEBHOOK_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9999".to_string())
        .parse()
        .expect("Invalid WEBHOOK_BIND_ADDR");

    tracing::info!("Deploy webhook listener starting on {}", addr);

    let server = HttpServer::new({
        let build_lock = build_lock.clone();
        move || {
            App::new()
                .wrap(actix_web::middleware::Logger::default())
                .app_data(web::PayloadConfig::default().limit(1_000_000))
                .route("/mr-franz-marc", web::post().to(handle_webhook))
                .app_data(build_lock.clone())
        }
    })
    .bind(addr)?
    .run();

    server.await
}

struct DeployResult {
    success: bool,
    pull_elapsed: std::time::Duration,
    build_elapsed: std::time::Duration,
    restart_elapsed: std::time::Duration,
    error_lines: Vec<String>,
}

struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::StatusCode;
    use actix_web::{test, web, App};

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn make_payload(ref_branch: &str) -> String {
        format!(
            r#"{{"ref":"refs/heads/{}","before":"abc","after":"def","repository":{{}},"pusher":{{}},"sender":{{}}}}"#,
            ref_branch
        )
    }

    fn sign_payload(payload: &[u8], secret: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

 #[actix_web::test]
    async fn test_handler_main_branch_triggers_deploy() {
        let _env = ENV_MUTEX.lock().unwrap();
        let secret = "test-secret-123";
        unsafe {
            std::env::set_var("GITHUB_WEBHOOK_SECRET", secret);
        }

        let payload = make_payload("main");
        let signature = sign_payload(payload.as_bytes(), secret);
   let build_lock = web::Data::new(Mutex::new(false));

        let req = test::TestRequest::post()
            .uri("/mr-franz-marc")
            .insert_header(("X-GitHub-Event", "push"))
            .insert_header(("X-Hub-Signature-256", signature.as_str()))
            .set_payload(payload.clone())
            .to_request();

        let app = test::init_service(
            App::new()
                .route("/mr-franz-marc", web::post().to(handle_webhook))
                .app_data(build_lock.clone()),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        unsafe {
            std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        }
    }

    #[actix_web::test]
    async fn test_handler_wrong_branch_ignores() {
        let _env = ENV_MUTEX.lock().unwrap();
        let secret = "test-secret-123";
        unsafe {
            std::env::set_var("GITHUB_WEBHOOK_SECRET", secret);
        }

        let payload = make_payload("develop");
        let signature = sign_payload(payload.as_bytes(), secret);

        let req = test::TestRequest::post()
            .uri("/mr-franz-marc")
            .insert_header(("X-GitHub-Event", "push"))
            .insert_header(("X-Hub-Signature-256", signature.as_str()))
            .set_payload(payload.clone())
            .to_request();

        let app = test::init_service(
            App::new()
                .route("/mr-franz-marc", web::post().to(handle_webhook))
                .app_data(web::Data::new(Mutex::new(false))),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        unsafe {
            std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        }
    }

    #[actix_web::test]
    async fn test_handler_bad_signature_returns_401() {
        let _env = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("GITHUB_WEBHOOK_SECRET", "real-secret");
        }

        let payload = make_payload("main");
        let bad_signature = sign_payload(payload.as_bytes(), "wrong-secret");

        let req = test::TestRequest::post()
            .uri("/mr-franz-marc")
            .insert_header(("X-GitHub-Event", "push"))
            .insert_header(("X-Hub-Signature-256", bad_signature.as_str()))
            .set_payload(payload.clone())
            .to_request();

        let app = test::init_service(
            App::new()
                .route("/mr-franz-marc", web::post().to(handle_webhook))
                .app_data(web::Data::new(Mutex::new(false))),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        unsafe {
            std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        }
    }

    #[actix_web::test]
    async fn test_handler_non_push_event_ignores() {
        let req = test::TestRequest::post()
            .uri("/mr-franz-marc")
            .insert_header(("X-GitHub-Event", "ping"))
            .set_payload("{}".to_string())
            .to_request();

        let app = test::init_service(
            App::new()
                .route("/mr-franz-marc", web::post().to(handle_webhook))
                .app_data(web::Data::new(Mutex::new(false))),
        )
        .await;

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_verify_signature_tampered() {
        unsafe {
            std::env::set_var("GITHUB_WEBHOOK_SECRET", "my-secret");
        }

        let payload = b"test payload";
        let mut mac = HmacSha256::new_from_slice(b"my-secret").unwrap();
        mac.update(payload);
        let mut sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        // Tamper with the signature
        sig.pop();
        sig.push('0');

        assert!(!verify_signature(b"tampered payload", &sig));

        unsafe {
            std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_verify_signature_missing_prefix() {
        unsafe {
            std::env::set_var("GITHUB_WEBHOOK_SECRET", "my-secret");
        }

        assert!(!verify_signature(b"test", "noshaprefix"));

        unsafe {
            std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_verify_signature_no_secret() {
        unsafe {
            std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        }

        assert!(!verify_signature(b"test", "sha256=abc"));
    }
}
