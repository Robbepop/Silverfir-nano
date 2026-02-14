//! Test summary and result reporting

use crate::types::TestStats;
use log::{error, info};

pub fn print_summary(stats: &TestStats, duration: std::time::Duration) {
    info!("");
    info!("=== Test Summary ===");
    info!("Total:   {}", stats.total());
    let total = stats.total();
    info!(
        "Passed:  {} ({:.1}%)",
        stats.passed,
        (stats.passed as f64 / total as f64) * 100.0
    );
    if stats.failed != 0 {
        error!(
            "Failed:  {} ({:.1}%)",
            stats.failed,
            (stats.failed as f64 / total as f64) * 100.0
        );
    }
    if stats.skipped != 0 {
        info!(
            "Skipped: {} ({:.1}%)",
            stats.skipped,
            (stats.skipped as f64 / total as f64) * 100.0
        );
    }
    if stats.errored != 0 {
        error!(
            "Errored: {} ({:.1}%)",
            stats.errored,
            (stats.errored as f64 / total as f64) * 100.0
        );
    }
    info!("Duration: {:?}", duration);
    info!("");

    if stats.failed > 0 || stats.errored > 0 {
        error!("Some tests failed.");
    } else if total > 0 {
        info!("All tests passed!");
    } else {
        error!("No tests were executed.");
    }
}
