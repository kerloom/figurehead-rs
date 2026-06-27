//! Pie chart ASCII renderer
//!
//! Renders pie charts as a symbolic circle plus legend.

use super::database::PieDatabase;
use crate::core::RenderConfig;
use anyhow::Result;
use std::f64::consts::PI;

/// ASCII renderer for pie charts.
pub struct PieRenderer {
    color: bool,
}

impl PieRenderer {
    pub fn new() -> Self {
        Self { color: true }
    }

    pub fn with_config(_config: RenderConfig) -> Self {
        Self::new()
    }

    pub fn with_color(color: bool) -> Self {
        Self { color }
    }

    fn symbol(index: usize) -> char {
        const SYMBOLS: &[u8] = b"123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
        SYMBOLS
            .get(index)
            .map(|symbol| *symbol as char)
            .unwrap_or('*')
    }

    fn format_value(value: f64) -> String {
        if value.fract() == 0.0 {
            format!("{}", value as i64)
        } else {
            format!("{:.2}", value)
        }
    }

    fn slice_for_ratio(cumulative: &[f64], ratio: f64) -> usize {
        cumulative
            .iter()
            .position(|limit| ratio <= *limit)
            .unwrap_or_else(|| cumulative.len().saturating_sub(1))
    }

    fn color_code(index: usize) -> u8 {
        const COLORS: &[u8] = &[31, 32, 33, 34, 35, 36, 91, 92, 93, 94, 95, 96];
        COLORS[index % COLORS.len()]
    }

    fn render_symbol(&self, index: usize) -> String {
        let symbol = Self::symbol(index);
        if self.color {
            format!("\x1b[{}m{}\x1b[0m", Self::color_code(index), symbol)
        } else {
            symbol.to_string()
        }
    }

    /// Render the database to ASCII.
    pub fn render(&self, database: &PieDatabase) -> Result<String> {
        let total = database.total();
        if database.slices().is_empty() || total <= 0.0 {
            return Ok(String::new());
        }

        let mut output = Vec::new();
        if let Some(title) = database.title() {
            output.push(title.to_string());
            output.push(String::new());
        }

        let mut cumulative = Vec::with_capacity(database.slice_count());
        let mut running = 0.0;
        for slice in database.slices() {
            running += slice.value / total;
            cumulative.push(running);
        }

        let radius_x = 10.0;
        let radius_y = 5.0;
        for y in -5..=5 {
            let mut row = String::new();
            for x in -10..=10 {
                let nx = x as f64 / radius_x;
                let ny = y as f64 / radius_y;

                if nx * nx + ny * ny > 1.0 {
                    row.push(' ');
                    continue;
                }

                let mut angle = ny.atan2(nx) + PI / 2.0;
                if angle < 0.0 {
                    angle += 2.0 * PI;
                }
                let ratio = angle / (2.0 * PI);
                let slice_index = Self::slice_for_ratio(&cumulative, ratio);
                row.push_str(&self.render_symbol(slice_index));
            }
            output.push(row.trim_end().to_string());
        }

        output.push(String::new());
        output.push("Legend".to_string());
        for (index, slice) in database.slices().iter().enumerate() {
            let percent = slice.value / total * 100.0;
            let mut line = format!(
                "{} {} ({:.1}%)",
                self.render_symbol(index),
                slice.label,
                percent
            );
            if database.show_data() {
                line.push_str(&format!(" - {}", Self::format_value(slice.value)));
            }
            output.push(line);
        }

        Ok(output.join("\n"))
    }
}

impl Default for PieRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::core::Renderer<PieDatabase> for PieRenderer {
    type Output = String;

    fn render(&self, database: &PieDatabase) -> Result<Self::Output> {
        self.render(database)
    }

    fn name(&self) -> &'static str {
        "ascii"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn format(&self) -> &'static str {
        "ascii"
    }
}

#[cfg(test)]
mod tests {
    use super::super::database::PieSlice;
    use super::*;

    #[test]
    fn test_render_pie() {
        let mut db = PieDatabase::new();
        db.set_title("Pets");
        db.set_show_data(true);
        db.add_slice(PieSlice::new("Dogs", 386.0)).unwrap();
        db.add_slice(PieSlice::new("Cats", 85.0)).unwrap();

        let renderer = PieRenderer::new();
        let output = renderer.render(&db).unwrap();

        assert!(output.contains("Pets"));
        assert!(output.contains("Dogs"));
        assert!(output.contains("Cats"));
        assert!(output.contains("386"));
        assert!(output.contains("\x1b[31m1\x1b[0m"));
        assert!(output.contains("\x1b[32m2\x1b[0m"));
    }

    #[test]
    fn test_render_colored_pie() {
        let mut db = PieDatabase::new();
        db.add_slice(PieSlice::new("Dogs", 386.0)).unwrap();
        db.add_slice(PieSlice::new("Cats", 85.0)).unwrap();

        let renderer = PieRenderer::with_color(true);
        let output = renderer.render(&db).unwrap();

        assert!(output.contains("\x1b[31m1\x1b[0m"));
        assert!(output.contains("\x1b[32m2\x1b[0m"));
    }

    #[test]
    fn test_render_empty_pie() {
        let db = PieDatabase::new();
        let renderer = PieRenderer::new();

        assert!(renderer.render(&db).unwrap().is_empty());
    }
}
