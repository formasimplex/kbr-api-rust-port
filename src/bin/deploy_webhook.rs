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

    let expected = if expected.starts_with("sha256=") {
        &expected[7..]
    } else {
        return false;
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
        let mut locked = lock.lock().unwrap();
        if *locked {
            tracing::info!("Build already in progress, skipping");
            return;
        }
        *locked = true;
    }

    let result = do_deploy().await;

    {
        let mut locked = lock.lock().unwrap();
        *locked = false;
    }

    let elapsed = log_start.elapsed();
    write_deploy_log(&result, &elapsed);
}

async fn do_deploy() -> DeployResult {
    let build_start = Instant::now();

    let build_output = run_command("cargo", &["build", "--release"]).await;
    let build_elapsed = build_start.elapsed();

    if !build_output.success {
        tracing::error!("Build failed after {:?}", build_elapsed);
        return DeployResult {
            success: false,
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("deploy_webhook=info".parse().unwrap()),
        )
        .init();

    dotenvy::dotenv().ok();

    let secret = std::env::var("GITHUB_WEBHOOK_SECRET");
    if secret.is_err() {
        tracing::error!("GITHUB_WEBHOOK_SECRET must be set in .env");
        std::process::exit(1);
    }

    let build_lock = Arc::new(Mutex::new(false));

    let addr: std::net::SocketAddr = std::env::var("WEBHOOK_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:9999".to_string())
        .parse()
        .expect("Invalid WEBHOOK_BIND_ADDR");

    tracing::info!("Deploy webhook listener starting on {}", addr);

    let server = HttpServer::new({
        let build_lock = web::Data::new(build_lock);
        move || {
            App::new()
                .wrap(actix_web::middleware::Logger::default())
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
    build_elapsed: std::time::Duration,
    restart_elapsed: std::time::Duration,
    error_lines: Vec<String>,
}

struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}
