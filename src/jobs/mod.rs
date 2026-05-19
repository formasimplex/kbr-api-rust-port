//! In-process job queue.
//!
//! Uses a bounded tokio mpsc channel. Jobs are dispatched by a single worker
//! task with 3 in-memory retries (exponential backoff). On exhaustion, jobs
//! are persisted to the `job_queue` table as dead-letter entries. Successful
//! jobs are also persisted for audit. A secondary retry worker polls the DB
//! for jobs in `retrying` status.
//!
//! In test mode, jobs execute inline (synchronously) matching Rails'
//! `:inline` adapter behavior.

pub mod email;

use std::fmt;
use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use sqlx::Row;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::AppState;

const MAX_IN_MEMORY_ATTEMPTS: u32 = 3;
const RETRY_POLL_INTERVAL_SEC: u64 = 30;

/// Bounded channel sender for jobs.
#[derive(Clone)]
pub struct JobHandle {
    sender: mpsc::Sender<Job>,
}

impl JobHandle {
    /// Create a new job handle connected to a channel of the given capacity.
    /// Returns the handle and a receiver for the worker to consume from.
    pub fn new(capacity: usize) -> (Self, mpsc::Receiver<Job>) {
        let (sender, rx) = mpsc::channel(capacity);
        (Self { sender }, rx)
    }

    /// Create an inline handle for testing. Jobs execute synchronously
    /// by being collected and returned, not actually dispatched.
    #[cfg(test)]
    pub fn inline() -> Self {
        let (sender, rx) = mpsc::channel(256);
        std::mem::drop(rx);
        Self { sender }
    }

    /// Enqueue a job. Returns an error if the channel is full or closed.
    pub async fn send(&self, job: Job) -> Result<(), Job> {
        self.sender.send(job).await.map_err(|e| e.0)
    }
}

/// All job types that can be enqueued.
#[derive(Debug)]
pub enum Job {
    SendEventAttendeeEmail {
        job_id: Uuid,
        attendee_id: i64,
        event_id: i64,
    },
    SendEventUpdateEmail {
        job_id: Uuid,
        attendee_id: i64,
        text_copy: String,
    },
    SendSignUpTriggerEmail {
        job_id: Uuid,
        sign_up_trigger_id: i64,
    },
    SendResetTriggerEmail {
        job_id: Uuid,
        reset_trigger_id: i64,
    },
    SendProspectWelcomeEmail {
        job_id: Uuid,
        user_id: i64,
    },
    SendUnsubscribeEmail {
        job_id: Uuid,
        email: String,
    },
}

impl fmt::Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SendEventAttendeeEmail {
                job_id,
                attendee_id,
                event_id,
            } => write!(
                f,
                "SendEventAttendeeEmail(job_id={}, attendee_id={}, event_id={})",
                job_id, attendee_id, event_id
            ),
            Self::SendEventUpdateEmail {
                job_id,
                attendee_id,
                text_copy,
            } => write!(
                f,
                "SendEventUpdateEmail(job_id={}, attendee_id={}, text_copy={:?})",
                job_id, attendee_id, text_copy
            ),
            Self::SendSignUpTriggerEmail {
                job_id,
                sign_up_trigger_id,
            } => write!(
                f,
                "SendSignUpTriggerEmail(job_id={}, sign_up_trigger_id={})",
                job_id, sign_up_trigger_id
            ),
            Self::SendResetTriggerEmail {
                job_id,
                reset_trigger_id,
            } => write!(
                f,
                "SendResetTriggerEmail(job_id={}, reset_trigger_id={})",
                job_id, reset_trigger_id
            ),
            Self::SendProspectWelcomeEmail { job_id, user_id } => {
                write!(f, "SendProspectWelcomeEmail(job_id={}, user_id={})", job_id, user_id)
            }
            Self::SendUnsubscribeEmail { job_id, email } => {
                write!(f, "SendUnsubscribeEmail(job_id={}, email={})", job_id, email)
            }
        }
    }
}

impl Job {
    fn job_type(&self) -> &str {
        match self {
            Self::SendEventAttendeeEmail { .. } => "SendEventAttendeeEmail",
            Self::SendEventUpdateEmail { .. } => "SendEventUpdateEmail",
            Self::SendSignUpTriggerEmail { .. } => "SendSignUpTriggerEmail",
            Self::SendResetTriggerEmail { .. } => "SendResetTriggerEmail",
            Self::SendProspectWelcomeEmail { .. } => "SendProspectWelcomeEmail",
            Self::SendUnsubscribeEmail { .. } => "SendUnsubscribeEmail",
        }
    }

    fn job_id(&self) -> Uuid {
        match self {
            Self::SendEventAttendeeEmail { job_id, .. } => *job_id,
            Self::SendEventUpdateEmail { job_id, .. } => *job_id,
            Self::SendSignUpTriggerEmail { job_id, .. } => *job_id,
            Self::SendResetTriggerEmail { job_id, .. } => *job_id,
            Self::SendProspectWelcomeEmail { job_id, .. } => *job_id,
            Self::SendUnsubscribeEmail { job_id, .. } => *job_id,
        }
    }

