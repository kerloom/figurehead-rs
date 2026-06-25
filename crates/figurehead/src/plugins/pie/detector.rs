//! Pie chart detector
//!
//! Identifies Mermaid pie chart syntax.

use crate::core::Detector;

/// Detector for pie chart syntax.
pub struct PieDetector;

impl PieDetector {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PieDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for PieDetector {
    fn detect(&self, input: &str) -> bool {
        self.confidence(input) > 0.5
    }

    fn confidence(&self, input: &str) -> f64 {
        let trimmed = input.trim_start();
        let first_line = trimmed.lines().next().unwrap_or_default().trim_start();
        let lower = first_line.to_lowercase();

        if lower == "pie" || lower.starts_with("pie ") {
            return 1.0;
        }

        0.0
    }

    fn diagram_type(&self) -> &'static str {
        "pie"
    }

    fn patterns(&self) -> Vec<&'static str> {
        vec!["pie", "showData", "title", "\"label\" : value"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_pie_keyword() {
        let detector = PieDetector::new();

        assert!(detector.detect("pie\n    \"Dogs\" : 386"));
        assert!(detector.detect("PIE\n    \"Dogs\" : 386"));
        assert!(detector.detect("pie showData\n    \"Dogs\" : 386"));
        assert!(detector.detect("pie title Pets\n    \"Dogs\" : 386"));
    }

    #[test]
    fn test_rejects_other_diagrams() {
        let detector = PieDetector::new();

        assert!(!detector.detect("graph TD\n    A --> B"));
        assert!(!detector.detect("sequenceDiagram\n    A->>B: Hi"));
    }
}
