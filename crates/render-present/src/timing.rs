use std::num::NonZeroU32;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct FramePacer {
    period: Duration,
    deadline: Instant,
}

impl FramePacer {
    pub fn new(fps: NonZeroU32, now: Instant) -> Self {
        Self {
            period: Duration::from_nanos((1_000_000_000 / u64::from(fps.get())).max(1)),
            deadline: now,
        }
    }

    pub fn is_due(&self, now: Instant) -> bool {
        now >= self.deadline
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn frame_started(&mut self, now: Instant) {
        self.deadline = now + self.period;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameMetrics {
    pub frames: u64,
    pub render_total: Duration,
    pub presentation_total: Duration,
    pub last_render: Duration,
    pub last_presentation: Duration,
}

impl FrameMetrics {
    pub fn record(&mut self, render: Duration, presentation: Duration) {
        self.frames += 1;
        self.render_total += render;
        self.presentation_total += presentation;
        self.last_render = render;
        self.last_presentation = presentation;
    }

    pub fn average_render(&self) -> Duration {
        self.render_total.div_f64(self.frames.max(1) as f64)
    }

    pub fn average_presentation(&self) -> Duration {
        self.presentation_total.div_f64(self.frames.max(1) as f64)
    }
}
