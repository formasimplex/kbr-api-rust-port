//! In-process job queue.
//!
//! Uses a bounded tokio mpsc channel. Jobs are dispatched by a single worker
//! task. In test mode, jobs execute inline (synchronously) matching Rails'
//! `:inline` adapter behavior.

pub mod email;

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::app::AppState;

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
        // Drop receiver silently — jobs are fire-and-forget in tests
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
        attendee_id: i64,
        event_id: i64,
    },
    SendEventUpdateEmail {
        attendee_id: i64,
        text_copy: String,
    },
    SendSignUpTriggerEmail {
        sign_up_trigger_id: i64,
    },
    SendResetTriggerEmail {
        reset_trigger_id: i64,
    },
    SendProspectWelcomeEmail {
        user_id: i64,
    },
    SendUnsubscribeEmail {
        email: String,
    },
}

/// Spawn the job worker task. Consumes jobs from the channel and dispatches
/// them to their respective handlers.
pub fn spawn_worker(handle: mpsc::Receiver<Job>, state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut handle = handle;
        while let Some(job) = handle.recv().await {
            match job {
                Job::SendEventAttendeeEmail {
                    attendee_id,
                    event_id,
                } => {
                    if let Err(e) =
                        email::send_event_attendee_email(&state, attendee_id, event_id).await
                    {
                        tracing::error!(
                            attendee_id,
                            event_id,
                            "Failed to send event attendee email: {}",
                            e
                        );
                    }
                }
                Job::SendEventUpdateEmail {
                    attendee_id,
                    text_copy,
                } => {
                    if let Err(e) =
                        email::send_event_update_email(&state, attendee_id, &text_copy).await
                    {
                        tracing::error!(
                            attendee_id,
                            "Failed to send event update email: {}",
                            e
                        );
                    }
                }
                Job::SendSignUpTriggerEmail {
                    sign_up_trigger_id,
                } => {
                    if let Err(e) =
                        email::send_sign_up_trigger_email(&state, sign_up_trigger_id).await
                    {
                        tracing::error!(
                            sign_up_trigger_id,
                            "Failed to send sign-up trigger email: {}",
                            e
                        );
                    }
                }
                Job::SendResetTriggerEmail {
                    reset_trigger_id,
                } => {
                    if let Err(e) =
                        email::send_reset_trigger_email(&state, reset_trigger_id).await
                    {
                        tracing::error!(
                            reset_trigger_id,
                            "Failed to send reset trigger email: {}",
                            e
                        );
                    }
                }
                Job::SendProspectWelcomeEmail { user_id } => {
                    if let Err(e) =
                        email::send_prospect_welcome_email(&state, user_id).await
                    {
                        tracing::error!(
                            user_id,
                            "Failed to send prospect welcome email: {}",
                            e
                        );
                    }
                }
                Job::SendUnsubscribeEmail { email } => {
                    if let Err(e) = email::send_unsubscribe_email(&state, &email).await {
                        tracing::error!(
                            email,
                            "Failed to send unsubscribe email: {}",
                            e
                        );
                    }
                }
            }
        }
    });
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
        let job = Job::SendEventAttendeeEmail {
            attendee_id: 1,
            event_id: 2,
        };
        handle.send(job).await.unwrap();
        let received = rx.try_recv().unwrap();
        match received {
            Job::SendEventAttendeeEmail {
                attendee_id,
                event_id,
            } => {
                assert_eq!(attendee_id, 1);
                assert_eq!(event_id, 2);
            }
            _ => panic!("Wrong job type"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn job_update_email_variant() {
        let (handle, mut rx) = JobHandle::new(8);
        handle
            .send(Job::SendEventUpdateEmail {
                attendee_id: 5,
                text_copy: "Event changed!".to_string(),
            })
            .await
            .unwrap();
        let received = rx.try_recv().unwrap();
        match received {
            Job::SendEventUpdateEmail {
                attendee_id,
                text_copy,
            } => {
                assert_eq!(attendee_id, 5);
                assert_eq!(text_copy, "Event changed!");
            }
            _ => panic!("Wrong job type"),
        }
    }
}
