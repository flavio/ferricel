use std::{
    collections::{BTreeSet, HashMap, HashSet},
    rc::Rc,
};

use ferricel_types::{
    extensions::{BuilderChainDecl, BuilderStep, ExtensionDecl},
    functions::RuntimeFunction,
};
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

/// Compilation context that holds state during expression compilation
/// This includes local variable bindings for comprehensions and other scoped contexts
pub struct CompilerContext {
    /// Maps variable names to their local IDs in the Wasm function
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
        extensions: &BTreeSet<ExtensionDecl>,
        builder_chains: &[BuilderChainDecl],
    ) -> Self {
        Self {
            local_vars: HashMap::new(),
            schema,
            container,
            logger,
            extensions: Rc::new(ExtensionRegistry::new(extensions, builder_chains)),
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
    /// Map from `ExtensionKey` to the declaration (flat extensions).
    pub by_name: HashMap<ExtensionKey, ExtensionDecl>,
    /// Set of all registered namespace strings (e.g. `"math"`).
    pub namespaces: HashSet<String>,
    /// Builder entry-point steps, keyed by their full dotted function name
    /// (e.g. `"kw.k8s.apiVersion"`).
    pub builder_entries: HashMap<String, BuilderStep>,
    /// Builder chain and terminal steps, keyed by their short method name
    /// (e.g. `"kind"`, `"list"`, `"get"`, `"verify"`).
    ///
    /// When a function name matches here and the call has a target (receiver),
    /// it is dispatched as a builder step rather than a flat extension.
    pub builder_steps: HashMap<String, Vec<BuilderStep>>,
}

impl ExtensionRegistry {
    pub fn new(extensions: &BTreeSet<ExtensionDecl>, builder_chains: &[BuilderChainDecl]) -> Self {
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

        let mut builder_entries: HashMap<String, BuilderStep> = HashMap::new();
        let mut builder_steps: HashMap<String, Vec<BuilderStep>> = HashMap::new();

        for chain in builder_chains {
            for step in &chain.steps {
                match step {
                    BuilderStep::Entry { function, .. } => {
                        builder_entries.insert(function.clone(), step.clone());
                    }
                    BuilderStep::Chain { function, .. }
                    | BuilderStep::Terminal { function, .. } => {
                        builder_steps
                            .entry(function.clone())
                            .or_default()
                            .push(step.clone());
                    }
                }
            }
        }

        Self {
            by_name,
            namespaces,
            builder_entries,
            builder_steps,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty() && self.builder_entries.is_empty()
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
