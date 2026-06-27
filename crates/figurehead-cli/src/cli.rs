//! Command-line interface for the figurehead utility
//!
//! Provides a CLI to convert Mermaid.js diagram markup into ASCII diagrams.

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use crate::colorizer::{colorize_output, extract_styles, StyleInfo};
use figurehead::core::logging::init_logging;
use figurehead::plugins::Orchestrator;
use figurehead::{CharacterSet, DiamondStyle, RenderConfig};

/// Figurehead - Convert Mermaid.js diagrams to ASCII art
#[derive(Parser)]
#[command(name = "figurehead")]
#[command(about = "A Rust utility to convert Mermaid.js diagrams to ASCII diagrams")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(author = env!("CARGO_PKG_AUTHORS"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Set log level (trace|debug|info|warn|error)
    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    pub log_level: LogLevel,

    /// Set log format (compact|pretty|json)
    #[arg(long, value_enum, default_value_t = LogFormat::Compact)]
    pub log_format: LogFormat,
}

/// Log level options
#[derive(Copy, Clone, Debug, clap::ValueEnum, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "trace",
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

/// Log format options
#[derive(Copy, Clone, Debug, clap::ValueEnum, PartialEq, Eq)]
pub enum LogFormat {
    Compact,
    Pretty,
    Json,
}

impl LogFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogFormat::Compact => "compact",
            LogFormat::Pretty => "pretty",
            LogFormat::Json => "json",
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Convert a Mermaid.js diagram to ASCII
    Convert {
        /// Input file containing Mermaid.js diagram (use - for stdin)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output file for ASCII diagram (use - for stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Skip diagram type detection (treat as flowchart)
        #[arg(long)]
        skip_detection: bool,

        /// Character set to use for rendering output
        #[arg(
            long,
            value_enum,
            default_value_t = StyleChoice::Unicode
        )]
        style: StyleChoice,

        /// Diamond (decision) node style
        #[arg(
            long,
            value_enum,
            default_value_t = DiamondChoice::Box
        )]
        diamond: DiamondChoice,

        /// When to use colors in output
        #[arg(
            long,
            value_enum,
            default_value_t = ColorChoice::Auto
        )]
        color: ColorChoice,
    },

    /// Detect diagram type in input
    Detect {
        /// Input file to analyze (use - for stdin)
        #[arg(short, long)]
        input: Option<PathBuf>,
    },

    /// Show supported diagram types
    Types {
        /// Show in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Validate Mermaid.js syntax
    Validate {
        /// Input file to validate (use - for stdin)
        #[arg(short, long)]
        input: Option<PathBuf>,
    },
}

/// Supported output character sets
#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq)]
pub enum StyleChoice {
    Ascii,
    Unicode,
    UnicodeMath,
    Compact,
}

impl From<StyleChoice> for CharacterSet {
    fn from(value: StyleChoice) -> Self {
        match value {
            StyleChoice::Ascii => CharacterSet::Ascii,
            StyleChoice::Unicode => CharacterSet::Unicode,
            StyleChoice::UnicodeMath => CharacterSet::UnicodeMath,
            StyleChoice::Compact => CharacterSet::Compact,
        }
    }
}

/// Diamond (decision node) rendering styles
#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq, Default)]
pub enum DiamondChoice {
    /// Compact 3-line box with ◆ corners
    #[default]
    Box,
    /// Minimal single-line ◆ text ◆
    Inline,
    /// Traditional tall diamond with /\ and \/
    Tall,
}

impl From<DiamondChoice> for DiamondStyle {
    fn from(value: DiamondChoice) -> Self {
        match value {
            DiamondChoice::Box => DiamondStyle::Box,
            DiamondChoice::Inline => DiamondStyle::Inline,
            DiamondChoice::Tall => DiamondStyle::Tall,
        }
    }
}

/// When to colorize output
#[derive(Copy, Clone, Debug, ValueEnum, PartialEq, Eq, Default)]
pub enum ColorChoice {
    /// Use colors if output is a terminal and NO_COLOR is not set
    #[default]
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

/// Main CLI application
pub struct FigureheadApp {
    orchestrator: Orchestrator,
}

impl FigureheadApp {
    /// Create a new application instance with default settings
    pub fn new() -> Self {
        Self::with_config(RenderConfig::default())
    }

