//! Entity-relationship diagram parser
//!
//! Parses ER diagram syntax into the database using chumsky.

use super::chumsky_parser::{ChumskyErParser, Statement};
use super::database::{Attribute, Entity, ErDatabase, Relationship};
use crate::core::Parser;
use anyhow::Result;

/// ER diagram parser using chumsky
pub struct ErParser {
    chumsky: ChumskyErParser,
}

impl ErParser {
    pub fn new() -> Self {
        Self {
            chumsky: ChumskyErParser::new(),
        }
    }
}

impl Default for ErParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser<ErDatabase> for ErParser {
    fn parse(&self, input: &str, database: &mut ErDatabase) -> Result<()> {
        let statements = self.chumsky.parse_diagram(input)?;

        for statement in statements {
            match statement {
                Statement::Entity(parsed_entity) => {
                    let mut entity = Entity::new(&parsed_entity.name);
                    if let Some(alias) = parsed_entity.alias {
                        entity = entity.with_alias(alias);
                    }
                    for attr in parsed_entity.attributes {
                        let mut db_attr = Attribute::new(attr.name, attr.attr_type);
                        if !attr.keys.is_empty() {
                            db_attr = db_attr.with_keys(attr.keys);
                        }
                        if let Some(comment) = attr.comment {
                            db_attr = db_attr.with_comment(comment);
                        }
                        entity.add_attribute(db_attr);
                    }
                    database.add_entity(entity)?;
                }
                Statement::Relationship(parsed_rel) => {
                    database.get_or_create_entity(&parsed_rel.from);
                    database.get_or_create_entity(&parsed_rel.to);

                    let mut rel = Relationship::new(
                        parsed_rel.from,
                        parsed_rel.to,
                        parsed_rel.from_cardinality,
                        parsed_rel.to_cardinality,
                    );
                    if let Some(label) = parsed_rel.label {
                        rel = rel.with_label(label);
                    }
                    database.add_relationship(rel)?;
                }
                Statement::Comment | Statement::Direction(_) | Statement::Style => {}
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "er"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn can_parse(&self, input: &str) -> bool {
        let lower = input.to_lowercase();
        lower.contains("erdiagram")
    }
}

#[cfg(test)]
mod tests {
    use super::super::database::{Cardinality, KeyKind};
    use super::*;

    #[test]
    fn test_parse_simple_entity() {
        let parser = ErParser::new();
        let mut db = ErDatabase::new();

        parser
            .parse("erDiagram\n    PayGroup { int Id PK }", &mut db)
            .unwrap();

        assert_eq!(db.entity_count(), 1);
        assert_eq!(db.entities()[0].name, "PayGroup");
        assert_eq!(db.entities()[0].attributes.len(), 1);
        assert_eq!(db.entities()[0].attributes[0].keys, vec![KeyKind::Pk]);
    }

    #[test]
    fn test_parse_relationship_creates_entities() {
        let parser = ErParser::new();
        let mut db = ErDatabase::new();

        parser
            .parse(
                r#"erDiagram
    PayGroup ||--o{ PayGroupUserMapping : "has users""#,
                &mut db,
            )
            .unwrap();

        assert_eq!(db.entity_count(), 2);
        assert_eq!(db.relationship_count(), 1);
        let rel = &db.relationships()[0];
        assert_eq!(rel.from, "PayGroup");
        assert_eq!(rel.to, "PayGroupUserMapping");
        assert_eq!(rel.from_cardinality, Cardinality::ExactlyOne);
        assert_eq!(rel.to_cardinality, Cardinality::ZeroOrMore);
        assert_eq!(rel.label.as_deref(), Some("has users"));
    }

    #[test]
    fn test_parse_full_diagram() {
        let parser = ErParser::new();
        let mut db = ErDatabase::new();

        let input = r#"erDiagram
    PayGroup {
        int Id PK
        varchar Name
        varchar CountryCode
    }
    PayGroupUserMapping {
        int Id PK
        int PayGroupId FK
        varchar UserId
    }
    PayGroup ||--o{ PayGroupUserMapping : "has users""#;

        parser.parse(input, &mut db).unwrap();

        assert_eq!(db.entity_count(), 2);
        assert_eq!(db.relationship_count(), 1);
        assert_eq!(db.entities()[1].attributes.len(), 3);
    }

    #[test]
    fn test_parse_comments_ignored() {
        let parser = ErParser::new();
        let mut db = ErDatabase::new();

        let input = r#"erDiagram
    %% top comment
    PayGroup {
        int Id PK
    }"#;

        parser.parse(input, &mut db).unwrap();

        assert_eq!(db.entity_count(), 1);
        assert_eq!(db.entities()[0].name, "PayGroup");
    }
}
