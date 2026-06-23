//! Entity-relationship diagram renderer
//!
//! Renders ER diagrams to ASCII art.

use anyhow::Result;
use unicode_width::UnicodeWidthStr;

use super::database::{Cardinality, ErDatabase};
use super::layout::{
    ErLayoutAlgorithm, ErLayoutResult, Point, PortSide, PositionedEntity, PositionedRelationship,
};
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

    /// Return a side-aware marker string. Left/top ports are visually mirrored
    /// so markers face the entity box (`o{` instead of canonical `}o`).
    fn marker_for_side(card: Cardinality, side: PortSide) -> &'static str {
        let mirror = matches!(side, PortSide::Left | PortSide::Top);
        match (card, mirror) {
            (Cardinality::ExactlyOne, _) => "||",
            (Cardinality::ZeroOrOne, false) => "|o",
            (Cardinality::ZeroOrOne, true) => "o|",
            (Cardinality::OneOrMore, false) => "}|",
            (Cardinality::OneOrMore, true) => "|{",
            (Cardinality::ZeroOrMore, false) => "}o",
            (Cardinality::ZeroOrMore, true) => "o{",
        }
    }

    /// Draw the relationship line with cardinality markers at each end.
    fn draw_relationship_line(&self, canvas: &mut AsciiCanvas, rel: &PositionedRelationship) {
        for pair in rel.route.windows(2) {
            Self::draw_segment(canvas, pair[0], pair[1]);
        }

        for triple in rel.route.windows(3) {
            let c = Self::corner_char(triple[0], triple[1], triple[2]);
            canvas.set_char(triple[1].x, triple[1].y, c);
        }

        Self::draw_marker(canvas, rel.route[0], rel.from_side, rel.from_cardinality);
        Self::draw_marker(
            canvas,
            *rel.route.last().unwrap(),
            rel.to_side,
            rel.to_cardinality,
        );
    }

    fn draw_segment(canvas: &mut AsciiCanvas, from: Point, to: Point) {
        if from.y == to.y {
            let (left, right) = sort_pair(from.x, to.x);
            for x in left..=right {
                Self::set_line_char(canvas, x, from.y, '─');
            }
        } else if from.x == to.x {
            let (top, bottom) = sort_pair(from.y, to.y);
            for y in top..=bottom {
                Self::set_line_char(canvas, from.x, y, '│');
            }
        }
    }

    fn set_line_char(canvas: &mut AsciiCanvas, x: usize, y: usize, c: char) {
        let existing = canvas.get_char(x, y);
        if existing == ' ' || existing == c {
            canvas.set_char(x, y, c);
        } else if matches!(
            existing,
            '─' | '│' | '┌' | '┐' | '└' | '┘' | '┬' | '┴' | '├' | '┤' | '┼'
        ) {
            canvas.set_char(x, y, '┼');
        }
    }

    fn corner_char(prev: Point, curr: Point, next: Point) -> char {
        let left = prev.x < curr.x || next.x < curr.x;
        let right = prev.x > curr.x || next.x > curr.x;
        let up = prev.y < curr.y || next.y < curr.y;
        let down = prev.y > curr.y || next.y > curr.y;

        match (left, right, up, down) {
            (true, true, true, true) => '┼',
            (true, true, true, false) => '┴',
            (true, true, false, true) => '┬',
            (true, false, true, true) => '┤',
            (false, true, true, true) => '├',
            (false, true, false, true) => '┌',
            (true, false, false, true) => '┐',
            (false, true, true, false) => '└',
            (true, false, true, false) => '┘',
            (true, true, false, false) => '─',
            (false, false, true, true) => '│',
            _ => '┼',
        }
    }

    fn draw_marker(canvas: &mut AsciiCanvas, point: Point, side: PortSide, card: Cardinality) {
        let marker = Self::marker_for_side(card, side);
        match side {
            PortSide::Left => canvas.draw_text(point.x.saturating_sub(2), point.y, marker),
            PortSide::Right => canvas.draw_text(point.x, point.y, marker),
            PortSide::Top | PortSide::Bottom => {
                canvas.draw_text(point.x.saturating_sub(1), point.y, marker)
            }
        }
    }

    // --- Label placement with collision avoidance ---

    /// Check if all cells in a horizontal span are safe for a label.
    fn is_clear_horizontal(
        canvas: &AsciiCanvas,
        x: usize,
        y: usize,
        len: usize,
        allow_line: bool,
    ) -> bool {
        (0..len).all(|i| {
            let c = canvas.get_char(x + i, y);
            c == ' ' || (allow_line && c == '─')
        })
    }

    /// Draw a single-line label only if target cells are safe. Never overwrites
    /// entity content; if all candidates are blocked, the label is skipped.
    fn draw_label_safe(canvas: &mut AsciiCanvas, candidates: &[(usize, usize, bool)], label: &str) {
        let len = label.chars().count();
        for &(x, y, allow_line) in candidates {
            if Self::is_clear_horizontal(canvas, x, y, len, allow_line) {
                canvas.draw_text(x, y, label);
                return;
            }
        }
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

        let Some((from, to)) = Self::longest_segment(rel) else {
            return;
        };

        if from.y == to.y {
            let y = from.y;
            let (left_x, right_x) = sort_pair(from.x, to.x);
            let label_len = label.chars().count();
            let mid_x = (left_x + right_x) / 2;
            let start_x = mid_x.saturating_sub(label_len / 2);
            Self::draw_label_safe(
                canvas,
                &[
                    (start_x, y.saturating_sub(1), false),
                    (start_x, y, true),
                    (start_x, y + 1, false),
                ],
                label,
            );
        } else {
            let x = from.x;
            let (top_y, bottom_y) = sort_pair(from.y, to.y);
            let mid_y = (top_y + bottom_y) / 2;
            let label_len = label.chars().count();
            Self::draw_label_safe(
                canvas,
                &[
                    (x + 2, mid_y, false),
                    (x + 2, mid_y.saturating_sub(1), false),
                    (x + 2, mid_y + 1, false),
                    (x.saturating_sub(label_len + 2), mid_y, false),
                ],
                label,
            );
        }
    }

    fn longest_segment(rel: &PositionedRelationship) -> Option<(Point, Point)> {
        rel.route
            .windows(2)
            .map(|pair| {
                let len = pair[0].x.abs_diff(pair[1].x) + pair[0].y.abs_diff(pair[1].y);
                (len, pair[0], pair[1])
            })
            .max_by_key(|(len, _, _)| *len)
            .map(|(_, from, to)| (from, to))
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
        assert!(result.contains("o{"));
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
    fn test_marker_for_side_exactly_one() {
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::ExactlyOne, PortSide::Right),
            "||"
        );
    }

    #[test]
    fn test_marker_for_side_zero_or_one() {
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::ZeroOrOne, PortSide::Right),
            "|o"
        );
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::ZeroOrOne, PortSide::Left),
            "o|"
        );
    }

    #[test]
    fn test_marker_for_side_one_or_more() {
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::OneOrMore, PortSide::Right),
            "}|"
        );
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::OneOrMore, PortSide::Top),
            "|{"
        );
    }

    #[test]
    fn test_marker_for_side_zero_or_more() {
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::ZeroOrMore, PortSide::Right),
            "}o"
        );
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::ZeroOrMore, PortSide::Bottom),
            "}o"
        );
        assert_eq!(
            ErRenderer::marker_for_side(Cardinality::ZeroOrMore, PortSide::Left),
            "o{"
        );
    }

    #[test]
    fn test_render_vertical_marker_side_by_side() {
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
        assert!(
            result.contains("||") || result.contains("}o"),
            "Expected side-by-side vertical port markers, got:\n{result}"
        );
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

    #[test]
    fn test_non_aligned_relationship_draws_corner() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_entity(Entity::new("C")).unwrap();
        db.add_entity(Entity::new("D")).unwrap();
        db.add_relationship(
            Relationship::new("A", "D", Cardinality::ExactlyOne, Cardinality::ZeroOrMore)
                .with_label("rel"),
        )
        .unwrap();

        let result = ErRenderer::new().render_database(&db).unwrap();
        assert!(
            result.contains('┌')
                || result.contains('┐')
                || result.contains('└')
                || result.contains('┘'),
            "expected an orthogonal route with a corner, got:\n{result}"
        );
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
        assert!(ErRenderer::is_clear_horizontal(&canvas, 0, 0, 5, false));
        canvas.set_char(2, 0, 'X');
        assert!(!ErRenderer::is_clear_horizontal(&canvas, 0, 0, 5, false));
        assert!(ErRenderer::is_clear_horizontal(&canvas, 3, 0, 5, false));
        canvas.set_char(4, 0, '─');
        assert!(!ErRenderer::is_clear_horizontal(&canvas, 3, 0, 5, false));
        assert!(ErRenderer::is_clear_horizontal(&canvas, 3, 0, 5, true));
    }
}