    /// Create a new application instance with a render config
    pub fn with_config(config: RenderConfig) -> Self {
        let mut orchestrator = Orchestrator::all_plugins(config);
        orchestrator.register_default_detectors();
        Self { orchestrator }
    }

    fn build_config(style: StyleChoice, diamond: DiamondChoice) -> RenderConfig {
        RenderConfig::new(style.into(), diamond.into())
    }

    /// Run the application with the given CLI arguments
    pub fn run(&mut self, cli: Cli) -> Result<()> {
        // Initialize logging with CLI flags (environment variables take precedence)
        let log_level_str = std::env::var("FIGUREHEAD_LOG_LEVEL")
            .ok()
            .or_else(|| std::env::var("RUST_LOG").ok())
            .or_else(|| Some(cli.log_level.as_str().to_string()));

        let log_format_str = std::env::var("FIGUREHEAD_LOG_FORMAT")
            .ok()
            .or_else(|| Some(cli.log_format.as_str().to_string()));

        // Reinitialize logging with CLI/environment settings
        if let Err(e) = init_logging(log_level_str.as_deref(), log_format_str.as_deref()) {
            eprintln!("Warning: Failed to initialize logging: {}", e);
        }

        if cli.verbose {
            eprintln!("Figurehead v{}", env!("CARGO_PKG_VERSION"));
        }

        match cli.command {
            Commands::Convert {
                input,
                output,
                skip_detection,
                style,
                diamond,
                color,
            } => self.convert_command(
                input,
                output,
                skip_detection,
                style,
                diamond,
                color,
                cli.verbose,
            ),
            Commands::Detect { input } => self.detect_command(input, cli.verbose),
            Commands::Types { json } => self.types_command(json, cli.verbose),
            Commands::Validate { input } => self.validate_command(input, cli.verbose),
        }
    }

    /// Handle the convert command
    #[allow(clippy::too_many_arguments)]
    fn convert_command(
        &mut self,
        input: Option<PathBuf>,
        output: Option<PathBuf>,
        skip_detection: bool,
        style: StyleChoice,
        diamond: DiamondChoice,
        color: ColorChoice,
        verbose: bool,
    ) -> Result<()> {
        // Read input
        let content = self.read_input(input)?;

        if verbose {
            eprintln!("Read {} bytes of input", content.len());
        }

        let should_colorize = self.should_colorize(&output, color);

        // Apply style, diamond, and color options to renderer
        let config = Self::build_config(style, diamond).with_color(should_colorize);
        let mut orchestrator = Orchestrator::all_plugins(config);
        orchestrator.register_default_detectors();
        self.orchestrator = orchestrator;

        // Process the diagram
        // For flowcharts, we can get the database for proper style extraction
        let (ascii_output, styles) = if skip_detection {
            // Direct flowchart processing - use database for styles
            let (output, db) = self
                .orchestrator
                .process_flowchart_with_database(&content)?;
            let styles = if should_colorize {
                StyleInfo::from_database(&db)
            } else {
                StyleInfo::default()
            };
            (output, styles)
        } else {
            // Auto-detection - fall back to text-based style extraction
            let output = self.orchestrator.process(&content)?;
            let styles = if should_colorize {
                extract_styles(&content)
            } else {
                StyleInfo::default()
            };
            (output, styles)
        };

        if verbose {
            eprintln!("Successfully converted diagram to ASCII");
        }

        // Apply colors if enabled and styles are present
        let final_output = if should_colorize {
            colorize_output(&content, &ascii_output, &styles)
        } else {
            ascii_output
        };
        self.write_output(output, &final_output)?;
        Ok(())
    }

    /// Determine if we should colorize the output based on color choice and output destination
    fn should_colorize(&self, output: &Option<PathBuf>, color: ColorChoice) -> bool {
        match color {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                // Check NO_COLOR environment variable
                if std::env::var("NO_COLOR").is_ok() {
                    return false;
                }
                // Only colorize if outputting to stdout and it's a terminal
                match output {
                    None => crossterm::tty::IsTty::is_tty(&std::io::stdout()),
                    Some(ref p) if p.to_str() == Some("-") => {
                        crossterm::tty::IsTty::is_tty(&std::io::stdout())
                    }
                    Some(_) => false, // Writing to file, no colors
                }
            }
        }
    }