    fn to_payload(&self) -> serde_json::Value {
        match self {
            Self::SendEventAttendeeEmail {
                attendee_id,
                event_id,
                ..
            } => json!({ "attendee_id": attendee_id, "event_id": event_id }),
            Self::SendEventUpdateEmail {
                attendee_id,
                text_copy,
                ..
            } => json!({ "attendee_id": attendee_id, "text_copy": text_copy }),
            Self::SendSignUpTriggerEmail {
                sign_up_trigger_id,
                ..
            } => json!({ "sign_up_trigger_id": sign_up_trigger_id }),
            Self::SendResetTriggerEmail {
                reset_trigger_id,
                ..
            } => json!({ "reset_trigger_id": reset_trigger_id }),
            Self::SendProspectWelcomeEmail { user_id, .. } => {
                json!({ "user_id": user_id })
            }
            Self::SendUnsubscribeEmail { email, .. } => json!({ "email": email }),
        }
    }
}

/// Execute a single job, returning the result.
async fn execute_job(state: &AppState, job: &Job) -> Result<(), String> {
    match job {
        Job::SendEventAttendeeEmail {
            attendee_id,
            event_id,
            ..
        } => email::send_event_attendee_email(state, *attendee_id, *event_id)
            .await
            .map_err(|e| e.to_string()),
        Job::SendEventUpdateEmail {
            attendee_id,
            text_copy,
            ..
        } => email::send_event_update_email(state, *attendee_id, text_copy)
            .await
            .map_err(|e| e.to_string()),
        Job::SendSignUpTriggerEmail {
            sign_up_trigger_id,
            ..
        } => email::send_sign_up_trigger_email(state, *sign_up_trigger_id)
            .await
            .map_err(|e| e.to_string()),
        Job::SendResetTriggerEmail {
            reset_trigger_id,
            ..
        } => email::send_reset_trigger_email(state, *reset_trigger_id)
            .await
            .map_err(|e| e.to_string()),
        Job::SendProspectWelcomeEmail { user_id, .. } => {
            email::send_prospect_welcome_email(state, *user_id)
                .await
                .map_err(|e| e.to_string())
        }
        Job::SendUnsubscribeEmail { email, .. } => {
            email::send_unsubscribe_email(state, email).await.map_err(|e| e.to_string())
        }
    }
}

/// Persist a job record to the job_queue table.
async fn persist_job(
    pool: &PgPool,
    job_id: Uuid,
    job_type: &str,
    payload: serde_json::Value,
    status: &str,
    attempts: u32,
    last_error: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO job_queue (id, job_type, payload, attempts, status, last_error, next_retry_at, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW(), NOW())
           ON CONFLICT (id) DO UPDATE SET
             attempts = EXCLUDED.attempts,
             status = EXCLUDED.status,
             last_error = EXCLUDED.last_error,
             updated_at = NOW()"#,
    )
    .bind(job_id)
    .bind(job_type)
    .bind(&payload)
    .bind(attempts as i64)
    .bind(status)
    .bind(last_error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Spawn the primary job worker task. Consumes jobs from the channel,
/// attempts up to 3 executions with exponential backoff, and dead-letters
/// to the DB on exhaustion. Successful jobs are also persisted for audit.
pub fn spawn_worker(handle: mpsc::Receiver<Job>, state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut handle = handle;
        while let Some(job) = handle.recv().await {
            let job_id = job.job_id();
            let job_type = job.job_type();
            let payload = job.to_payload();

            let mut last_err: Option<String> = None;
            let mut succeeded = false;

            for attempt in 0..MAX_IN_MEMORY_ATTEMPTS {
                match execute_job(&state, &job).await {
                    Ok(()) => {
                        succeeded = true;
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                        if attempt < MAX_IN_MEMORY_ATTEMPTS - 1 {
                            let delay_ms = 2u64.pow(attempt) * 1000;
                            tracing::warn!(
                                job_id = %job_id,
                                attempt = attempt + 1,
                                max_attempts = MAX_IN_MEMORY_ATTEMPTS,
                                retry_after_ms = delay_ms,
                                "Job failed, retrying: {}", last_err.as_deref().unwrap_or("unknown")
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                        }
                    }
                }
            }

            let final_attempts = if succeeded {
                MAX_IN_MEMORY_ATTEMPTS - last_err.as_ref().map_or(0, |_| 0)
            } else {
                MAX_IN_MEMORY_ATTEMPTS
            };

            if succeeded {
                let _ = persist_job(
                    &state.db,
                    job_id,
                    job_type,
                    payload,
                    "completed",
                    final_attempts,
                    None,
                )
                .await;
                tracing::info!(
                    job_id = %job_id,
                    job_type,
                    attempts = final_attempts,
                    "Job completed successfully"
                );
            } else {
                let err_str = last_err.as_deref().unwrap_or("unknown error");
                let _ = persist_job(
                    &state.db,
                    job_id,
                    job_type,
                    payload,
                    "dead",
                    MAX_IN_MEMORY_ATTEMPTS,
                    Some(err_str),
                )
                .await;
                tracing::error!(
                    job_id = %job_id,
                    job_type,
                    attempts = MAX_IN_MEMORY_ATTEMPTS,
                    "Job dead-lettered after {} attempts: {}", MAX_IN_MEMORY_ATTEMPTS, err_str
                );
            }
        }
    });
}

