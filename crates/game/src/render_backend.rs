use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendRequest {
    Cpu,
    Gpu,
    Auto,
}

impl BackendRequest {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "gpu" => Ok(Self::Gpu),
            "auto" => Ok(Self::Auto),
            _ => Err(format!("backend must be cpu, gpu, or auto (got {value})")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendDecision {
    Cpu { reason: Option<String> },
    Gpu,
}

impl BackendDecision {
    pub fn selected(&self) -> &'static str {
        match self {
            Self::Cpu { .. } => "cpu",
            Self::Gpu => "gpu",
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Cpu { reason } => reason.as_deref(),
            Self::Gpu => None,
        }
    }
}

impl fmt::Display for BackendDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.selected())?;
        if let Some(reason) = self.reason() {
            write!(f, " ({reason})")?;
        }
        Ok(())
    }
}

pub fn select_backend(
    requested: BackendRequest,
    mut gpu_probe: impl FnMut() -> Result<(), String>,
) -> Result<BackendDecision, String> {
    match requested {
        BackendRequest::Cpu => Ok(BackendDecision::Cpu { reason: None }),
        BackendRequest::Gpu => {
            gpu_probe()
                .map_err(|reason| format!("GPU backend requested but unavailable: {reason}"))?;
            Ok(BackendDecision::Gpu)
        }
        BackendRequest::Auto => match gpu_probe() {
            Ok(()) => Ok(BackendDecision::Gpu),
            Err(reason) => Ok(BackendDecision::Cpu {
                reason: Some(format!("GPU probe failed: {reason}")),
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_requests() {
        assert_eq!(BackendRequest::parse("cpu"), Ok(BackendRequest::Cpu));
        assert_eq!(BackendRequest::parse("gpu"), Ok(BackendRequest::Gpu));
        assert_eq!(BackendRequest::parse("auto"), Ok(BackendRequest::Auto));
        assert!(BackendRequest::parse("vulkan").is_err());
    }

    #[test]
    fn cpu_never_probes_gpu() {
        let mut probes = 0;
        let decision = select_backend(BackendRequest::Cpu, || {
            probes += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(decision, BackendDecision::Cpu { reason: None });
        assert_eq!(probes, 0);
    }

    #[test]
    fn auto_falls_back_with_reason() {
        let decision = select_backend(BackendRequest::Auto, || Err("no adapter".into())).unwrap();
        assert_eq!(decision.selected(), "cpu");
        assert_eq!(decision.reason(), Some("GPU probe failed: no adapter"));
    }

    #[test]
    fn gpu_does_not_fallback() {
        let error = select_backend(BackendRequest::Gpu, || Err("device lost".into())).unwrap_err();
        assert!(error.contains("GPU backend requested"));
    }
}