    /// Handle the detect command
    fn detect_command(&self, input: Option<PathBuf>, verbose: bool) -> Result<()> {
        let content = self.read_input(input)?;

        if verbose {
            eprintln!("Read {} bytes of input", content.len());
        }

        match self.orchestrator.detect_diagram_type(&content) {
            Ok(diagram_type) => {
                println!("{}", diagram_type);
                Ok(())
            }
            Err(e) => {
                eprintln!("Could not detect diagram type: {}", e);
                Err(e)
            }
        }
    }

    /// Handle the types command
    fn types_command(&self, json: bool, verbose: bool) -> Result<()> {
        if verbose {
            eprintln!("Listing supported diagram types");
        }

        if json {
            // JSON output
            let types = serde_json::json!({
                "supported_types": [
                    {
                        "name": "flowchart",
                        "description": "Flowchart diagrams with nodes and edges",
                        "status": "supported"
                    }
                ],
                "total": 1
            });
            println!("{}", serde_json::to_string_pretty(&types)?);
        } else {
            // Human-readable output
            println!("Supported diagram types:");
            println!("  flowchart  - Flowchart diagrams with nodes and edges");
            println!();
            println!("Total: 1 diagram type supported");
        }

        Ok(())
    }

    /// Handle the validate command
    fn validate_command(&self, input: Option<PathBuf>, verbose: bool) -> Result<()> {
        let content = self.read_input(input)?;

        if verbose {
            eprintln!("Read {} bytes of input", content.len());
        }

        // Try to detect diagram type first
        let detection_result = self.orchestrator.detect_diagram_type(&content);

        match detection_result {
            Ok(diagram_type) => {
                if verbose {
                    eprintln!("Detected diagram type: {}", diagram_type);
                }

                // Try to process it
                match self.orchestrator.process(&content) {
                    Ok(_) => {
                        println!("✓ Valid {} diagram", diagram_type);
                        Ok(())
                    }
                    Err(e) => {
                        println!("✗ Invalid {} diagram: {}", diagram_type, e);
                        Err(e)
                    }
                }
            }
            Err(_) => {
                println!("✗ Could not detect diagram type");
                Err(anyhow!("Unknown diagram type"))
            }
        }
    }

    /// Read input from file or stdin
    pub fn read_input(&self, input: Option<PathBuf>) -> Result<String> {
        match input {
            Some(path) => {
                if path.to_string_lossy() == "-" {
                    // Read from stdin
                    let mut content = String::new();
                    io::stdin().read_to_string(&mut content)?;
                    Ok(content)
                } else {
                    // Read from file
                    fs::read_to_string(&path).map_err(|e| {
                        anyhow!("Failed to read input file '{}': {}", path.display(), e)
                    })
                }
            }
            None => {
                // No input file specified, read from stdin
                let mut content = String::new();
                io::stdin().read_to_string(&mut content)?;
                Ok(content)
            }
        }
    }

    /// Write output to file or stdout
    pub fn write_output(&self, output: Option<PathBuf>, content: &str) -> Result<()> {
        let stdout_content = if content.is_empty() || content.ends_with('\n') {
            content.to_string()
        } else {
            format!("{}\n", content)
        };

        match output {
            Some(path) => {
                if path.to_string_lossy() == "-" {
                    // Write to stdout
                    print!("{}", stdout_content);
                    io::stdout().flush()?;
                } else {
                    // Write to file
                    fs::write(&path, content).map_err(|e| {
                        anyhow!("Failed to write output file '{}': {}", path.display(), e)
                    })?;
                }
            }
            None => {
                // No output file specified, write to stdout
                print!("{}", stdout_content);
                io::stdout().flush()?;
            }
        }
        Ok(())
    }

    /// Get a reference to the orchestrator (for testing)
    #[cfg(test)]
    pub fn orchestrator(&self) -> &Orchestrator {
        &self.orchestrator
    }

    /// Get a mutable reference to the orchestrator (for testing)
    #[cfg(test)]
    pub fn orchestrator_mut(&mut self) -> &mut Orchestrator {
        &mut self.orchestrator
    }
}

