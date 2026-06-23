//! Entity-relationship diagram plugin
//!
//! Implements ER diagram parsing and rendering.

mod chumsky_parser;
mod database;
mod detector;
mod layout;
mod parser;
mod renderer;

pub use chumsky_parser::ChumskyErParser;
pub use database::{Attribute, Cardinality, Entity, ErDatabase, KeyKind};
pub use detector::ErDetector;
pub use layout::{ErLayoutAlgorithm, ErLayoutResult, PositionedEntity};
pub use parser::ErParser;
pub use renderer::ErRenderer;
