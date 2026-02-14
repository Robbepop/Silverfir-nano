//! Test statistics and result types

#[derive(Default)]
pub struct TestStats {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errored: usize,
}

impl TestStats {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped + self.errored
    }
}
