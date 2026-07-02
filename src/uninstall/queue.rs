use crate::models::ApplicationEntry;
use crate::uninstall::engine::uninstall_application;
use anyhow::Result;

pub struct UninstallQueue {
    pending: Vec<ApplicationEntry>,
    current_index: usize,
    succeeded: usize,
    failed: usize,
    last_error: Option<String>,
}

impl UninstallQueue {
    pub fn new(apps: Vec<ApplicationEntry>) -> Self {
        Self {
            pending: apps,
            current_index: 0,
            succeeded: 0,
            failed: 0,
            last_error: None,
        }
    }

    pub fn total(&self) -> usize {
        self.pending.len()
    }

    pub fn done(&self) -> usize {
        self.current_index
    }

    pub fn succeeded(&self) -> usize {
        self.succeeded
    }

    pub fn failed(&self) -> usize {
        self.failed
    }

    pub fn is_finished(&self) -> bool {
        self.current_index >= self.pending.len()
    }

    pub fn current(&self) -> Option<&ApplicationEntry> {
        self.pending.get(self.current_index)
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn advance(&mut self) -> Result<()> {
        if self.is_finished() {
            return Ok(());
        }

        let app = &self.pending[self.current_index];
        match uninstall_application(app) {
            Ok(()) => {
                self.succeeded += 1;
                self.last_error = None;
            }
            Err(e) => {
                self.failed += 1;
                self.last_error = Some(e.to_string());
            }
        }

        self.current_index += 1;
        Ok(())
    }
}
