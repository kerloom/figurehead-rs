//! Entity-relationship diagram layout algorithm
//!
//! Calculates positions for entity boxes in a grid layout.

use anyhow::Result;
use unicode_width::UnicodeWidthStr;

use super::database::{Cardinality, Entity, ErDatabase};

/// Positioned entity box for rendering.
#[derive(Debug, Clone)]
pub struct PositionedEntity {
    pub name: String,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub attribute_lines: Vec<String>,
}

/// Positioned relationship for rendering.
#[derive(Debug, Clone)]
pub struct PositionedRelationship {
    pub from_entity: String,
    pub to_entity: String,
    pub from_cardinality: Cardinality,
    pub to_cardinality: Cardinality,
    pub label: Option<String>,
    pub from_x: usize,
    pub from_y: usize,
    pub to_x: usize,
    pub to_y: usize,
    pub horizontal: bool,
}

/// Layout result containing all positioned elements.
#[derive(Debug)]
pub struct ErLayoutResult {
    pub entities: Vec<PositionedEntity>,
    pub relationships: Vec<PositionedRelationship>,
    pub width: usize,
    pub height: usize,
}

/// ER diagram layout algorithm.
pub struct ErLayoutAlgorithm {
    box_padding: usize,
    box_spacing: usize,
    max_entities_per_row: usize,
}

impl ErLayoutAlgorithm {
    pub fn new() -> Self {
        Self {
            box_padding: 1,
            box_spacing: 4,
            max_entities_per_row: 2,
        }
    }

    /// Horizontal gap needed to fit the longest relationship label plus
    /// markers between two horizontally-adjacent entity boxes.
    fn needed_spacing(database: &ErDatabase, base: usize) -> usize {
        let max_label = database
            .relationships()
            .iter()
            .map(|r| r.label.as_ref().map(|l| l.chars().count()).unwrap_or(0))
            .max()
            .unwrap_or(0);
        base.max(max_label + 8)
    }

    /// Format an attribute's base line: `TYPE NAME [PK FK UK]`.
    fn base_attr_line(attr: &super::database::Attribute) -> String {
        let mut s = format!("{} {}", attr.attr_type, attr.name);
        for key in &attr.keys {
            s.push(' ');
            s.push_str(key.as_str());
        }
        s
    }

    /// Produce display lines for an attribute, wrapping comments to a second
    /// line when they would overflow the natural content width.
    fn attr_lines(attr: &super::database::Attribute, content_width: usize) -> Vec<String> {
        let base = Self::base_attr_line(attr);
        match &attr.comment {
            Some(comment) if !comment.is_empty() => {
                let with_comment = format!("{} \"{}\"", base, comment);
                if UnicodeWidthStr::width(with_comment.as_str()) <= content_width {
                    vec![with_comment]
                } else {
                    vec![base, format!("  \"{}\"", comment)]
                }
            }
            _ => vec![base],
        }
    }

    /// Compute the natural content width and resolved display lines for an
    /// entity.
    fn entity_content(entity: &Entity) -> (usize, Vec<String>) {
        let display = entity.display_name();
        let name_w = UnicodeWidthStr::width(display);
        let base_max = entity
            .attributes
            .iter()
            .map(|a| UnicodeWidthStr::width(Self::base_attr_line(a).as_str()))
            .max()
            .unwrap_or(0);
        let natural = name_w.max(base_max);

        let lines: Vec<String> = entity
            .attributes
            .iter()
            .flat_map(|a| Self::attr_lines(a, natural))
            .collect();

        let lines_max = lines
            .iter()
            .map(|l| UnicodeWidthStr::width(l.as_str()))
            .max()
            .unwrap_or(0);

        (natural.max(lines_max), lines)
    }

