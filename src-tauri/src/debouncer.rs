use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{task::JoinHandle, time::sleep};

const DEFAULT_TIMEOUT: Duration = Duration::from_millis(50);
const DEFAULT_CLICK_THRESHOLD: Duration = Duration::from_millis(100);

pub struct ClickHandler {
    join_handle: Option<JoinHandle<()>>,
    last_click_time: Option<Instant>,
    timeout: Duration,
    click_threshold: Duration,
}

impl ClickHandler {
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            click_threshold: DEFAULT_CLICK_THRESHOLD,
            join_handle: None,
            last_click_time: None,
        }
    }

    pub fn click(&mut self, callback: Box<dyn FnOnce() + Send>) {
        self.cancel();

        // Only check for double-click if there was a previous click
        if let Some(last_time) = self.last_click_time {
            let time_since_last_click = Instant::now().duration_since(last_time);

            // Double click
            if time_since_last_click < self.click_threshold {
                return;
            }
        }

        let timeout = self.timeout;
        let jh = tokio::spawn(async move {
            sleep(timeout).await;
            callback();
        });

        self.join_handle = Some(jh);
        self.last_click_time = Some(Instant::now());
    }

    pub fn cancel(&mut self) {
        if let Some(jh) = self.join_handle.take() {
            jh.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{sync::oneshot, time::sleep};

    #[tokio::test]
    async fn test_click_handler_triggers_callback_after_timeout() {
        let counter = Arc::new(Mutex::new(0));
        let mut click_handler = ClickHandler::new();

        let counter_ = Arc::clone(&counter);
        click_handler.click(Box::new(move || {
            *counter_.lock().unwrap() = 42;
        }));

        sleep(Duration::from_millis(20)).await;

        assert_eq!(
            *counter.lock().unwrap(),
            0,
            "Callback should not have been triggered yet"
        );

        sleep(Duration::from_millis(33)).await;

        assert_eq!(*counter.lock().unwrap(), 42, "Callback should have been triggered");
    }

    #[tokio::test]
    async fn test_click_handler_cancel() {
        let counter = Arc::new(Mutex::new(0));
        let mut click_handler = ClickHandler::new();

        let counter_ = Arc::clone(&counter);
        click_handler.click(Box::new(move || {
            *counter_.lock().unwrap() += 1;
        }));

        click_handler.cancel();

        // Sleep longer than the timeout
        sleep(Duration::from_millis(200)).await;

        assert_eq!(*counter.lock().unwrap(), 0, "Callback should have been cancelled");
    }

    #[tokio::test]
    async fn test_double_click_detection() {
        let counter = Arc::new(Mutex::new(0));
        let mut click_handler = ClickHandler::new();

        // First click
        let counter_ = Arc::clone(&counter);
        click_handler.click(Box::new(move || {
            *counter_.lock().unwrap() += 1;
        }));

        // Sleep for a short time
        sleep(Duration::from_millis(50)).await;

        // Second click within double-click threshold (100ms)
        // This should cancel the first click and not schedule a new one
        let counter_ = Arc::clone(&counter);
        click_handler.click(Box::new(move || {
            *counter_.lock().unwrap() += 1;
        }));

        // Sleep longer than the timeout
        sleep(Duration::from_millis(400)).await;

        // Counter should still be 0 because the second click was detected as a double click
        // and no callback was scheduled
        assert_eq!(
            *counter.lock().unwrap(),
            0,
            "No callbacks should have been triggered due to double-click detection"
        );
    }

    #[tokio::test]
    async fn test_click_handler_double_click_spaced() {
        let counter = Arc::new(Mutex::new(0));
        let mut click_handler = ClickHandler::new();

        // First click
        let counter_ = Arc::clone(&counter);
        click_handler.click(Box::new(move || {
            *counter_.lock().unwrap() += 1;
        }));

        sleep(Duration::from_millis(120)).await;

        assert_eq!(*counter.lock().unwrap(), 1);

        let counter_ = Arc::clone(&counter);
        click_handler.click(Box::new(move || {
            *counter_.lock().unwrap() += 1;
        }));

        sleep(Duration::from_millis(120)).await;

        assert_eq!(*counter.lock().unwrap(), 2);
    }
}
