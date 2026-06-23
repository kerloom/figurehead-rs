//! Integration tests for the ER diagram plugin.

use figurehead::render;

const PAYMENTS_FIXTURE: &str = r#"erDiagram
    PayGroup {
        int Id PK
        varchar Name
        varchar CountryCode
        varchar CurrencyCode
        int PayFrequency
    }

    PayGroupUserMapping {
        int Id PK
        int PayGroupId FK
        varchar UserId
    }

    PayGroupEmployeeMapping {
        int Id PK
        int PayGroupId FK
        varchar EmployeeId
    }

    PayGroup ||--o{ PayGroupUserMapping : "has users"
    PayGroup ||--o{ PayGroupEmployeeMapping : "has employees""#;

#[test]
fn test_render_payments_fixture() {
    let result = render(PAYMENTS_FIXTURE);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("PayGroup"));
    assert!(output.contains("PayGroupUserMapping"));
    assert!(output.contains("PayGroupEmployeeMapping"));
    assert!(output.contains("||"));
    assert!(output.contains("o{"));
    assert!(output.contains("has users"));
    assert!(output.contains("has employees"));
}

#[test]
fn test_render_entity_with_comment() {
    let input = r#"erDiagram
    Document {
        int Id PK
        varchar FileUUID "DMS document reference"
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("Document"));
    assert!(output.contains("int Id PK"));
    assert!(output.contains("DMS document reference"));
}

#[test]
fn test_render_all_cardinality_markers() {
    let input = r#"erDiagram
    A ||--|| B : "one to one"
    C }o--o{ D : "many to many"
    E |o--o| F : "zero or one""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("||"));
    assert!(output.contains("}o"));
    assert!(output.contains("|o"));
}

#[test]
fn test_render_relationship_no_label() {
    let input = r#"erDiagram
    CustomerSubscription }o--|| SubscriptionType"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("CustomerSubscription"));
    assert!(output.contains("SubscriptionType"));
    assert!(output.contains("}o"));
    assert!(output.contains("||"));
}

#[test]
fn test_render_dashed_connector() {
    let input = r#"erDiagram
    A }o..o{ B : "non-identifying""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
}

#[test]
fn test_render_comments_ignored() {
    let input = r#"erDiagram
    %% this is a comment
    A {
        int Id PK
    }
    %% another comment
    A ||--o{ B : "rel""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("A"));
    assert!(output.contains("o{"));
}

#[test]
fn test_render_forward_reference() {
    let input = r#"erDiagram
    A ||--o{ B : "refers to B before def"
    B {
        int Id PK
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("A"));
    assert!(output.contains("B"));
}

#[test]
fn test_render_entity_alias() {
    let input = r#"erDiagram
    PERSON[Customer] {
        int Id PK
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("Customer"));
}

#[test]
fn test_render_attribute_star_prefix() {
    let input = r#"erDiagram
    User {
        int *Id
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("int Id PK"));
}

#[test]
fn test_render_uk_key() {
    let input = r#"erDiagram
    User {
        varchar Email UK
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("varchar Email UK"));
}

#[test]
fn test_render_multiple_keys() {
    let input = r#"erDiagram
    User {
        int Id PK, FK
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("int Id PK FK"));
}

#[test]
fn test_render_type_with_parens() {
    let input = r#"erDiagram
    User {
        varchar(255) Name
        decimal(10,2) Salary
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("varchar(255) Name"));
    assert!(output.contains("decimal(10,2) Salary"));
}

#[test]
fn test_render_quoted_entity_name() {
    let input = r#"erDiagram
    "name with space" {
        int Id PK
    }"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("name with space"));
}

#[test]
fn test_render_direction_ignored() {
    let input = r#"erDiagram
    direction LR
    A {
        int Id PK
    }
    A ||--o{ B : "rel""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("A"));
    assert!(output.contains("o{"));
}

#[test]
fn test_render_style_ignored() {
    let input = r#"erDiagram
    A {
        int Id PK
    }
    style A fill:#f9f
    classDef foo fill:#f9f
    class A foo"#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("A"));
}

#[test]
fn test_render_cardinality_words() {
    let input = r#"erDiagram
    A one or more -- zero or more B : "rel""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("}|"));
    assert!(output.contains("o{"));
}

#[test]
fn test_render_connector_to() {
    let input = r#"erDiagram
    A || to || B : "rel""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("||"));
}

#[test]
fn test_render_connector_optionally_to() {
    let input = r#"erDiagram
    A }o optionally to o{ B : "rel""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
}

#[test]
fn test_render_standalone_entity() {
    let input = r#"erDiagram
    CUSTOMER
    ORDER {
        int Id PK
    }
    CUSTOMER ||--o{ ORDER : "places""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("CUSTOMER"));
    assert!(output.contains("ORDER"));
}

#[test]
fn test_render_class_apply_ignored() {
    let input = r#"erDiagram
    A:::highlight {
        int Id PK
    }
    A ||--o{ B:::highlight : "rel""#;
    let result = render(input);
    assert!(result.is_ok(), "render failed: {:?}", result.err());
    let output = result.unwrap();
    assert!(output.contains("A"));
    assert!(output.contains("o{"));
}
