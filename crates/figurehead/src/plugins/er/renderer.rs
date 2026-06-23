//! Entity-relationship diagram renderer
//!
//! Renders ER diagrams to ASCII art.

use anyhow::Result;
use unicode_width::UnicodeWidthStr;

use super::database::ErDatabase;
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

    /// Draw the relationship line with cardinality markers at each end.
    fn draw_relationship_line(&self, canvas: &mut AsciiCanvas, rel: &PositionedRelationship) {
        if rel.horizontal {
            let y = rel.from_y;
            let (left_x, right_x) = sort_pair(rel.from_x, rel.to_x);

            for x in left_x..right_x {
                canvas.set_char(x, y, '─');
            }

            let (left_marker, right_marker) = if rel.from_x <= rel.to_x {
                (
                    rel.from_cardinality.to_marker(),
                    rel.to_cardinality.to_marker(),
                )
            } else {
                (
                    rel.to_cardinality.to_marker(),
                    rel.from_cardinality.to_marker(),
                )
            };

            canvas.draw_text(left_x, y, left_marker);
            canvas.draw_text(
                right_x.saturating_sub(right_marker.chars().count()),
                y,
                right_marker,
            );
        } else {
            let x = rel.from_x;
            let (top_y, bottom_y) = sort_pair(rel.from_y, rel.to_y);

            for y in top_y..bottom_y {
                canvas.set_char(x, y, '│');
            }

            let (top_marker, bottom_marker) = if rel.from_y <= rel.to_y {
                (
                    rel.from_cardinality.to_marker(),
                    rel.to_cardinality.to_marker(),
                )
            } else {
                (
                    rel.to_cardinality.to_marker(),
                    rel.from_cardinality.to_marker(),
                )
            };

            canvas.draw_text(x.saturating_sub(1), top_y, top_marker);
            canvas.draw_text(
                x.saturating_sub(1),
                bottom_y.saturating_sub(1),
                bottom_marker,
            );
        }
    }

    /// Draw the relationship label centered in the gap between markers.
    fn draw_relationship_label(&self, canvas: &mut AsciiCanvas, rel: &PositionedRelationship) {
        let Some(ref label) = rel.label else { return };
        if label.is_empty() {
            return;
        }

        if rel.horizontal {
            let y = rel.from_y;
            let (left_x, right_x) = sort_pair(rel.from_x, rel.to_x);
            let mid_x = (left_x + 2 + right_x.saturating_sub(2)) / 2;
            canvas.draw_text(mid_x.saturating_sub(label.chars().count() / 2), y, label);
        } else {
            let x = rel.from_x;
            let (top_y, bottom_y) = sort_pair(rel.from_y, rel.to_y);
            let mid_y = (top_y + bottom_y) / 2;
            canvas.draw_text(x + 1, mid_y, label);
        }
    }

    /// Render the layout to ASCII art.
    pub fn render(&self, layout: &ErLayoutResult) -> Result<String> {
        if layout.entities.is_empty() {
            return Ok(String::new());
        }

        let extra = if layout.relationships.is_empty() {
            0
        } else {
            2
        };
        let mut canvas = AsciiCanvas::new(layout.width + 2, layout.height + extra + 1);

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
}
