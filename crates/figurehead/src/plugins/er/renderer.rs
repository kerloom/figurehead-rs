//! Entity-relationship diagram renderer
//!
//! Renders ER diagrams to ASCII art.

use anyhow::Result;
use unicode_width::UnicodeWidthStr;

use super::database::{Cardinality, ErDatabase};
use super::layout::{ErLayoutAlgorithm, ErLayoutResult, PositionedEntity, PositionedRelationship};
use crate::core::{AsciiCanvas, BoxChars, CharacterSet};

/// ER diagram renderer.
pub struct ErRenderer;

impl ErRenderer {
    pub fn new() -> Self {
        Self
    }

    fn draw_text_centered(
        &self,
        canvas: &mut AsciiCanvas,
        x: usize,
        y: usize,
        width: usize,
        text: &str,
    ) {
        let text_width = UnicodeWidthStr::width(text);
        let padding = width.saturating_sub(text_width) / 2;
        canvas.draw_text(x + padding, y, text);
    }

    fn draw_entity(&self, canvas: &mut AsciiCanvas, entity: &PositionedEntity) {
        let x = entity.x;
        let y = entity.y;
        let w = entity.width;
        let chars = BoxChars::rectangle(CharacterSet::Unicode);

        // Top border
        canvas.set_char(x, y, chars.top_left);
        canvas.draw_horizontal_line(x + 1, y, w - 2, chars.horizontal);
        canvas.set_char(x + w - 1, y, chars.top_right);

        // Entity name (centered)
        canvas.set_char(x, y + 1, chars.vertical);
        canvas.draw_horizontal_line(x + 1, y + 1, w - 2, ' ');
        self.draw_text_centered(canvas, x + 1, y + 1, w - 2, &entity.name);
        canvas.set_char(x + w - 1, y + 1, chars.vertical);

        // Attribute section
        let mut cy = y + 2;
        if !entity.attribute_lines.is_empty() {
            canvas.set_char(x, cy, chars.t_right);
            canvas.draw_horizontal_line(x + 1, cy, w - 2, chars.horizontal);
            canvas.set_char(x + w - 1, cy, chars.t_left);
            cy += 1;

            for line in &entity.attribute_lines {
                canvas.set_char(x, cy, chars.vertical);
                canvas.draw_horizontal_line(x + 1, cy, w - 2, ' ');
                canvas.draw_text(x + 2, cy, line);
                canvas.set_char(x + w - 1, cy, chars.vertical);
                cy += 1;
            }
        }

        // Bottom border
        canvas.set_char(x, cy, chars.bottom_left);
        canvas.draw_horizontal_line(x + 1, cy, w - 2, chars.horizontal);
        canvas.set_char(x + w - 1, cy, chars.bottom_right);
    }

    // --- Side-aware cardinality markers ---

    /// Return the two characters of a cardinality marker, split for vertical
    /// stacking.
    fn marker_chars(card: Cardinality) -> (char, char) {
        match card {
            Cardinality::ExactlyOne => ('|', '|'),
            Cardinality::ZeroOrOne => ('|', 'o'),
            Cardinality::OneOrMore => ('}', '|'),
            Cardinality::ZeroOrMore => ('}', 'o'),
        }
    }

    /// Draw the relationship line with cardinality markers at each end.
    ///
    /// For horizontal: canonical 2-char marker strings (e.g. `||`, `}o`).
    /// For vertical: markers stacked vertically (one char per row).
    fn draw_relationship_line(&self, canvas: &mut AsciiCanvas, rel: &PositionedRelationship) {
        if rel.horizontal {
            let y = rel.from_y;
            let (left_x, right_x) = sort_pair(rel.from_x, rel.to_x);

            // Draw the line, leaving 2 chars at each end for markers
            let line_start = left_x + 2;
            let line_end = right_x.saturating_sub(2);
            for x in line_start..line_end {
                canvas.set_char(x, y, '─');
            }

            let (left_card, right_card) = if rel.from_x <= rel.to_x {
                (rel.from_cardinality, rel.to_cardinality)
            } else {
                (rel.to_cardinality, rel.from_cardinality)
            };

            canvas.draw_text(left_x, y, left_card.to_marker());
            canvas.draw_text(right_x - 2, y, right_card.to_marker());
        } else {
            let x = rel.from_x;
            let (top_y, bottom_y) = sort_pair(rel.from_y, rel.to_y);

            // Draw the line, leaving 2 rows at each end for markers
            let line_start = top_y + 2;
            let line_end = bottom_y.saturating_sub(2);
            for y in line_start..line_end {
                canvas.set_char(x, y, '│');
            }

            let (top_card, bottom_card) = if rel.from_y <= rel.to_y {
                (rel.from_cardinality, rel.to_cardinality)
            } else {
                (rel.to_cardinality, rel.from_cardinality)
            };

            // Stack marker chars vertically: first char on top, second below
            let (t1, t2) = Self::marker_chars(top_card);
            canvas.set_char(x, top_y, t1);
            canvas.set_char(x, top_y + 1, t2);

            let (b1, b2) = Self::marker_chars(bottom_card);
            canvas.set_char(x, bottom_y - 2, b1);
            canvas.set_char(x, bottom_y - 1, b2);
        }
    }

