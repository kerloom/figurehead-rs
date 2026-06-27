//! Pie chart database implementation
//!
//! Stores slices and display options for Mermaid pie charts.

use crate::core::Database;
use anyhow::{anyhow, Result};

/// A single slice in a pie chart.
#[derive(Debug, Clone, PartialEq)]
pub struct PieSlice {
    pub label: String,
    pub value: f64,
}

impl PieSlice {
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

/// Pie chart data.
#[derive(Debug, Default)]
pub struct PieDatabase {
    title: Option<String>,
    show_data: bool,
    slices: Vec<PieSlice>,
}

impl PieDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        self.title = (!title.is_empty()).then_some(title);
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn set_show_data(&mut self, show_data: bool) {
        self.show_data = show_data;
    }

    pub fn show_data(&self) -> bool {
        self.show_data
    }

    pub fn add_slice(&mut self, slice: PieSlice) -> Result<()> {
        if slice.label.trim().is_empty() {
            return Err(anyhow!("pie slice label cannot be empty"));
        }

        if !slice.value.is_finite() || slice.value < 0.0 {
            return Err(anyhow!("pie slice value must be a non-negative number"));
        }

        self.slices.push(slice);
        Ok(())
    }

    pub fn slices(&self) -> &[PieSlice] {
        &self.slices
    }

    pub fn slice_count(&self) -> usize {
        self.slices.len()
    }

    pub fn total(&self) -> f64 {
        self.slices.iter().map(|slice| slice.value).sum()
    }
}

impl Database for PieDatabase {
    type Node = PieSlice;
    type Edge = ();

    fn add_node(&mut self, node: Self::Node) -> Result<()> {
        self.add_slice(node)
    }

    fn add_edge(&mut self, _edge: Self::Edge) -> Result<()> {
        Ok(())
    }

    fn get_node(&self, id: &str) -> Option<&Self::Node> {
        self.slices.iter().find(|slice| slice.label == id)
    }

    fn nodes(&self) -> impl Iterator<Item = &Self::Node> {
        self.slices.iter()
    }

    fn edges(&self) -> impl Iterator<Item = &Self::Edge> {
        std::iter::empty()
    }

    fn clear(&mut self) {
        self.title = None;
        self.show_data = false;
        self.slices.clear();
    }

    fn node_count(&self) -> usize {
        self.slice_count()
    }

    fn edge_count(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_slice() {
        let mut db = PieDatabase::new();
        db.add_slice(PieSlice::new("Dogs", 386.0)).unwrap();

        assert_eq!(db.slice_count(), 1);
        assert_eq!(db.total(), 386.0);
        assert_eq!(db.get_node("Dogs").unwrap().value, 386.0);
    }

    #[test]
    fn test_rejects_invalid_slice() {
        let mut db = PieDatabase::new();

        assert!(db.add_slice(PieSlice::new("", 1.0)).is_err());
        assert!(db.add_slice(PieSlice::new("Bad", -1.0)).is_err());
    }
}
