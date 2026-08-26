use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

const TICK_INTERVAL: Duration = Duration::from_millis(50);
const TICK_RATE: u8 = 13;
const SHOW_DELAY: Duration = Duration::from_millis(70);
const TICK_STRINGS: [&str; 11] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "];

/// A transient stderr spinner that is inert when interactive output is disabled.
pub struct Spinner {
    progress: Option<ProgressBar>,
    delayed_start: Option<DelayedStart>,
}

struct DelayedStart {
    cancelled: Arc<AtomicBool>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Spinner {
    /// Creates a spinner when `enabled` is true; otherwise all operations are no-ops.
    pub fn new(enabled: bool, colored: bool, message: impl Into<String>) -> Self {
        let progress = enabled.then(|| {
            let template = if colored {
                "{spinner:.dim} {msg}"
            } else {
                "{spinner} {msg}"
            };
            let style = ProgressStyle::with_template(template)
                .expect("spinner template is valid")
                .tick_strings(&TICK_STRINGS);
            let progress =
                ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr_with_hz(TICK_RATE));
            progress.set_style(style);
            progress.set_draw_target(ProgressDrawTarget::hidden());
            progress.set_message(message.into());
            progress
        });

        let delayed_start = progress.as_ref().map(|progress| {
            let cancelled = Arc::new(AtomicBool::new(false));
            let thread_cancelled = Arc::clone(&cancelled);
            let progress = progress.clone();
            let handle = thread::spawn(move || {
                thread::park_timeout(SHOW_DELAY);
                if !thread_cancelled.load(Ordering::Acquire) {
                    progress.set_draw_target(ProgressDrawTarget::stderr_with_hz(TICK_RATE));
                    progress.enable_steady_tick(TICK_INTERVAL);
                    progress.force_draw();
                }
            });
            DelayedStart {
                cancelled,
                handle: Mutex::new(Some(handle)),
            }
        });

        Self {
            progress,
            delayed_start,
        }
    }

    /// Updates the status text shown next to the spinner.
    pub fn set_message(&self, message: impl Into<String>) {
        if let Some(progress) = &self.progress {
            progress.set_message(message.into());
        }
    }

    /// Stops the spinner and removes it from the terminal.
    pub fn finish_and_clear(&self) {
        if let Some(delayed_start) = &self.delayed_start {
            delayed_start.cancelled.store(true, Ordering::Release);
            if let Ok(mut handle) = delayed_start.handle.lock()
                && let Some(handle) = handle.take()
            {
                handle.thread().unpark();
                drop(handle.join());
            }
        }
        if let Some(progress) = &self.progress {
            progress.finish_and_clear();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.finish_and_clear();
    }
}