    // --- Label placement with collision avoidance ---

    /// Check if all cells in a horizontal span are whitespace (or canvas
    /// default).  Returns true if safe to draw.
    fn is_clear_horizontal(canvas: &AsciiCanvas, x: usize, y: usize, len: usize) -> bool {
        (0..len).all(|i| canvas.get_char(x + i, y) == ' ')
    }

    /// Draw a single-line label at (x, y) only if the target cells are
    /// whitespace.  Falls back to y-1 or y+1 if blocked.
    fn draw_label_safe(canvas: &mut AsciiCanvas, x: usize, y: usize, label: &str) {
        let len = label.chars().count();
        // Try the preferred row, then above, then below
        for &try_y in &[y, y.saturating_sub(1), y + 1] {
            if Self::is_clear_horizontal(canvas, x, try_y, len) {
                canvas.draw_text(x, try_y, label);
                return;
            }
        }
        // Last resort: draw at the original position
        canvas.draw_text(x, y, label);
    }

    /// Draw the relationship label in a reserved whitespace lane.
    ///
    /// For horizontal: above the connector line.
    /// For vertical: to the right of the connector line.
    fn draw_relationship_label(&self, canvas: &mut AsciiCanvas, rel: &PositionedRelationship) {
        let Some(ref label) = rel.label else { return };
        if label.is_empty() {
            return;
        }

        if rel.horizontal {
            let y = rel.from_y;
            let (left_x, right_x) = sort_pair(rel.from_x, rel.to_x);
            let label_len = label.chars().count();
            // Center the label in the gap between markers (above the line)
            let gap_start = left_x + 2;
            let gap_end = right_x.saturating_sub(2);
            let mid_x = (gap_start + gap_end) / 2;
            let start_x = mid_x.saturating_sub(label_len / 2);
            // Draw one row above the line
            Self::draw_label_safe(canvas, start_x, y.saturating_sub(1), label);
        } else {
            let x = rel.from_x;
            let (top_y, bottom_y) = sort_pair(rel.from_y, rel.to_y);
            let mid_y = (top_y + bottom_y) / 2;
            // Draw to the right of the line with a 1-char gap
            Self::draw_label_safe(canvas, x + 2, mid_y, label);
        }
    }

    /// Render the layout to ASCII art.
    pub fn render(&self, layout: &ErLayoutResult) -> Result<String> {
        if layout.entities.is_empty() {
            return Ok(String::new());
        }

        // Extra space: 1 row above for labels, 1 below for vertical markers
        let extra = if layout.relationships.is_empty() {
            0
        } else {
            3
        };
        let mut canvas = AsciiCanvas::new(layout.width + 4, layout.height + extra + 1);

        for rel in &layout.relationships {
            self.draw_relationship_line(&mut canvas, rel);
        }
        for entity in &layout.entities {
            self.draw_entity(&mut canvas, entity);
        }
        for rel in &layout.relationships {
            self.draw_relationship_label(&mut canvas, rel);
        }

        Ok(canvas.to_string())
    }

    /// Convenience method to render directly from a database.
    pub fn render_database(&self, database: &ErDatabase) -> Result<String> {
        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(database)?;
        self.render(&result)
    }
}

fn sort_pair(a: usize, b: usize) -> (usize, usize) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

