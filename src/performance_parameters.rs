use serde::Deserialize;

use crate::named::Named;

#[derive(Deserialize, Named)]
#[name = "performance"]
pub struct PerformanceParameters {
    /// The batch size for parallel iterations. Low batch sizes
    /// increase parallelism at the cost of additional overhead needed
    /// for spawning the futures, whereas large batch sizes prevent
    /// parallelization but reduce overhead
    /// A value of None will force effectively serial iterations.
    batch_size: Option<usize>,
}

impl Default for PerformanceParameters {
    fn default() -> Self {
        Self {
            batch_size: Some(1000),
        }
    }
}

impl PerformanceParameters {
    pub fn batch_size(&self) -> usize {
        // Using a really large value effectively turns off any kind of parallelism
        self.batch_size.unwrap_or(usize::MAX)
    }
}