impl Default for FigureheadApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use figurehead::plugins::flowchart::FlowchartDetector;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_cli_parsing_convert_command() {
        let args = vec![
            "figurehead",
            "convert",
            "--input",
            "test.mmd",
            "--output",
            "output.txt",
            "--style",
            "ascii",
        ];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Convert {
                input,
                output,
                skip_detection,
                style,
                diamond,
                color,
            } => {
                assert_eq!(input.unwrap().to_string_lossy(), "test.mmd");
                assert_eq!(output.unwrap().to_string_lossy(), "output.txt");
                assert!(!skip_detection);
                assert_eq!(style, StyleChoice::Ascii);
                assert_eq!(diamond, DiamondChoice::Box); // default
                assert_eq!(color, ColorChoice::Auto); // default
            }
            _ => panic!("Expected Convert command"),
        }
    }

    #[test]
    fn test_cli_parsing_diamond_option() {
        let args = vec!["figurehead", "convert", "--diamond", "tall"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Convert { diamond, .. } => {
                assert_eq!(diamond, DiamondChoice::Tall);
            }
            _ => panic!("Expected Convert command"),
        }
    }

    #[test]
    fn test_cli_parsing_detect_command() {
        let args = vec!["figurehead", "detect", "--input", "test.mmd"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Detect { input } => {
                assert_eq!(input.unwrap().to_string_lossy(), "test.mmd");
            }
            _ => panic!("Expected Detect command"),
        }
    }

    #[test]
    fn test_cli_parsing_types_command() {
        let args = vec!["figurehead", "types", "--json"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Types { json } => {
                assert!(json);
            }
            _ => panic!("Expected Types command"),
        }
    }

    #[test]
    fn test_cli_parsing_validate_command() {
        let args = vec!["figurehead", "validate"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Validate { input } => {
                assert!(input.is_none());
            }
            _ => panic!("Expected Validate command"),
        }
    }

    #[test]
    fn test_figurehead_app_creation() {
        // Verify the app can be created without panicking
        let _app = FigureheadApp::new();
    }

    #[test]
    fn test_figurehead_app_default() {
        // Verify the app can be created via Default without panicking
        let _app = FigureheadApp::default();
    }

    #[test]
    fn test_read_input_from_string() {
        let app = FigureheadApp::new();
        let input = "graph TD; A-->B;";

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.mmd");
        fs::write(&file_path, input).unwrap();

        let content = app.read_input(Some(file_path)).unwrap();
        assert_eq!(content, input);
    }

    #[test]
    fn test_write_output_to_string() {
        let app = FigureheadApp::new();
        let output = "Test output";

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("output.txt");

        app.write_output(Some(file_path.clone()), output).unwrap();

        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, output);
    }

    #[test]
    fn test_detect_command_with_flowchart() {
        let mut app = FigureheadApp::new();
        app.orchestrator_mut()
            .register_detector("flowchart".to_string(), Box::new(FlowchartDetector::new()));

        let input = "graph TD; A-->B;";
        let result = app.orchestrator().detect_diagram_type(input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "flowchart");
    }

    #[test]
    fn test_detect_command_with_non_flowchart() {
        let app = FigureheadApp::new();
        let input = "This is not a diagram";

        let result = app.orchestrator().detect_diagram_type(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_command_simple_flowchart() {
        let app = FigureheadApp::new();
        let input = "graph TD; A-->B;";

        let result = app.orchestrator().process_flowchart(input);
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_validate_command_valid_flowchart() {
        let mut app = FigureheadApp::new();
        app.orchestrator_mut()
            .register_detector("flowchart".to_string(), Box::new(FlowchartDetector::new()));

        let input = "graph TD; A-->B;";

        let detection_result = app.orchestrator().detect_diagram_type(input);
        assert!(detection_result.is_ok());

        let process_result = app.orchestrator().process(input);
        assert!(process_result.is_ok());
    }

    #[test]
    fn test_validate_command_invalid_diagram() {
        let app = FigureheadApp::new();
        let input = "This is not a diagram";

        let detection_result = app.orchestrator().detect_diagram_type(input);
        assert!(detection_result.is_err());
    }

    #[test]
    fn test_types_command_json_format() {
        let app = FigureheadApp::new();
        let result = app.types_command(true, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_types_command_human_format() {
        let app = FigureheadApp::new();
        let result = app.types_command(false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_skip_detection_flag() {
        let args = vec!["figurehead", "convert", "--skip-detection"];
        let cli = Cli::try_parse_from(args).unwrap();

        match cli.command {
            Commands::Convert { skip_detection, .. } => {
                assert!(skip_detection);
            }
            _ => panic!("Expected Convert command"),
        }
    }

    #[test]
    fn test_verbose_flag() {
        let args = vec!["figurehead", "--verbose", "convert"];
        let cli = Cli::try_parse_from(args).unwrap();

        assert!(cli.verbose);
    }
}
