//! Pie chart plugin
//!
//! Implements Mermaid pie chart parsing and ASCII rendering.
//!
//! Syntax examples:
//! ```text
//! pie showData title Pets adopted by volunteers
//!     "Dogs" : 386
//!     "Cats" : 85
//! ```

mod database;
mod detector;
mod parser;
mod renderer;

pub use database::{PieDatabase, PieSlice};
pub use detector::PieDetector;
pub use parser::PieParser;
pub use renderer::PieRenderer;

use crate::core::{Detector, Diagram};
use std::sync::Arc;

/// Pie chart diagram implementation.
pub struct PieDiagram;

impl Diagram for PieDiagram {
    type Database = PieDatabase;
    type Parser = PieParser;
    type Renderer = PieRenderer;

    fn detector() -> Arc<dyn Detector> {
        Arc::new(PieDetector::new())
    }

    fn create_parser() -> Self::Parser {
        PieParser::new()
    }

    fn create_database() -> Self::Database {
        PieDatabase::new()
    }

    fn create_renderer() -> Self::Renderer {
        PieRenderer::new()
    }

    fn name() -> &'static str {
        "pie"
    }

    fn version() -> &'static str {
        "0.1.0"
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::core::{Database, Parser};

    #[test]
    fn test_full_pipeline() {
        let detector = PieDiagram::detector();
        let parser = PieDiagram::create_parser();
        let mut database = PieDiagram::create_database();
        let renderer = PieDiagram::create_renderer();

        let input = "pie\n    title Pets\n    \"Dogs\" : 386\n    \"Cats\" : 85";
        assert!(detector.detect(input));

        parser.parse(input, &mut database).unwrap();
        assert_eq!(database.node_count(), 2);

        let output = renderer.render(&database).unwrap();
        assert!(output.contains("Pets"));
        assert!(output.contains("Dogs"));
    }
}
