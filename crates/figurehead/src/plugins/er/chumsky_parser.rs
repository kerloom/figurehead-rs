//! Entity-relationship diagram parser using chumsky
//!
//! Parses Mermaid.js erDiagram syntax into AST structures.

use super::database::{Cardinality, KeyKind};
use crate::core::chumsky_utils::{
    inline_whitespace, mermaid_comment, optional_whitespace, whitespace_required,
};
use anyhow::Result;
use chumsky::prelude::*;
use chumsky::text::keyword;

// =========================================================================
// AST types
// =========================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAttribute {
    pub attr_type: String,
    pub name: String,
    pub keys: Vec<KeyKind>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEntity {
    pub name: String,
    pub alias: Option<String>,
    pub attributes: Vec<ParsedAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRelationship {
    pub from: String,
    pub to: String,
    pub from_cardinality: Cardinality,
    pub to_cardinality: Cardinality,
    pub label: Option<String>,
}

/// Diagram direction (parsed but layout is grid-based for v1).
#[derive(Debug, Clone, PartialEq)]
pub enum Direction {
    Tb,
    Bt,
    Lr,
    Rl,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Entity(ParsedEntity),
    Relationship(ParsedRelationship),
    Direction(Direction),
    Comment,
    Style,
}

// =========================================================================
// Parser
// =========================================================================

/// Chumsky-based ER diagram parser
pub struct ChumskyErParser;

impl ChumskyErParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_diagram(&self, input: &str) -> Result<Vec<Statement>> {
        Self::diagram_parser()
            .parse(input)
            .into_result()
            .map_err(|errors| anyhow::anyhow!("Parse errors: {:?}", errors))
    }

    pub fn parse_statement(&self, input: &str) -> Result<Statement> {
        Self::statement_parser()
            .then_ignore(end())
            .parse(input)
            .into_result()
            .map_err(|errors| anyhow::anyhow!("Parse errors: {:?}", errors))
    }

    fn diagram_parser<'src>() -> impl Parser<'src, &'src str, Vec<Statement>> {
        let header = keyword("erDiagram")
            .or(keyword("erdiagram"))
            .or(keyword("ERDIAGRAM"))
            .or_not();

        optional_whitespace()
            .ignore_then(header)
            .then_ignore(optional_whitespace())
            .ignore_then(
                Self::statement_parser()
                    .separated_by(whitespace_required())
                    .allow_trailing()
                    .collect(),
            )
            .then_ignore(optional_whitespace())
            .then_ignore(end())
    }

    fn statement_parser<'src>() -> impl Parser<'src, &'src str, Statement> + Clone {
        Self::comment_parser()
            .to(Statement::Comment)
            .or(Self::direction_parser().map(Statement::Direction))
            .or(Self::style_parser().to(Statement::Style))
            .or(Self::entity_parser().map(Statement::Entity))
            .or(Self::relationship_parser().map(Statement::Relationship))
            .or(Self::standalone_entity_parser().map(Statement::Entity))
    }

    fn comment_parser<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
        mermaid_comment()
    }

    fn direction_parser<'src>() -> impl Parser<'src, &'src str, Direction> + Clone {
        keyword("direction")
            .ignore_then(inline_whitespace())
            .ignore_then(
                keyword("TB")
                    .to(Direction::Tb)
                    .or(keyword("BT").to(Direction::Bt))
                    .or(keyword("LR").to(Direction::Lr))
                    .or(keyword("RL").to(Direction::Rl)),
            )
    }

    /// `style`, `classDef`, and `class` statements — parsed and discarded.
    fn style_parser<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
        keyword("style")
            .or(keyword("classDef"))
            .or(keyword("class"))
            .ignore_then(inline_whitespace())
            .ignore_then(none_of('\n').repeated().ignored())
    }

    // --- Token parsers --------------------------------------------------

    /// The `:::className` operator (discarded — styling is N/A for ASCII).
    fn class_apply<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
        just(":::").ignore_then(none_of(" \t\n\r").repeated().ignored())
    }

    /// A quoted string: `"any text"`.
    fn quoted_token<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        just('"')
            .ignore_then(none_of("\"\n\r").repeated().to_slice())
            .then_ignore(just('"'))
            .map(|s: &str| s.to_string())
    }

    /// A bare identifier: any run of characters that aren't whitespace or
    /// syntax-delimiters.  Allows `.`, `(`, `)`, `-`, `_`, etc.
    fn ident_token<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        none_of(" \t\n\r{}[]\"|}:;,")
            .repeated()
            .at_least(1)
            .to_slice()
            .map(|s: &str| s.trim().to_string())
    }

    /// An entity/relation name: quoted or bare identifier.
    fn name_token<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        Self::quoted_token().or(Self::ident_token())
    }

    /// `Name` or `Name[alias]`, optionally followed by `:::className`.
    fn name_with_alias<'src>() -> impl Parser<'src, &'src str, (String, Option<String>)> + Clone {
        Self::name_token()
            .then(
                just('[')
                    .ignore_then(Self::name_token())
                    .then_ignore(just(']'))
                    .or_not(),
            )
            .then_ignore(Self::class_apply().or_not())
            .map(|(name, alias)| (name, alias))
    }

    /// Attribute type token — allows parentheses, brackets, commas, etc.
    /// Excludes only whitespace, braces, quotes, and pipe/colon which would
    /// terminate the attribute line.
    fn type_token<'src>() -> impl Parser<'src, &'src str, String> + Clone {
        none_of(" \t\n\r{}\"|}:;.")
            .repeated()
            .at_least(1)
            .to_slice()
            .map(|s: &str| s.trim().to_string())
    }

    // --- Statement parsers ---------------------------------------------

    /// A standalone entity name (no body, no relationship).
    fn standalone_entity_parser<'src>() -> impl Parser<'src, &'src str, ParsedEntity> + Clone {
        Self::name_with_alias().map(|(name, alias)| ParsedEntity {
            name,
            alias,
            attributes: vec![],
        })
    }

    fn entity_parser<'src>() -> impl Parser<'src, &'src str, ParsedEntity> + Clone {
        Self::name_with_alias()
            .then_ignore(optional_whitespace())
            .then_ignore(just('{'))
            .then_ignore(optional_whitespace())
            .then(
                Self::attribute_parser()
                    .separated_by(whitespace_required())
                    .allow_trailing()
                    .collect(),
            )
            .then_ignore(optional_whitespace())
            .then_ignore(just('}'))
            .map(|((name, alias), attributes)| ParsedEntity {
                name,
                alias,
                attributes,
            })
    }

    fn attribute_parser<'src>() -> impl Parser<'src, &'src str, ParsedAttribute> + Clone {
        let ws1 = just(' ').or(just('\t')).repeated().at_least(1).ignored();

        // Attribute name: `*` prefix indicates PK (spec). The `*` is stripped
        // and PK is added to keys.
        let attr_name = just('*')
            .ignore_then(Self::name_token())
            .map(|n| (n, Some(KeyKind::Pk)))
            .or(Self::name_token().map(|n| (n, None)));

        let key = keyword("PK")
            .to(KeyKind::Pk)
            .or(keyword("FK").to(KeyKind::Fk))
            .or(keyword("UK").to(KeyKind::Uk));
        let explicit_keys = key
            .separated_by(just(',').then_ignore(inline_whitespace().or_not()))
            .at_least(1)
            .collect::<Vec<_>>()
            .or(empty().to(vec![]));

        Self::type_token()
            .then_ignore(ws1)
            .then(attr_name)
            .then(inline_whitespace().ignore_then(explicit_keys).or_not())
            .then(
                inline_whitespace()
                    .ignore_then(Self::quoted_token())
                    .or_not(),
            )
            .map(|(((attr_type, (name, star_pk)), explicit_keys), comment)| {
                let mut keys = explicit_keys.unwrap_or_default();
                if let Some(pk) = star_pk {
                    if !keys.contains(&pk) {
                        keys.insert(0, pk);
                    }
                }
                ParsedAttribute {
                    attr_type,
                    name,
                    keys,
                    comment,
                }
            })
    }

    // --- Cardinality / connector ---------------------------------------

    fn cardinality_mark<'src>() -> impl Parser<'src, &'src str, Cardinality> + Clone {
        just("||")
            .to(Cardinality::ExactlyOne)
            .or(just("}|").to(Cardinality::OneOrMore))
            .or(just("|{").to(Cardinality::OneOrMore))
            .or(just("}o").to(Cardinality::ZeroOrMore))
            .or(just("o{").to(Cardinality::ZeroOrMore))
            .or(just("|o").to(Cardinality::ZeroOrOne))
            .or(just("o|").to(Cardinality::ZeroOrOne))
    }

    fn cardinality_word<'src>() -> impl Parser<'src, &'src str, Cardinality> + Clone {
        just("one or zero")
            .to(Cardinality::ZeroOrOne)
            .or(just("zero or one").to(Cardinality::ZeroOrOne))
            .or(just("one or more").to(Cardinality::OneOrMore))
            .or(just("one or many").to(Cardinality::OneOrMore))
            .or(just("many(1)").to(Cardinality::OneOrMore))
            .or(just("1+").to(Cardinality::OneOrMore))
            .or(just("zero or more").to(Cardinality::ZeroOrMore))
            .or(just("zero or many").to(Cardinality::ZeroOrMore))
            .or(just("many(0)").to(Cardinality::ZeroOrMore))
            .or(just("0+").to(Cardinality::ZeroOrMore))
            .or(just("only one").to(Cardinality::ExactlyOne))
            .or(just("1").to(Cardinality::ExactlyOne))
    }

    fn cardinality<'src>() -> impl Parser<'src, &'src str, Cardinality> + Clone {
        Self::cardinality_mark().or(Self::cardinality_word())
    }

    /// `--` (identifying), `..` (non-identifying), `to`, `optionally to`.
    fn connector<'src>() -> impl Parser<'src, &'src str, ()> + Clone {
        just("--")
            .or(just(".."))
            .or(keyword("to"))
            .or(keyword("optionally")
                .ignore_then(inline_whitespace())
                .ignore_then(keyword("to")))
            .ignored()
    }

    fn relationship_parser<'src>() -> impl Parser<'src, &'src str, ParsedRelationship> + Clone {
        let ws = inline_whitespace();
        let label = just(':')
            .ignore_then(ws.clone())
            .ignore_then(Self::quoted_token().or(Self::ident_token()))
            .or_not();

        Self::name_with_alias()
            .then_ignore(ws.clone())
            .then(Self::cardinality())
            .then_ignore(ws.clone())
            .then_ignore(Self::connector())
            .then_ignore(ws.clone())
            .then(Self::cardinality())
            .then_ignore(ws.clone())
            .then(Self::name_with_alias())
            .then_ignore(ws.clone())
            .then(label)
            .map(
                |(((((from, _from_alias), from_card), to_card), (to, _to_alias)), label)| {
                    ParsedRelationship {
                        from,
                        to,
                        from_cardinality: from_card,
                        to_cardinality: to_card,
                        label: label.filter(|s: &String| !s.is_empty()),
                    }
                },
            )
    }
}

