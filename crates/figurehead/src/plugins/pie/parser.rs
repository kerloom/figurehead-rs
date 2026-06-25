//! Pie chart parser
//!
//! Parses Mermaid pie syntax into a pie database.

use super::database::{PieDatabase, PieSlice};
use crate::core::Parser;
use anyhow::{anyhow, Result};

/// Parser for Mermaid pie charts.
pub struct PieParser;

impl PieParser {
    pub fn new() -> Self {
        Self
    }

    fn parse_header(&self, line: &str, database: &mut PieDatabase) {
        let line = line.trim();
        let lower = line.to_lowercase();
        if lower != "pie" && !lower.starts_with("pie ") {
            return;
        };

        let mut rest = line[3..].trim();
        if let Some(after_show_data) = rest.strip_prefix("showData") {
            database.set_show_data(true);
            rest = after_show_data.trim();
        }

        if let Some(title) = rest.strip_prefix("title") {
            database.set_title(title.trim());
        }
    }

    fn parse_title_line(&self, line: &str) -> Option<String> {
        line.trim()
            .strip_prefix("title")
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(ToOwned::to_owned)
    }

    fn parse_slice_line(&self, line: &str) -> Result<Option<PieSlice>> {
        let line = line.trim();

        if line.is_empty() || line.starts_with("%%") {
            return Ok(None);
        }

        let Some(colon_pos) = line.rfind(':') else {
            return Ok(None);
        };

        let label = line[..colon_pos].trim().trim_matches('"').trim();
        let value_text = line[colon_pos + 1..].trim();

        if label.is_empty() || value_text.is_empty() {
            return Ok(None);
        }

        let value = value_text
            .parse::<f64>()
            .map_err(|_| anyhow!("invalid pie slice value: {}", value_text))?;

        Ok(Some(PieSlice::new(label, value)))
    }
}

impl Default for PieParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser<PieDatabase> for PieParser {
    fn parse(&self, input: &str, database: &mut PieDatabase) -> Result<()> {
        let mut saw_header = false;

        for line in input.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with("%%") {
                continue;
            }

            let lower = line.to_lowercase();
            if lower == "pie" || lower.starts_with("pie ") {
                saw_header = true;
                self.parse_header(line, database);
                continue;
            }

            if let Some(title) = self.parse_title_line(line) {
                database.set_title(title);
                continue;
            }

            if let Some(slice) = self.parse_slice_line(line)? {
                database.add_slice(slice)?;
            }
        }

        if !saw_header {
            return Err(anyhow!("Parse error: expected pie diagram header"));
        }

        if database.slice_count() == 0 {
            return Err(anyhow!("Parse error: pie diagram contains no slices"));
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "pie"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn can_parse(&self, input: &str) -> bool {
        let lower = input.trim_start().to_lowercase();
        lower == "pie" || lower.starts_with("pie ") || lower.starts_with("pie\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pie() {
        let parser = PieParser::new();
        let mut db = PieDatabase::new();

        parser
            .parse(
                r#"pie
    title Pets adopted by volunteers
    "Dogs" : 386
    "Cats" : 85
    "Rats" : 15"#,
                &mut db,
            )
            .unwrap();

        assert_eq!(db.title(), Some("Pets adopted by volunteers"));
        assert_eq!(db.slice_count(), 3);
        assert_eq!(db.total(), 486.0);
        assert_eq!(db.slices()[0].label, "Dogs");
    }

    #[test]
    fn test_parse_header_options() {
        let parser = PieParser::new();
        let mut db = PieDatabase::new();

        parser
            .parse(
                r#"pie showData title Pets
    "Dogs" : 386"#,
                &mut db,
            )
            .unwrap();

        assert!(db.show_data());
        assert_eq!(db.title(), Some("Pets"));
    }

    #[test]
    fn test_parse_uppercase_header() {
        let parser = PieParser::new();
        let mut db = PieDatabase::new();

        parser.parse("PIE\n    \"Dogs\" : 386", &mut db).unwrap();

        assert_eq!(db.slice_count(), 1);
    }

    #[test]
    fn test_parse_invalid_value() {
        let parser = PieParser::new();
        let mut db = PieDatabase::new();

        let result = parser.parse("pie\n    \"Dogs\" : many", &mut db);

        assert!(result.is_err());
    }
}
