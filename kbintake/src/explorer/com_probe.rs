use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComFeasibilityVerdict {
    NeedsSeparateDllSpike,
    UnsupportedPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComFeasibilityReport {
    pub platform: String,
    pub com_apartment_init_ok: bool,
    pub requires_inproc_dll: bool,
    pub current_explorer_model: &'static str,
    pub recommended_next_step: &'static str,
    pub verdict: ComFeasibilityVerdict,
}

impl ComFeasibilityReport {
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("Platform: {}", self.platform),
            format!(
                "COM apartment init: {}",
                if self.com_apartment_init_ok {
                    "ok"
                } else {
                    "not available"
                }
            ),
            format!(
                "IExplorerCommand in-proc DLL required: {}",
                yes_no(self.requires_inproc_dll)
            ),
            format!("Current Explorer model: {}", self.current_explorer_model),
            format!("Verdict: {}", verdict_label(&self.verdict)),
            format!("Next step: {}", self.recommended_next_step),
        ]
    }
}

#[cfg(windows)]
pub fn probe() -> Result<ComFeasibilityReport> {
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

    let init_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    if init_ok {
        unsafe { CoUninitialize() };
    }

    Ok(ComFeasibilityReport {
        platform: "windows".to_string(),
        com_apartment_init_ok: init_ok,
        requires_inproc_dll: true,
        current_explorer_model: "HKCU shell command registration to kbintakew.exe",
        recommended_next_step:
            "Build a separate Windows-only COM DLL spike; do not fold COM into the current exe registration path.",
        verdict: ComFeasibilityVerdict::NeedsSeparateDllSpike,
    })
}

#[cfg(not(windows))]
pub fn probe() -> Result<ComFeasibilityReport> {
    Ok(ComFeasibilityReport {
        platform: std::env::consts::OS.to_string(),
        com_apartment_init_ok: false,
        requires_inproc_dll: true,
        current_explorer_model: "unsupported outside Windows",
        recommended_next_step: "Run this probe on a Windows 11 machine before any COM spike work.",
        verdict: ComFeasibilityVerdict::UnsupportedPlatform,
    })
}

fn verdict_label(verdict: &ComFeasibilityVerdict) -> &'static str {
    match verdict {
        ComFeasibilityVerdict::NeedsSeparateDllSpike => "needs-separate-dll-spike",
        ComFeasibilityVerdict::UnsupportedPlatform => "unsupported-platform",
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use super::{probe, ComFeasibilityVerdict};

    #[test]
    fn probe_returns_actionable_report() {
        let report = probe().unwrap();

        assert!(report.requires_inproc_dll);
        assert!(!report.current_explorer_model.is_empty());
        assert!(!report.recommended_next_step.is_empty());
        assert!(matches!(
            report.verdict,
            ComFeasibilityVerdict::NeedsSeparateDllSpike
                | ComFeasibilityVerdict::UnsupportedPlatform
        ));
        assert!(report.lines().iter().any(|line| line.contains("Verdict:")));
    }
}
