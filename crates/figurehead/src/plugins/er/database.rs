//! Entity-relationship diagram database
//!
//! Stores entities, attributes, and relationships for ER diagrams.

use crate::core::Database;
use anyhow::Result;

/// Key marker for an attribute (Primary Key, Foreign Key, Unique Key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Pk,
    Fk,
    Uk,
}

impl KeyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyKind::Pk => "PK",
            KeyKind::Fk => "FK",
            KeyKind::Uk => "UK",
        }
    }
}

/// ER participation cardinality (crow's-foot markers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    ExactlyOne,
    ZeroOrOne,
    OneOrMore,
    ZeroOrMore,
}

impl Cardinality {
    /// Parse a 2-character Mermaid ER marker into a cardinality.
    ///
    /// Accepts both canonical orientations, e.g. `}o` and `o{` both map to
    /// `ZeroOrMore`.
    pub fn from_marker(marker: &str) -> Option<Self> {
        match marker {
            "||" => Some(Cardinality::ExactlyOne),
            "|o" | "o|" => Some(Cardinality::ZeroOrOne),
            "}|" | "|{" => Some(Cardinality::OneOrMore),
            "}o" | "o{" => Some(Cardinality::ZeroOrMore),
            _ => None,
        }
    }

    /// Canonical 2-character marker for this cardinality.
    pub fn to_marker(self) -> &'static str {
        match self {
            Cardinality::ExactlyOne => "||",
            Cardinality::ZeroOrOne => "|o",
            Cardinality::OneOrMore => "}|",
            Cardinality::ZeroOrMore => "}o",
        }
    }
}

/// An attribute on an entity.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub attr_type: String,
    pub keys: Vec<KeyKind>,
    pub comment: Option<String>,
}

impl Attribute {
    pub fn new(name: impl Into<String>, attr_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attr_type: attr_type.into(),
            keys: Vec::new(),
            comment: None,
        }
    }

    pub fn with_key(mut self, key: KeyKind) -> Self {
        self.keys.push(key);
        self
    }

    pub fn with_keys(mut self, keys: impl IntoIterator<Item = KeyKind>) -> Self {
        self.keys.extend(keys);
        self
    }

    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }
}

/// An entity in the diagram.
#[derive(Debug, Clone)]
pub struct Entity {
    pub name: String,
    pub alias: Option<String>,
    pub attributes: Vec<Attribute>,
}

impl Entity {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            alias: None,
            attributes: Vec::new(),
        }
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }

    pub fn add_attribute(&mut self, attr: Attribute) {
        self.attributes.push(attr);
    }

    /// The display name: alias if set, otherwise the entity name.
    pub fn display_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}

/// A relationship between two entities.
#[derive(Debug, Clone)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub from_cardinality: Cardinality,
    pub to_cardinality: Cardinality,
    pub label: Option<String>,
}

impl Relationship {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        from_cardinality: Cardinality,
        to_cardinality: Cardinality,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            from_cardinality,
            to_cardinality,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// ER diagram database.
pub struct ErDatabase {
    entities: Vec<Entity>,
    relationships: Vec<Relationship>,
}

impl ErDatabase {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            relationships: Vec::new(),
        }
    }

    pub fn add_entity(&mut self, entity: Entity) -> Result<()> {
        self.entities.push(entity);
        Ok(())
    }

    pub fn add_relationship(&mut self, rel: Relationship) -> Result<()> {
        self.relationships.push(rel);
        Ok(())
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn relationship_count(&self) -> usize {
        self.relationships.len()
    }

    pub fn get_entity(&self, name: &str) -> Option<&Entity> {
        self.entities.iter().find(|e| e.name == name)
    }

    pub fn get_entity_mut(&mut self, name: &str) -> Option<&mut Entity> {
        self.entities.iter_mut().find(|e| e.name == name)
    }

    /// Get an entity by name, creating an empty one if it does not exist.
    ///
    /// This supports forward references in relationship lines that mention an
    /// entity before (or without) its block being defined.
    pub fn get_or_create_entity(&mut self, name: &str) -> &mut Entity {
        if self.get_entity(name).is_none() {
            self.entities.push(Entity::new(name));
        }
        self.get_entity_mut(name).unwrap()
    }
}

impl Default for ErDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl Database for ErDatabase {
    type Node = Entity;
    type Edge = Relationship;

    fn add_node(&mut self, node: Self::Node) -> Result<()> {
        self.add_entity(node)
    }