impl Default for ChumskyErParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_entity() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("PayGroup { int Id PK }").unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.name, "PayGroup");
                assert_eq!(entity.attributes.len(), 1);
                assert_eq!(entity.attributes[0].attr_type, "int");
                assert_eq!(entity.attributes[0].name, "Id");
                assert_eq!(entity.attributes[0].keys, vec![KeyKind::Pk]);
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_entity_with_comment() {
        let parser = ChumskyErParser::new();
        let input = r#"FileUUID { varchar FileUUID "DMS document reference" }"#;
        let result = parser.parse_statement(input).unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(
                    entity.attributes[0].comment.as_deref(),
                    Some("DMS document reference")
                );
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_entity_multiline() {
        let parser = ChumskyErParser::new();
        let input = "PayGroup {\n    int Id PK\n    varchar Name\n}";
        let result = parser.parse_statement(input).unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.name, "PayGroup");
                assert_eq!(entity.attributes.len(), 2);
                assert_eq!(entity.attributes[1].name, "Name");
                assert!(entity.attributes[1].keys.is_empty());
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_entity_with_alias() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("PERSON[Customer] { int Id PK }")
            .unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.name, "PERSON");
                assert_eq!(entity.alias.as_deref(), Some("Customer"));
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_entity_quoted_name() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement(r#""name with space" { int Id PK }"#)
            .unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.name, "name with space");
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_type_with_parens() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("User { varchar(255) Name }")
            .unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.attributes[0].attr_type, "varchar(255)");
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_type_decimal_with_precision() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("Price { decimal(10,2) Amount }")
            .unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.attributes[0].attr_type, "decimal(10,2)");
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_uk_key() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("User { varchar Email UK }").unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.attributes[0].keys, vec![KeyKind::Uk]);
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_multiple_keys() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("User { int Id PK, FK }").unwrap();

        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.attributes[0].keys, vec![KeyKind::Pk, KeyKind::Fk]);
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_relationship() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement(r#"PayGroup ||--o{ PayGroupUserMapping : "has users""#)
            .unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from, "PayGroup");
                assert_eq!(rel.to, "PayGroupUserMapping");
                assert_eq!(rel.from_cardinality, Cardinality::ExactlyOne);
                assert_eq!(rel.to_cardinality, Cardinality::ZeroOrMore);
                assert_eq!(rel.label.as_deref(), Some("has users"));
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_no_label() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("CustomerSubscription }o--|| SubscriptionType")
            .unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from_cardinality, Cardinality::ZeroOrMore);
                assert_eq!(rel.to_cardinality, Cardinality::ExactlyOne);
                assert!(rel.label.is_none());
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_bare_label() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("Invoice ||--o| InvoiceSubscription : recurring")
            .unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.to_cardinality, Cardinality::ZeroOrOne);
                assert_eq!(rel.label.as_deref(), Some("recurring"));
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_cardinality_words() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("A one or more -- zero or more B")
            .unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from_cardinality, Cardinality::OneOrMore);
                assert_eq!(rel.to_cardinality, Cardinality::ZeroOrMore);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_cardinality_1plus() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("A 1+ -- 0+ B").unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from_cardinality, Cardinality::OneOrMore);
                assert_eq!(rel.to_cardinality, Cardinality::ZeroOrMore);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_connector_to() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("A || to || B").unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from_cardinality, Cardinality::ExactlyOne);
                assert_eq!(rel.to_cardinality, Cardinality::ExactlyOne);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_connector_optionally_to() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement(r#"A }o optionally to o{ B : "rel""#)
            .unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from_cardinality, Cardinality::ZeroOrMore);
                assert_eq!(rel.to_cardinality, Cardinality::ZeroOrMore);
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_relationship_dashed() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement(r#"PERSON }|..|{ CAR : "driver""#)
            .unwrap();

        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from_cardinality, Cardinality::OneOrMore);
                assert_eq!(rel.to_cardinality, Cardinality::OneOrMore);
                assert_eq!(rel.label.as_deref(), Some("driver"));
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_direction() {
        let parser = ChumskyErParser::new();
        assert!(matches!(
            parser.parse_statement("direction LR").unwrap(),
            Statement::Direction(Direction::Lr)
        ));
        assert!(matches!(
            parser.parse_statement("direction TB").unwrap(),
            Statement::Direction(Direction::Tb)
        ));
    }

    #[test]
    fn test_parse_standalone_entity() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("CUSTOMER").unwrap();
        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.name, "CUSTOMER");
                assert!(entity.attributes.is_empty());
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_attribute_star_prefix() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("User { int *Id }").unwrap();
        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.attributes[0].name, "Id");
                assert_eq!(entity.attributes[0].keys, vec![KeyKind::Pk]);
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_attribute_star_plus_explicit_key() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("User { int *Id FK }").unwrap();
        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.attributes[0].name, "Id");
                assert_eq!(entity.attributes[0].keys, vec![KeyKind::Pk, KeyKind::Fk]);
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_class_apply_on_entity() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("User:::highlight { int Id PK }")
            .unwrap();
        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.name, "User");
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_class_apply_on_relationship() {
        let parser = ChumskyErParser::new();
        let result = parser
            .parse_statement("A ||--o{ B:::highlight : rel")
            .unwrap();
        match result {
            Statement::Relationship(rel) => {
                assert_eq!(rel.from, "A");
                assert_eq!(rel.to, "B");
            }
            _ => panic!("Expected relationship statement"),
        }
    }

    #[test]
    fn test_parse_standalone_entity_with_class_apply() {
        let parser = ChumskyErParser::new();
        let result = parser.parse_statement("CUSTOMER:::foo").unwrap();
        match result {
            Statement::Entity(entity) => {
                assert_eq!(entity.name, "CUSTOMER");
            }
            _ => panic!("Expected entity statement"),
        }
    }

    #[test]
    fn test_parse_style_ignored() {
        let parser = ChumskyErParser::new();
        assert!(matches!(
            parser.parse_statement("style Person fill:#f9f").unwrap(),
            Statement::Style
        ));
        assert!(matches!(
            parser.parse_statement("classDef foo fill:#f9f").unwrap(),
            Statement::Style
        ));
    }

    #[test]
    fn test_parse_full_diagram() {
        let parser = ChumskyErParser::new();
        let input = r#"erDiagram
    PayGroup {
        int Id PK
        varchar Name
    }
    PayGroup ||--o{ PayGroupUserMapping : "has users""#;

        let result = parser.parse_diagram(input).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_diagram_with_comments() {
        let parser = ChumskyErParser::new();
        let input = r#"erDiagram
    %% a comment
    PayGroup {
        int Id PK
    }"#;

        let result = parser.parse_diagram(input).unwrap();
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], Statement::Comment));
    }
}
