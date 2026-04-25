use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimingStep {
    pub name: &'static str,
    pub duration_ms: u128,
}

#[derive(Debug, Default, Clone)]
pub struct TimingCollector {
    steps: Vec<TimingStep>,
}

impl TimingCollector {
    pub fn push_duration(&mut self, name: &'static str, duration_ms: u128) {
        self.steps.push(TimingStep { name, duration_ms });
    }

    pub fn record_elapsed(&mut self, name: &'static str, started_at: Instant) -> u128 {
        let duration_ms = elapsed_ms(started_at);
        self.push_duration(name, duration_ms);
        duration_ms
    }

    pub fn steps(&self) -> &[TimingStep] {
        &self.steps
    }
}

pub fn elapsed_ms(started_at: Instant) -> u128 {
    started_at.elapsed().as_millis()
}

pub fn elapsed_ms_u64(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis() as u64
}
