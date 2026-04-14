#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexProgressPhase {
    Scanning,
    Tokenizing,
    Writing,
    Finalizing,
    RefreshingOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexProgress {
    pub phase: IndexProgressPhase,
    pub message: String,
    pub processed: usize,
    pub total: Option<usize>,
}

impl IndexProgress {
    pub fn new(
        phase: IndexProgressPhase,
        message: impl Into<String>,
        processed: usize,
        total: Option<usize>,
    ) -> Self {
        Self {
            phase,
            message: message.into(),
            processed,
            total,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.total.is_some_and(|total| self.processed >= total)
    }
}
