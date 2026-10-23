use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendKind {
    Cpu,
    Gpu,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendSnapshot<T> {
    pub value: T,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HotSwapState<T> {
    backend: BackendKind,
    snapshot: BackendSnapshot<T>,
    transitions: u64,
    last_transition: Option<Duration>,
    gpu_error: Option<String>,
}

impl<T> HotSwapState<T> {
    pub fn new(backend: BackendKind, snapshot: BackendSnapshot<T>) -> Self {
        Self {
            backend,
            snapshot,
            transitions: 0,
            last_transition: None,
            gpu_error: None,
        }
    }

    pub fn backend(&self) -> BackendKind {
        self.backend
    }
    pub fn snapshot(&self) -> &BackendSnapshot<T> {
        &self.snapshot
    }
    pub fn transitions(&self) -> u64 {
        self.transitions
    }
    pub fn last_transition(&self) -> Option<Duration> {
        self.last_transition
    }
    pub fn gpu_error(&self) -> Option<&str> {
        self.gpu_error.as_deref()
    }

    pub fn update_snapshot(&mut self, snapshot: BackendSnapshot<T>) {
        self.snapshot = snapshot;
    }

    pub fn try_switch<F>(&mut self, target: BackendKind, create: F) -> Result<bool, String>
    where
        F: FnOnce(&BackendSnapshot<T>) -> Result<(), String>,
    {
        if target == self.backend {
            return Ok(false);
        }
        let started = Instant::now();
        if target == BackendKind::Gpu {
            if let Err(error) = create(&self.snapshot) {
                self.gpu_error = Some(error.clone());
                return Err(error);
            }
            self.gpu_error = None;
        } else {
            create(&self.snapshot)?;
        }
        self.backend = target;
        self.transitions += 1;
        self.last_transition = Some(started.elapsed());
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> HotSwapState<Vec<u8>> {
        HotSwapState::new(
            BackendKind::Cpu,
            BackendSnapshot {
                value: vec![1, 2, 3],
                width: 640,
                height: 360,
            },
        )
    }

    #[test]
    fn repeated_switches_preserve_snapshot() {
        let mut state = state();
        let expected = state.snapshot().clone();
        assert!(state.try_switch(BackendKind::Gpu, |_| Ok(())).unwrap());
        assert!(state.try_switch(BackendKind::Cpu, |_| Ok(())).unwrap());
        assert_eq!(state.snapshot(), &expected);
        assert_eq!(state.transitions(), 2);
    }

    #[test]
    fn gpu_failure_keeps_cpu_and_reason() {
        let mut state = state();
        assert_eq!(
            state.try_switch(BackendKind::Gpu, |_| Err("no adapter".into())),
            Err("no adapter".into())
        );
        assert_eq!(state.backend(), BackendKind::Cpu);
        assert_eq!(state.gpu_error(), Some("no adapter"));
    }

    #[test]
    fn resize_snapshot_is_retained() {
        let mut state = state();
        state.update_snapshot(BackendSnapshot {
            value: vec![9],
            width: 1280,
            height: 720,
        });
        assert!(state
            .try_switch(BackendKind::Gpu, |snapshot| {
                assert_eq!((snapshot.width, snapshot.height), (1280, 720));
                Ok(())
            })
            .unwrap());
    }
}
