use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use ferricel_types::{extensions::ExtensionDecl, functions::RuntimeFunction};
use walrus::{FunctionId, LocalId};

use crate::schema::ProtoSchema;

/// A struct to hold the handles to your runtime functions
pub struct CompilerEnv {
    pub(crate) functions: HashMap<RuntimeFunction, FunctionId>,
}

impl CompilerEnv {
    pub fn get(&self, func: RuntimeFunction) -> FunctionId {
        *self
            .functions
            .get(&func)
            .unwrap_or_else(|| panic!("Function {} missing in runtime module", func.name()))
    }
}

/// Options for CEL compilation
pub struct CompilerOptions {
    /// Optional Protocol Buffer schema for proper wrapper type semantics
    /// This should be a FileDescriptorSet binary (output of `protoc --descriptor_set_out`)
    pub proto_descriptor: Option<Vec<u8>>,
    /// Optional container (namespace) for type name resolution
    /// Example: "google.protobuf" allows using "Timestamp" instead of "google.protobuf.Timestamp"
    /// Follows CEL-go hierarchical resolution: tries container.name, parent.name, ..., name
    pub container: Option<String>,
    /// Logger for compilation warnings and errors
    pub logger: slog::Logger,
    /// Extension function declarations for compile-time validation.
    /// Defaults to an empty list (no extensions).
    pub extensions: Vec<ExtensionDecl>,
}

/// Compilation context that holds state during expression compilation
/// This includes local variable bindings for comprehensions and other scoped contexts
pub struct CompilerContext {
    /// Maps variable names to their local IDs in the WASM function
    /// Used for iteration variables in comprehensions (e.g., "x" in \[1,2,3\].all(x, x > 0))
    pub local_vars: HashMap<String, LocalId>,
    /// Optional Protocol Buffer schema for wrapper type semantics
    pub schema: Option<ProtoSchema>,
    /// Optional container (namespace) for type name resolution
    pub container: Option<String>,
    /// Logger for compilation warnings and errors
    pub logger: slog::Logger,
    /// Registry of host-provided extension functions (shared via Arc for cheap cloning)
    pub extensions: Rc<ExtensionRegistry>,
}

impl CompilerContext {
    /// Create a new context
    pub fn new(
        schema: Option<ProtoSchema>,
        container: Option<String>,
        logger: slog::Logger,
        extensions: &[ExtensionDecl],
    ) -> Self {
        Self {
            local_vars: HashMap::new(),
            schema,
            container,
            logger,
            extensions: Rc::new(ExtensionRegistry::new(extensions)),
        }
    }

    /// Create a child context with an additional local variable binding
    /// This is used when entering a new scope (e.g., comprehension)
    pub fn with_local(&self, name: String, local_id: LocalId) -> Self {
        let mut local_vars = self.local_vars.clone();
        local_vars.insert(name, local_id);
        Self {
            local_vars,
            schema: self.schema.clone(),
            container: self.container.clone(),
            logger: self.logger.clone(),
            extensions: self.extensions.clone(),
        }
    }
}

/// Typed key for looking up a registered extension function.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtensionKey {
    /// Namespace prefix (e.g. `Some("math")` for `math.abs()`), or `None` for flat names.
    pub namespace: Option<String>,
    /// Function name (e.g. `"abs"`).
    pub function: String,
}

impl ExtensionKey {
    pub fn new(namespace: Option<String>, function: String) -> Self {
        Self {
            namespace,
            function,
        }
    }
}

/// Registry of host-provided extension functions, built from `Vec<ExtensionDecl>` at
/// the start of compilation.  Enables fast lookup by `ExtensionKey` and
/// quick detection of registered namespace prefixes.
pub struct ExtensionRegistry {
    /// Map from `ExtensionKey` to the declaration.
    pub by_name: HashMap<ExtensionKey, ExtensionDecl>,
    /// Set of all registered namespace strings (e.g. `"math"`).
    pub namespaces: HashSet<String>,
}

impl ExtensionRegistry {
    pub fn new(extensions: &[ExtensionDecl]) -> Self {
        let mut by_name = HashMap::new();
        let mut namespaces = HashSet::new();
        for decl in extensions {
            if let Some(ref ns) = decl.namespace {
                namespaces.insert(ns.clone());
            }
            by_name.insert(
                ExtensionKey::new(decl.namespace.clone(), decl.function.clone()),
                decl.clone(),
            );
        }
        Self {
            by_name,
            namespaces,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// Describes how an extension function is being called at a particular call-site.
///
/// Used during extension function resolution to determine the call shape:
/// - `Global`     – a plain global call, e.g. `myFunc(x)`
/// - `Namespaced` – a namespaced call, e.g. `math.abs(x)`
/// - `Receiver`   – a receiver/method call, e.g. `x.myFunc()`
pub enum CallShape<'a> {
    Global,
    Namespaced(&'a str),
    Receiver(Option<&'a cel::common::ast::IdedExpr>),
}
