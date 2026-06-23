//! Entity-relationship diagram layout algorithm
//!
//! Calculates positions for entity boxes in a grid layout.

use anyhow::Result;
use unicode_width::UnicodeWidthStr;

use super::database::{Cardinality, Entity, ErDatabase};

/// A point in canvas coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

impl Point {
    fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

/// Side of an entity box used as a connector port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSide {
    Left,
    Right,
    Top,
    Bottom,
}

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
    pub from_side: PortSide,
    pub to_side: PortSide,
    pub route: Vec<Point>,
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
                let (from_side, to_side) = Self::choose_ports(from, to);
                let from_port = Self::port_point(from, from_side);
                let to_port = Self::port_point(to, to_side);
                let route = Self::orthogonal_route(from_port, from_side, to_port, to_side);
                let horizontal = route.len() == 2 && from_port.y == to_port.y;

                positioned_rels.push(PositionedRelationship {
                    from_entity: rel.from.clone(),
                    to_entity: rel.to.clone(),
                    from_cardinality: rel.from_cardinality,
                    to_cardinality: rel.to_cardinality,
                    label: rel.label.clone(),
                    from_x: from_port.x,
                    from_y: from_port.y,
                    to_x: to_port.x,
                    to_y: to_port.y,
                    horizontal,
                    from_side,
                    to_side,
                    route,
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

    fn choose_ports(from: &PositionedEntity, to: &PositionedEntity) -> (PortSide, PortSide) {
        let from_cx = from.x + from.width / 2;
        let from_cy = from.y + from.height / 2;
        let to_cx = to.x + to.width / 2;
        let to_cy = to.y + to.height / 2;

        let dx = to_cx as isize - from_cx as isize;
        let dy = to_cy as isize - from_cy as isize;

        if dx.abs() >= dy.abs() {
            if dx >= 0 {
                (PortSide::Right, PortSide::Left)
            } else {
                (PortSide::Left, PortSide::Right)
            }
        } else if dy >= 0 {
            (PortSide::Bottom, PortSide::Top)
        } else {
            (PortSide::Top, PortSide::Bottom)
        }
    }

    fn port_point(entity: &PositionedEntity, side: PortSide) -> Point {
        let mid_x = entity.x + entity.width / 2;
        let mid_y = entity.y + entity.height / 2;

        match side {
            PortSide::Left => Point::new(entity.x.saturating_sub(1), mid_y),
            PortSide::Right => Point::new(entity.x + entity.width, mid_y),
            PortSide::Top => Point::new(mid_x, entity.y.saturating_sub(1)),
            PortSide::Bottom => Point::new(mid_x, entity.y + entity.height),
        }
    }

    fn orthogonal_route(
        from: Point,
        from_side: PortSide,
        to: Point,
        to_side: PortSide,
    ) -> Vec<Point> {
        if from.x == to.x || from.y == to.y {
            return vec![from, to];
        }

        let from_horizontal = matches!(from_side, PortSide::Left | PortSide::Right);
        let to_horizontal = matches!(to_side, PortSide::Left | PortSide::Right);

        if from_horizontal && to_horizontal {
            let mid_x = (from.x + to.x) / 2;
            vec![from, Point::new(mid_x, from.y), Point::new(mid_x, to.y), to]
        } else if !from_horizontal && !to_horizontal {
            let mid_y = (from.y + to.y) / 2;
            vec![from, Point::new(from.x, mid_y), Point::new(to.x, mid_y), to]
        } else if from_horizontal {
            vec![from, Point::new(to.x, from.y), to]
        } else {
            vec![from, Point::new(from.x, to.y), to]
        }
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
        assert_eq!(result.relationships[0].from_side, PortSide::Right);
        assert_eq!(result.relationships[0].to_side, PortSide::Left);
        assert_eq!(result.relationships[0].route.len(), 2);
    }

    #[test]
    fn test_relationship_positioning_vertical_ports() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_entity(Entity::new("C")).unwrap();
        db.add_relationship(Relationship::new(
            "A",
            "C",
            Cardinality::ExactlyOne,
            Cardinality::ZeroOrMore,
        ))
        .unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();
        let rel = &result.relationships[0];

        assert_eq!(rel.from_side, PortSide::Bottom);
        assert_eq!(rel.to_side, PortSide::Top);
        assert_eq!(rel.route.len(), 2);
    }

    #[test]
    fn test_relationship_non_aligned_route_has_waypoints() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();
        db.add_entity(Entity::new("C")).unwrap();
        db.add_entity(Entity::new("D")).unwrap();
        db.add_relationship(Relationship::new(
            "A",
            "D",
            Cardinality::ExactlyOne,
            Cardinality::ZeroOrMore,
        ))
        .unwrap();

        let layout = ErLayoutAlgorithm::new();
        let result = layout.layout(&db).unwrap();
        let rel = &result.relationships[0];

        assert!(
            rel.route.len() > 2,
            "non-aligned route should use waypoints"
        );
        for pair in rel.route.windows(2) {
            assert!(
                pair[0].x == pair[1].x || pair[0].y == pair[1].y,
                "each route segment must be orthogonal"
            );
        }
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