impl Default for ErRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::database::{Attribute, Cardinality, Entity, KeyKind, Relationship};
    use super::*;

    #[test]
    fn test_render_empty() {
        let db = ErDatabase::new();
        let renderer = ErRenderer::new();
        assert_eq!(renderer.render_database(&db).unwrap(), "");
    }

    #[test]
    fn test_render_simple_entity() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("PayGroup")).unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        assert!(result.contains("PayGroup"));
        assert!(result.contains('┌'));
        assert!(result.contains('└'));
    }

    #[test]
    fn test_render_entity_with_attributes() {
        let mut db = ErDatabase::new();
        let mut e = Entity::new("PayGroup");
        e.add_attribute(Attribute::new("Id", "int").with_key(KeyKind::Pk));
        e.add_attribute(Attribute::new("Name", "varchar"));
        db.add_entity(e).unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        assert!(result.contains("PayGroup"));
        assert!(result.contains("int Id PK"));
        assert!(result.contains("varchar Name"));
        assert!(result.contains('├'));
    }

    #[test]
    fn test_render_relationship_markers() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("PayGroup")).unwrap();
        db.add_entity(Entity::new("PayGroupUserMapping")).unwrap();
        db.add_relationship(
            Relationship::new(
                "PayGroup",
                "PayGroupUserMapping",
                Cardinality::ExactlyOne,
                Cardinality::ZeroOrMore,
            )
            .with_label("has users"),
        )
        .unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        assert!(result.contains("||"));
        assert!(result.contains("}o"));
        assert!(result.contains("has users"));
    }

    #[test]
    fn test_render_comment_wrapped() {
        let mut db = ErDatabase::new();
        let mut e = Entity::new("Doc");
        e.add_attribute(Attribute::new("Id", "int").with_key(KeyKind::Pk));
        e.add_attribute(
            Attribute::new("Name", "varchar").with_comment("a very long comment that overflows"),
        );
        db.add_entity(e).unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        assert!(result.contains("varchar Name"));
        assert!(result.contains("a very long comment that overflows"));
    }

    #[test]
    fn test_render_box_structure() {
        let mut db = ErDatabase::new();
        let mut e = Entity::new("X");
        e.add_attribute(Attribute::new("a", "int"));
        db.add_entity(e).unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        let lines: Vec<_> = result.lines().collect();
        assert!(lines.len() >= 5);
        assert!(lines[0].starts_with('┌'));
        assert!(lines[lines.len() - 1].starts_with('└'));
    }

    // --- Side-aware marker tests ---

    #[test]
    fn test_marker_chars_exactly_one() {
        assert_eq!(
            ErRenderer::marker_chars(Cardinality::ExactlyOne),
            ('|', '|')
        );
    }

    #[test]
    fn test_marker_chars_zero_or_one() {
        assert_eq!(ErRenderer::marker_chars(Cardinality::ZeroOrOne), ('|', 'o'));
    }

    #[test]
    fn test_marker_chars_one_or_more() {
        assert_eq!(ErRenderer::marker_chars(Cardinality::OneOrMore), ('}', '|'));
    }

    #[test]
    fn test_marker_chars_zero_or_more() {
        assert_eq!(
            ErRenderer::marker_chars(Cardinality::ZeroOrMore),
            ('}', 'o')
        );
    }

    #[test]
    fn test_render_vertical_marker_stacked() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_entity(Entity::new("C")).unwrap();
        // A and C are on different rows (2-per-row layout)
        db.add_relationship(Relationship::new(
            "A",
            "C",
            Cardinality::ExactlyOne,
            Cardinality::ZeroOrMore,
        ))
        .unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        // Vertical markers should be stacked (one char per row), not side by side
        let lines: Vec<&str> = result.lines().collect();
        let mut found_stacked = false;
        for x in 0..lines.iter().map(|l| l.len()).max().unwrap_or(0) {
            for i in 0..lines.len().saturating_sub(1) {
                let c1 = lines[i].chars().nth(x);
                let c2 = lines[i + 1].chars().nth(x);
                if c1 == Some('|') && c2 == Some('|') {
                    found_stacked = true;
                }
            }
        }
        assert!(found_stacked, "Expected stacked vertical || marker");
    }

    // --- Label lane tests ---

    #[test]
    fn test_horizontal_label_above_line() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_relationship(
            Relationship::new("A", "B", Cardinality::ExactlyOne, Cardinality::ZeroOrMore)
                .with_label("rel"),
        )
        .unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        let lines: Vec<&str> = result.lines().collect();
        // Find the row with "rel"
        let rel_row = lines.iter().position(|l| l.contains("rel"));
        assert!(rel_row.is_some(), "Label 'rel' should appear in output");
        let rel_row = rel_row.unwrap();
        // The row below should contain the connector line (─)
        if rel_row + 1 < lines.len() {
            assert!(
                lines[rel_row + 1].contains('─'),
                "Connector line should be below the label"
            );
        }
    }

    #[test]
    fn test_vertical_label_beside_line() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_entity(Entity::new("C")).unwrap();
        db.add_entity(Entity::new("D")).unwrap();
        db.add_relationship(
            Relationship::new("A", "C", Cardinality::ExactlyOne, Cardinality::ZeroOrMore)
                .with_label("rel"),
        )
        .unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        assert!(result.contains("rel"));
    }

    // --- Collision avoidance test ---

    #[test]
    fn test_label_does_not_overwrite_entity() {
        let mut db = ErDatabase::new();
        let mut e = Entity::new("Short");
        e.add_attribute(Attribute::new("Id", "int").with_key(KeyKind::Pk));
        db.add_entity(e).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_relationship(
            Relationship::new(
                "Short",
                "B",
                Cardinality::ExactlyOne,
                Cardinality::ZeroOrMore,
            )
            .with_label("rel"),
        )
        .unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        // The label "rel" should appear, and "int Id PK" should still be intact
        assert!(result.contains("int Id PK"));
        assert!(result.contains("rel"));
    }

    #[test]
    fn test_is_clear_horizontal() {
        let mut canvas = AsciiCanvas::new(10, 3);
        assert!(ErRenderer::is_clear_horizontal(&canvas, 0, 0, 5));
        canvas.set_char(2, 0, 'X');
        assert!(!ErRenderer::is_clear_horizontal(&canvas, 0, 0, 5));
        assert!(ErRenderer::is_clear_horizontal(&canvas, 3, 0, 5));
    }
}
