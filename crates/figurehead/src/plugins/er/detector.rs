//! Entity-relationship diagram detector
//!
//! Identifies ER diagram syntax from input text.

use crate::core::Detector;

/// Detector for ER diagram syntax
pub struct ErDetector;

impl ErDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ErDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for ErDetector {
    fn detect(&self, input: &str) -> bool {
        self.confidence(input) > 0.5
    }

    fn confidence(&self, input: &str) -> f64 {
        let input_lower = input.to_lowercase();
        let trimmed = input_lower.trim();

        if trimmed.starts_with("erdiagram") {
            return 1.0;
        }

        let has_er_markers = input.contains("||--")
            || input.contains("}o--")
            || input.contains("}|--")
            || input.contains("|o--")
            || input.contains("--o{")
            || input.contains("--||")
            || input.contains("--}o")
            || input.contains("--}|")
            || input.contains("--o|");

        if has_er_markers {
            return 0.8;
        }

        0.0
    }

    fn diagram_type(&self) -> &'static str {
        "er"
    }

    fn patterns(&self) -> Vec<&'static str> {
        vec!["erDiagram"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_er_diagram_keyword() {
        let detector = ErDetector::new();
        assert!(detector.detect("erDiagram\n    PayGroup {"));
        assert!(detector.detect("ERDIAGRAM\n    PayGroup {"));
        assert!(detector.detect("\nerDiagram\n    A ||--o{ B"));
    }

    #[test]
    fn test_detects_er_markers() {
        let detector = ErDetector::new();
        assert!(detector.detect("A ||--o{ B : has"));
        assert!(detector.detect("A }o--|| B"));
    }

    #[test]
    fn test_confidence_scoring() {
        let detector = ErDetector::new();
        assert_eq!(detector.confidence("erDiagram\n    A"), 1.0);
        assert!(detector.confidence("A ||--o{ B") >= 0.8);
        assert_eq!(detector.confidence("graph TD; A-->B"), 0.0);
    }

    #[test]
    fn test_rejects_flowchart() {
        let detector = ErDetector::new();
        assert!(!detector.detect("graph TD; A-->B"));
        assert!(!detector.detect("flowchart LR; A-->B"));
    }

    #[test]
    fn test_rejects_class() {
        let detector = ErDetector::new();
        assert!(!detector.detect("classDiagram\n    class Animal"));
    }

    #[test]
    fn test_rejects_sequence() {
        let detector = ErDetector::new();
        assert!(!detector.detect("sequenceDiagram\n    Alice->>Bob: Hello"));
    }
}
