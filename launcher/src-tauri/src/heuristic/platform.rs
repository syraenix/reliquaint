//! Classify a directory as DOS, Amiga, or Unknown based on top-level
//! files and a small number of inner markers.

use crate::catalog::Platform;
use std::path::Path;

/// Outcome of running the platform classifier on a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformDetection {
    pub platform: Option<Platform>,
    pub confidence: Confidence,
    /// Human-readable bullet list of the indicators that led to the
    /// classification. The wizard surfaces this when the classifier is
    /// uncertain so the user understands *why* and can disambiguate.
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub enum Confidence {
    None,
    Low,
    Medium,
    High,
}

/// Inspect `dir` and produce a [`PlatformDetection`]. Returns
/// `platform: None` for empty or ambiguous directories.
pub fn detect(dir: &Path) -> PlatformDetection {
    let entries = super::list_dir(dir);
    if entries.is_empty() {
        return PlatformDetection {
            platform: None,
            confidence: Confidence::None,
            evidence: vec!["directory is empty or unreadable".to_string()],
        };
    }

    let mut dos_score: u32 = 0;
    let mut amiga_score: u32 = 0;
    let mut evidence: Vec<String> = Vec::new();

    // Top-level extension and named-file checks.
    let mut bat = 0u32;
    let mut exe = 0u32;
    let mut com = 0u32;
    let mut adf = 0u32;
    let mut rom = 0u32;
    let mut info = 0u32;
    let mut slave = 0u32;
    let mut hdf = 0u32;
    let mut sci_resource_files = false;
    let mut other_sci = false;

    for (name, is_file) in &entries {
        if !is_file {
            continue;
        }
        let upper = name.to_ascii_uppercase();
        if let Some(ext) = upper.rsplit_once('.').map(|(_, e)| e) {
            match ext {
                "BAT" => bat += 1,
                "EXE" => exe += 1,
                "COM" => com += 1,
                "ADF" => adf += 1,
                "ROM" => rom += 1,
                "HDF" => hdf += 1,
                "SLAVE" => slave += 1,
                _ => {}
            }
            if ext.starts_with("SCI") {
                other_sci = true;
            }
        }
        // Named markers.
        if upper == "RESOURCE.000" {
            sci_resource_files = true;
        }
        if upper.ends_with(".INFO") {
            info += 1;
        }
    }

    // Inner `S/Startup-Sequence` is a strong Amiga marker.
    let startup_sequence = dir.join("S").join("Startup-Sequence").is_file()
        || dir.join("s").join("startup-sequence").is_file();

    if bat > 0 {
        dos_score += bat * 3;
        evidence.push(format!("{bat} .BAT file{}", plural(bat)));
    }
    if exe > 0 {
        dos_score += exe * 2;
        evidence.push(format!("{exe} .EXE file{}", plural(exe)));
    }
    if com > 0 {
        dos_score += com * 2;
        evidence.push(format!("{com} .COM file{}", plural(com)));
    }
    if sci_resource_files {
        dos_score += 5;
        evidence.push("RESOURCE.000 (Sierra SCI engine marker)".to_string());
    }
    if other_sci {
        dos_score += 2;
        evidence.push("`.SCI*` files".to_string());
    }

    if adf > 0 {
        amiga_score += adf * 4;
        evidence.push(format!("{adf} .ADF disk image{}", plural(adf)));
    }
    if rom > 0 {
        amiga_score += rom * 2;
        evidence.push(format!("{rom} .ROM file{}", plural(rom)));
    }
    if info > 0 {
        amiga_score += info * 2;
        evidence.push(format!("{info} .info icon{}", plural(info)));
    }
    if startup_sequence {
        amiga_score += 5;
        evidence.push("S/Startup-Sequence".to_string());
    }
    if slave > 0 {
        amiga_score += slave * 3;
        evidence.push(format!(
            "{slave} .slave file{} (WHDLoad)",
            plural(slave)
        ));
    }
    if hdf > 0 {
        amiga_score += hdf * 3;
        evidence.push(format!("{hdf} .hdf file{}", plural(hdf)));
    }

    let (platform, confidence) = classify(dos_score, amiga_score);
    PlatformDetection {
        platform,
        confidence,
        evidence,
    }
}

fn classify(dos: u32, amiga: u32) -> (Option<Platform>, Confidence) {
    if dos == 0 && amiga == 0 {
        return (None, Confidence::None);
    }
    if dos > 0 && amiga > 0 {
        // Both kinds of indicators present — ambiguous.
        return (None, Confidence::Low);
    }
    let (winner, score) = if dos >= amiga {
        (Platform::Dos, dos)
    } else {
        (Platform::Amiga, amiga)
    };
    let confidence = match score {
        0 => Confidence::None,
        1..=3 => Confidence::Low,
        4..=7 => Confidence::Medium,
        _ => Confidence::High,
    };
    (Some(winner), confidence)
}

fn plural(n: u32) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, "").unwrap();
    }

    #[test]
    fn empty_dir_is_unknown() {
        let tmp = tempdir().unwrap();
        let det = detect(tmp.path());
        assert_eq!(det.platform, None);
        assert_eq!(det.confidence, Confidence::None);
    }

    #[test]
    fn sierra_install_classifies_as_dos_high_confidence() {
        // A QFG1-EGA install has a SIERRA.BAT entry script, an EXE,
        // RESOURCE.000, and the SCI resource files.
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("SIERRA.BAT"));
        touch(&tmp.path().join("SIERRA.EXE"));
        touch(&tmp.path().join("RESOURCE.000"));
        touch(&tmp.path().join("RESOURCE.001"));
        touch(&tmp.path().join("VOCAB.SCI"));
        let det = detect(tmp.path());
        assert_eq!(det.platform, Some(Platform::Dos));
        assert_eq!(det.confidence, Confidence::High);
        assert!(det
            .evidence
            .iter()
            .any(|s| s.contains("RESOURCE.000")));
    }

    #[test]
    fn single_adf_classifies_as_amiga() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("Fatman.adf"));
        let det = detect(tmp.path());
        assert_eq!(det.platform, Some(Platform::Amiga));
        // One ADF only — Medium feels right.
        assert!(det.confidence >= Confidence::Medium);
    }

    #[test]
    fn startup_sequence_in_s_dir_is_amiga_signal() {
        let tmp = tempdir().unwrap();
        let s = tmp.path().join("S");
        std::fs::create_dir(&s).unwrap();
        touch(&s.join("Startup-Sequence"));
        touch(&tmp.path().join("Game.info"));
        let det = detect(tmp.path());
        assert_eq!(det.platform, Some(Platform::Amiga));
    }

    #[test]
    fn mixed_indicators_return_unknown_with_evidence() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("RUN.BAT"));
        touch(&tmp.path().join("DISK1.ADF"));
        let det = detect(tmp.path());
        assert_eq!(det.platform, None, "mixed indicators should be ambiguous");
        assert_eq!(det.confidence, Confidence::Low);
        assert!(!det.evidence.is_empty());
    }

    #[test]
    fn whdload_slave_is_amiga_signal() {
        let tmp = tempdir().unwrap();
        touch(&tmp.path().join("Fatman.slave"));
        touch(&tmp.path().join("Fatman.info"));
        let det = detect(tmp.path());
        assert_eq!(det.platform, Some(Platform::Amiga));
    }
}
