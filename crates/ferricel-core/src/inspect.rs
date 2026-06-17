//! Inspection of compiled ferricel Wasm modules.
//!
//! Reads all metadata embedded by the ferricel compiler into a [`ModuleInfo`]
//! struct, including the source custom sections, the host-extension manifest,
//! the standard `producers` section, and the module's exported functions.
//!
//! Use [`inspect`] to parse a Wasm binary.
//!
//! See the [Wasm Spec](https://flavio.github.io/ferricel/wasm-spec.html)
//! chapter of the user guide for details on each section.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use wasmparser::{ExternalKind, Parser, Payload};

use crate::UsedExtension;

// ─── Public types ─────────────────────────────────────────────────────────────

/// One value entry inside a `producers` field (e.g. `rustc: 1.95.0`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProducerValue {
    /// Tool or language name (e.g. `"rustc"`, `"CEL"`).
    pub name: String,
    /// Version string, may be empty.
    pub version: String,
}

/// One field in the standard WebAssembly `producers` custom section.
///
/// Common field names: `"language"`, `"processed-by"`, `"sdk"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProducerField {
    /// Field name, e.g. `"language"` or `"processed-by"`.
    pub name: String,
    /// Ordered list of values for this field.
    pub values: Vec<ProducerValue>,
}

/// All metadata embedded in a compiled ferricel Wasm module.
///
/// Returned by [`inspect`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// Original CEL expression, from the `ferricel.cel-source` section.
    /// `None` for VAP modules.
    pub cel_source: Option<String>,

    /// Original `ValidatingAdmissionPolicy` YAML, from the `ferricel.vap-source`
    /// section. `None` for plain CEL modules.
    pub vap_source: Option<String>,

    /// Host extensions the module may call, from the `ferricel.extensions`
    /// section. Empty if the module uses no extensions.
    pub extensions: Vec<UsedExtension>,

    /// Entries from the standard WebAssembly `producers` section.
    pub producers: Vec<ProducerField>,

    /// Names of all exported functions (e.g. `["evaluate", "cel_malloc"]`).
    pub exports: Vec<String>,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Parse a compiled ferricel Wasm module and return its embedded metadata.
///
/// Returns an error if `wasm` is not a valid WebAssembly binary.
///
/// # Example
///
/// ```no_run
/// use ferricel_core::inspect;
///
/// let wasm = std::fs::read("policy.wasm").unwrap();
/// let info = inspect(&wasm).unwrap();
/// if let Some(src) = &info.cel_source {
///     println!("CEL: {}", src);
/// }
/// for ext in &info.extensions {
///     println!(
///         "extension: {}/{}",
///         ext.namespace.as_deref().unwrap_or("(none)"),
///         ext.function
///     );
/// }
/// ```
pub fn inspect(wasm: &[u8]) -> Result<ModuleInfo, anyhow::Error> {
    let mut cel_source: Option<String> = None;
    let mut vap_source: Option<String> = None;
    let mut extensions: Vec<UsedExtension> = Vec::new();
    let mut producers: Vec<ProducerField> = Vec::new();
    let mut exports: Vec<String> = Vec::new();

    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload.context("Failed to parse Wasm module")?;
        match payload {
            Payload::CustomSection(reader) => {
                let name = reader.name();
                let data = reader.data();
                match name {
                    "ferricel.cel-source" => {
                        cel_source = Some(
                            String::from_utf8(data.to_vec())
                                .context("ferricel.cel-source is not valid UTF-8")?,
                        );
                    }
                    "ferricel.vap-source" => {
                        vap_source = Some(
                            String::from_utf8(data.to_vec())
                                .context("ferricel.vap-source is not valid UTF-8")?,
                        );
                    }
                    "ferricel.extensions" => {
                        extensions = serde_json::from_slice(data)
                            .context("Failed to deserialize ferricel.extensions")?;
                    }
                    "producers" => {
                        producers = parse_producers(data)?;
                    }
                    _ => {}
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.context("Failed to read export")?;
                    if export.kind == ExternalKind::Func {
                        exports.push(export.name.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    exports.sort();

    Ok(ModuleInfo {
        cel_source,
        vap_source,
        extensions,
        producers,
        exports,
    })
}

// ─── Producers section parser ─────────────────────────────────────────────────

fn parse_producers(data: &[u8]) -> Result<Vec<ProducerField>, anyhow::Error> {
    use wasmparser::{BinaryReader, ProducersSectionReader};

    let reader = BinaryReader::new(data, 0);
    let section =
        ProducersSectionReader::new(reader).context("Failed to create producers reader")?;

    let mut fields = Vec::new();
    for field in section {
        let field = field.context("Failed to read producers field")?;
        let mut values = Vec::new();
        for value in field.values {
            let value = value.context("Failed to read producers value")?;
            values.push(ProducerValue {
                name: value.name.to_string(),
                version: value.version.to_string(),
            });
        }
        fields.push(ProducerField {
            name: field.name.to_string(),
            values,
        });
    }

    Ok(fields)
}