    /// Layout the diagram.
    pub fn layout(&self, database: &ErDatabase) -> Result<ErLayoutResult> {
        let entities = database.entities();

        if entities.is_empty() {
            return Ok(ErLayoutResult {
                entities: Vec::new(),
                relationships: Vec::new(),
                width: 0,
                height: 0,
            });
        }

        let entity_info: Vec<_> = entities
            .iter()
            .map(|e| {
                let (content_w, lines) = Self::entity_content(e);
                let width = content_w + self.box_padding * 2 + 2;
                let height = if lines.is_empty() {
                    3
                } else {
                    3 + 1 + lines.len()
                };
                (e, width, height, lines)
            })
            .collect();

        let spacing = Self::needed_spacing(database, self.box_spacing);

        let mut positioned = Vec::new();
        let mut x = 0;
        let mut y = 0;
        let mut row_height = 0;
        let mut max_width = 0;
        let mut in_row = 0;

        for (entity, width, height, lines) in entity_info {
            if in_row >= self.max_entities_per_row {
                y += row_height + self.box_spacing;
                x = 0;
                row_height = 0;
                in_row = 0;
            }

            positioned.push(PositionedEntity {
                name: entity.display_name().to_string(),
                x,
                y,
                width,
                height,
                attribute_lines: lines,
            });

            x += width + spacing;
            max_width = max_width.max(x);
            row_height = row_height.max(height);
            in_row += 1;
        }

        let total_width = max_width;
        let total_height = y + row_height;

        let mut positioned_rels = Vec::new();
        for rel in database.relationships() {
            let from = positioned.iter().find(|e| e.name == rel.from);
            let to = positioned.iter().find(|e| e.name == rel.to);

            if let (Some(from), Some(to)) = (from, to) {
                let same_row = from.y == to.y;
                let (from_x, from_y, to_x, to_y, horizontal) = if same_row {
                    let (left, right) = if from.x <= to.x {
                        (from, to)
                    } else {
                        (to, from)
                    };
                    let y = left.y + left.height / 2;
                    (left.x + left.width, y, right.x, y, true)
                } else {
                    let (top, bottom) = if from.y <= to.y {
                        (from, to)
                    } else {
                        (to, from)
                    };
                    let x = top.x + top.width / 2;
                    (x, top.y + top.height, x, bottom.y, false)
                };

                positioned_rels.push(PositionedRelationship {
                    from_entity: rel.from.clone(),
                    to_entity: rel.to.clone(),
                    from_cardinality: rel.from_cardinality,
                    to_cardinality: rel.to_cardinality,
                    label: rel.label.clone(),
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    horizontal,
                });
            }
        }

        Ok(ErLayoutResult {
            entities: positioned,
            relationships: positioned_rels,
            width: total_width,
            height: total_height,
        })
    }
}

impl Default for ErLayoutAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::database::{Attribute, Entity, KeyKind, Relationship};
    use super::*;

    #[test]
    fn test_empty_layout() {
        let db = ErDatabase::new();
        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();
        assert_eq!(result.entities.len(), 0);
    }

    #[test]
    fn test_single_entity() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("PayGroup")).unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "PayGroup");
    }

    #[test]
    fn test_entity_with_attributes_width() {
        let mut db = ErDatabase::new();
        let mut e = Entity::new("PayGroup");
        e.add_attribute(Attribute::new("Id", "int").with_key(KeyKind::Pk));
        e.add_attribute(Attribute::new("Name", "varchar"));
        db.add_entity(e).unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert!(result.entities[0].width > 10);
        assert_eq!(result.entities[0].attribute_lines.len(), 2);
    }

    #[test]
    fn test_multiple_entities_grid() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_entity(Entity::new("C")).unwrap();
        db.add_entity(Entity::new("D")).unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.entities.len(), 4);
        assert_eq!(result.entities[0].y, result.entities[1].y);
        assert!(result.entities[3].y > result.entities[0].y);
    }

    #[test]
    fn test_relationship_positioning_horizontal() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_relationship(Relationship::new(
            "A",
            "B",
            Cardinality::ExactlyOne,
            Cardinality::ZeroOrMore,
        ))
        .unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.relationships.len(), 1);
        assert!(result.relationships[0].horizontal);
    }

    #[test]
    fn test_comment_wraps_to_second_line() {
        let mut db = ErDatabase::new();
        let mut e = Entity::new("Doc");
        e.add_attribute(Attribute::new("Id", "int").with_key(KeyKind::Pk));
        e.add_attribute(
            Attribute::new("Name", "varchar").with_comment("a very long comment that overflows"),
        );
        db.add_entity(e).unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        let lines = &result.entities[0].attribute_lines;
        assert!(lines.len() >= 3);
        assert!(lines[2].starts_with("  \""));
    }

    #[test]
    fn test_entity_alias_displayed() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("PERSON").with_alias("Customer"))
            .unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        assert_eq!(result.entities[0].name, "Customer");
    }

    #[test]
    fn test_needed_spacing_accounts_for_labels() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_relationship(
            Relationship::new("A", "B", Cardinality::ExactlyOne, Cardinality::ZeroOrMore)
                .with_label("a very long relationship label"),
        )
        .unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();

        // The horizontal gap between the two boxes should be wider than the
        // base spacing of 4.
        let a = result.entities.iter().find(|e| e.name == "A").unwrap();
        let b = result.entities.iter().find(|e| e.name == "B").unwrap();
        let gap = b.x - (a.x + a.width);
        assert!(gap > 4, "gap={gap} should accommodate the long label");
    }
}
