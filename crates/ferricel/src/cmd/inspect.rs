//! Implementation of the `ferricel inspect` subcommand.
//!
//! Reads all metadata embedded in a compiled ferricel Wasm module and prints
//! it in a human-readable form with optional syntax highlighting, or as JSON
//! with `--json`.

use std::{io::IsTerminal, path::Path};

use anyhow::Context;
use ferricel_core::{ModuleInfo, inspect};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
    util::{LinesWithEndings, as_24_bit_terminal_escaped},
};

// ─── CEL syntax asset ────────────────────────────────────────────────────────

const CEL_SYNTAX: &str = include_str!("inspect/cel.sublime-syntax");

// ─── Entry point ─────────────────────────────────────────────────────────────

pub fn run(wasm_path: &Path, no_color: bool, json: bool) -> Result<(), anyhow::Error> {
    if !wasm_path.exists() {
        anyhow::bail!("Wasm file not found at {}", wasm_path.display());
    }

    let wasm = std::fs::read(wasm_path)
        .with_context(|| format!("Failed to read {}", wasm_path.display()))?;

    let info =
        inspect(&wasm).with_context(|| format!("Failed to inspect {}", wasm_path.display()))?;

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    let use_color = should_color(no_color);
    let hl = if use_color {
        Some(Highlighter::new()?)
    } else {
        None
    };

    print_human(wasm_path, &info, hl.as_ref());
    Ok(())
}

// ─── Human output ────────────────────────────────────────────────────────────

fn print_human(path: &Path, info: &ModuleInfo, hl: Option<&Highlighter>) {
    println!("Module: {}", path.display());

    // Source
    if let Some(src) = &info.cel_source {
        println!("\nSource (CEL):");
        print_indented(src, "cel", hl);
    } else if let Some(src) = &info.vap_source {
        println!("\nSource (ValidatingAdmissionPolicy):");
        print_indented(src, "yaml", hl);
    }

    // Host extensions
    println!("\nHost extensions (may be called):");
    if info.extensions.is_empty() {
        println!("  (none)");
    } else {
        for ext in &info.extensions {
            match &ext.namespace {
                Some(ns) => println!("  - {ns}/{}", ext.function),
                None => println!("  - {}", ext.function),
            }
        }
    }

    // Exports
    if !info.exports.is_empty() {
        println!("\nExports: {}", info.exports.join(", "));
    }

    // Producers
    if !info.producers.is_empty() {
        println!("\nProducers:");
        for field in &info.producers {
            let values: Vec<String> = field
                .values
                .iter()
                .map(|v| {
                    if v.version.is_empty() {
                        v.name.clone()
                    } else {
                        format!("{} {}", v.name, v.version)
                    }
                })
                .collect();
            println!("  {}: {}", field.name, values.join(", "));
        }
    }
}

fn print_indented(source: &str, lang: &str, hl: Option<&Highlighter>) {
    match hl.and_then(|h| h.highlight(source, lang).ok()) {
        Some(highlighted) => {
            for line in highlighted.lines() {
                println!("  {line}");
            }
        }
        None => {
            for line in source.lines() {
                println!("  {line}");
            }
        }
    }
}

// ─── Color resolution ────────────────────────────────────────────────────────

fn should_color(no_color: bool) -> bool {
    if no_color {
        return false;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    std::io::stdout().is_terminal()
}

// ─── Theme selection ─────────────────────────────────────────────────────────

fn pick_theme() -> &'static str {
    let is_light = terminal_light::luma().map(|l| l > 0.6).unwrap_or(false);
    if is_light {
        "Solarized (light)"
    } else {
        "Solarized (dark)"
    }
}

// ─── Highlighter ─────────────────────────────────────────────────────────────

struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: String,
}

impl Highlighter {
    fn new() -> Result<Self, anyhow::Error> {
        // Build a SyntaxSet with syntect's defaults (includes YAML) + our CEL grammar.
        let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
        let cel_def = SyntaxDefinition::load_from_str(CEL_SYNTAX, true, None)
            .context("Failed to load CEL syntax definition")?;
        builder.add(cel_def);
        let syntax_set = builder.build();

        let theme_set = ThemeSet::load_defaults();
        let theme_name = pick_theme().to_string();

        Ok(Self {
            syntax_set,
            theme_set,
            theme_name,
        })
    }

    /// Highlight `source` using the syntax matched by `lang_hint`
    /// (`"cel"` or `"yaml"`). Returns highlighted lines joined with newlines.
    fn highlight(&self, source: &str, lang_hint: &str) -> Result<String, anyhow::Error> {
        let syntax = self
            .syntax_set
            .find_syntax_by_name(lang_hint)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang_hint))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = self
            .theme_set
            .themes
            .get(&self.theme_name)
            .context("Theme not found")?;

        let mut hl = HighlightLines::new(syntax, theme);
        let mut out = String::new();

        for line in LinesWithEndings::from(source) {
            let ranges: Vec<(Style, &str)> = hl
                .highlight_line(line, &self.syntax_set)
                .context("Highlight error")?;
            out.push_str(&as_24_bit_terminal_escaped(&ranges, false));
        }

        // Reset terminal colors at the end.
        out.push_str("\x1b[0m");

        Ok(out)
    }
}