    fn add_edge(&mut self, edge: Self::Edge) -> Result<()> {
        self.add_relationship(edge)
    }

    fn get_node(&self, id: &str) -> Option<&Self::Node> {
        self.get_entity(id)
    }

    fn nodes(&self) -> impl Iterator<Item = &Self::Node> {
        self.entities.iter()
    }

    fn edges(&self) -> impl Iterator<Item = &Self::Edge> {
        self.relationships.iter()
    }

    fn clear(&mut self) {
        self.entities.clear();
        self.relationships.clear();
    }

    fn node_count(&self) -> usize {
        self.entities.len()
    }

    fn edge_count(&self) -> usize {
        self.relationships.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cardinality_from_marker() {
        assert_eq!(
            Cardinality::from_marker("||"),
            Some(Cardinality::ExactlyOne)
        );
        assert_eq!(Cardinality::from_marker("|o"), Some(Cardinality::ZeroOrOne));
        assert_eq!(Cardinality::from_marker("o|"), Some(Cardinality::ZeroOrOne));
        assert_eq!(Cardinality::from_marker("}|"), Some(Cardinality::OneOrMore));
        assert_eq!(Cardinality::from_marker("|{"), Some(Cardinality::OneOrMore));
        assert_eq!(
            Cardinality::from_marker("}o"),
            Some(Cardinality::ZeroOrMore)
        );
        assert_eq!(
            Cardinality::from_marker("o{"),
            Some(Cardinality::ZeroOrMore)
        );
        assert_eq!(Cardinality::from_marker("xx"), None);
    }

    #[test]
    fn test_cardinality_to_marker_canonical() {
        assert_eq!(Cardinality::ExactlyOne.to_marker(), "||");
        assert_eq!(Cardinality::ZeroOrOne.to_marker(), "|o");
        assert_eq!(Cardinality::OneOrMore.to_marker(), "}|");
        assert_eq!(Cardinality::ZeroOrMore.to_marker(), "}o");
    }

    #[test]
    fn test_create_entity() {
        let entity = Entity::new("PayGroup");
        assert_eq!(entity.name, "PayGroup");
        assert!(entity.attributes.is_empty());
    }

    #[test]
    fn test_add_attributes() {
        let mut entity = Entity::new("PayGroup");
        entity.add_attribute(Attribute::new("Id", "int").with_key(KeyKind::Pk));
        entity.add_attribute(Attribute::new("Name", "varchar").with_comment("display name"));

        assert_eq!(entity.attributes.len(), 2);
        assert_eq!(entity.attributes[0].keys, vec![KeyKind::Pk]);
        assert_eq!(
            entity.attributes[1].comment,
            Some("display name".to_string())
        );
    }

    #[test]
    fn test_multiple_keys() {
        let attr = Attribute::new("Id", "int")
            .with_key(KeyKind::Pk)
            .with_key(KeyKind::Fk);
        assert_eq!(attr.keys, vec![KeyKind::Pk, KeyKind::Fk]);
    }

    #[test]
    fn test_entity_alias() {
        let entity = Entity::new("PERSON").with_alias("Customer");
        assert_eq!(entity.name, "PERSON");
        assert_eq!(entity.display_name(), "Customer");
    }

    #[test]
    fn test_entity_no_alias_uses_name() {
        let entity = Entity::new("PayGroup");
        assert_eq!(entity.display_name(), "PayGroup");
    }

    #[test]
    fn test_database_add_entity() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();
        db.add_entity(Entity::new("B")).unwrap();

        assert_eq!(db.entity_count(), 2);
        assert!(db.get_entity("A").is_some());
        assert!(db.get_entity("C").is_none());
    }

    #[test]
    fn test_database_add_relationship() {
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

        assert_eq!(db.relationship_count(), 1);
    }

    #[test]
    fn test_get_or_create_entity() {
        let mut db = ErDatabase::new();
        db.get_or_create_entity("A");
        assert_eq!(db.entity_count(), 1);
        db.get_or_create_entity("A");
        assert_eq!(db.entity_count(), 1);
        db.get_or_create_entity("B");
        assert_eq!(db.entity_count(), 2);
    }

    #[test]
    fn test_database_trait_nodes() {
        let mut db = ErDatabase::new();
        db.add_entity(Entity::new("A")).unwrap();

        let nodes: Vec<_> = db.nodes().collect();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "A");
    }
}