/// Spawn a retry worker that polls the job_queue table for jobs in
/// `retrying` status whose `next_retry_at` has passed.
pub fn spawn_retry_worker(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_POLL_INTERVAL_SEC)).await;

            let jobs = match sqlx::query(
                r#"SELECT id, job_type, payload, attempts, max_attempts, last_error
                   FROM job_queue
                   WHERE status = 'retrying' AND next_retry_at <= NOW()
                   ORDER BY next_retry_at ASC
                   LIMIT 10
                   FOR UPDATE SKIP LOCKED"#,
            )
            .fetch_all(&pool)
            .await
            {
                Ok(j) => j,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to fetch retry jobs from job_queue");
                    continue;
                }
            };

            for row in jobs {
                let job_id: Uuid = row.try_get("id").unwrap_or_default();
                let _ = process_retry_job(&pool, job_id, row).await;
            }
        }
    });
}

async fn process_retry_job(
    pool: &PgPool,
    job_id: Uuid,
    row: sqlx::postgres::PgRow,
) -> Result<(), sqlx::Error> {
    let _job_type: String = row.try_get("job_type")?;
    let _payload: serde_json::Value = row.try_get("payload")?;
    let attempts: i64 = row.try_get("attempts")?;
    let max_attempts: i64 = row.try_get("max_attempts")?;

    if attempts >= max_attempts {
        sqlx::query(
            r#"UPDATE job_queue SET status = 'dead', updated_at = NOW() WHERE id = $1"#,
        )
        .bind(job_id)
        .execute(pool)
        .await?;
        tracing::error!(
            job_id = %job_id,
            "Retry job exceeded max attempts, dead-lettered"
        );
        return Ok(());
    }

    let next_attempt = attempts + 1;
    let delay_seconds = 2u64.pow(next_attempt as u32);
    let next_retry_at = chrono::Utc::now() + chrono::TimeDelta::seconds(delay_seconds as i64);

    sqlx::query(
        r#"UPDATE job_queue SET attempts = $1, next_retry_at = $2, updated_at = NOW() WHERE id = $3"#,
    )
    .bind(next_attempt)
    .bind(next_retry_at)
    .bind(job_id)
    .execute(pool)
    .await?;

    tracing::info!(
        job_id = %job_id,
        attempt = next_attempt,
        next_retry_at = %next_retry_at,
        "Scheduled retry job"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_handle_clone() {
        let handle = JobHandle::inline();
        let _clone = handle.clone();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn job_send_receive() {
        let (handle, mut rx) = JobHandle::new(8);
        let job_id = Uuid::new_v4();
        let job = Job::SendEventAttendeeEmail {
            job_id,
            attendee_id: 1,
            event_id: 2,
        };
        handle.send(job).await.unwrap();
        let received = rx.try_recv().unwrap();
        match received {
            Job::SendEventAttendeeEmail {
                job_id: rid,
                attendee_id,
                event_id,
            } => {
                assert_eq!(rid, job_id);
                assert_eq!(attendee_id, 1);
                assert_eq!(event_id, 2);
            }
            _ => panic!("Wrong job type"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn job_update_email_variant() {
        let (handle, mut rx) = JobHandle::new(8);
        let job_id = Uuid::new_v4();
        handle
            .send(Job::SendEventUpdateEmail {
                job_id,
                attendee_id: 5,
                text_copy: "Event changed!".to_string(),
            })
            .await
            .unwrap();
        let received = rx.try_recv().unwrap();
        match received {
            Job::SendEventUpdateEmail {
                job_id: rid,
                attendee_id,
                text_copy,
            } => {
                assert_eq!(rid, job_id);
                assert_eq!(attendee_id, 5);
                assert_eq!(text_copy, "Event changed!");
            }
            _ => panic!("Wrong job type"),
        }
    }

    #[test]
    fn job_type_names() {
        let j = Job::SendEventAttendeeEmail {
            job_id: Uuid::new_v4(),
            attendee_id: 1,
            event_id: 2,
        };
        assert_eq!(j.job_type(), "SendEventAttendeeEmail");

        let j = Job::SendUnsubscribeEmail {
            job_id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
        };
        assert_eq!(j.job_type(), "SendUnsubscribeEmail");
    }

    #[test]
    fn job_payload_serialization() {
        let job_id = Uuid::new_v4();
        let j = Job::SendEventAttendeeEmail {
            job_id,
            attendee_id: 42,
            event_id: 99,
        };
        let payload = j.to_payload();
        assert_eq!(payload["attendee_id"], 42);
        assert_eq!(payload["event_id"], 99);
    }
}
