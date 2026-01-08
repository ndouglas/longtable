//! Compiler for transforming AST into bytecode.
//!
//! The compiler converts parsed AST nodes into executable bytecode
//! for the Longtable VM.

#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::only_used_in_recursion)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_wraps)]

use std::collections::HashMap;

use longtable_foundation::{Error, ErrorKind, Interner, KeywordId, LtMap, LtVec, Result, Value};

use crate::ast::Ast;
use crate::declaration::DeclarationAnalyzer;
use crate::macro_expander::MacroExpander;
use crate::macro_registry::MacroRegistry;
use crate::namespace::NamespaceContext;
use crate::opcode::{Bytecode, Opcode};
use crate::span::Span;

/// Compiler state for transforming AST to bytecode.
pub struct Compiler {
    /// Constants pool (literals referenced by Const opcode).
    constants: Vec<Value>,
    /// Map from constant value to index (for deduplication).
    constant_map: HashMap<ConstKey, u16>,
    /// Local variable bindings (name -> slot index).
    locals: HashMap<String, u16>,
    /// Next available local slot.
    next_local: u16,
    /// Global variable bindings (name -> slot index).
    /// Persists across compilations for REPL use.
    globals: HashMap<String, u16>,
    /// Next available global slot.
    next_global: u16,
    /// Native function name -> index mapping.
    natives: HashMap<String, u16>,
    /// Compiled functions.
    functions: Vec<CompiledFunction>,
    /// Variables from outer scopes that can be captured (name -> outer slot).
    /// Only set during function compilation.
    outer_locals: Option<HashMap<String, u16>>,
    /// Captured variables for the current function being compiled.
    /// Maps variable name to capture index.
    captures: HashMap<String, u16>,
    /// Namespace context for symbol resolution (aliases, refers).
    namespace_context: NamespaceContext,
    /// Macro registry for macro expansion.
    macro_registry: MacroRegistry,
    /// Optional interner for keyword resolution.
    /// When present, keywords are properly interned as `Value::Keyword(KeywordId)`.
    interner: Option<Interner>,
}

/// Key for constant deduplication.
/// We can't use Value directly as `HashMap` key due to float NaN issues,
/// so we use a wrapper that handles this.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum ConstKey {
    Nil,
    Bool(bool),
    Int(i64),
    Float(u64), // bits representation
    String(String),
    Symbol(String),
    Keyword(String),
}

impl ConstKey {
    fn from_value(v: &Value) -> Option<Self> {
        match v {
            Value::Nil => Some(Self::Nil),
            Value::Bool(b) => Some(Self::Bool(*b)),
            Value::Int(i) => Some(Self::Int(*i)),
            Value::Float(f) => Some(Self::Float(f.to_bits())),
            Value::String(s) => Some(Self::String(s.to_string())),
            Value::Symbol(id) => Some(Self::Symbol(format!("{}", id.index()))),
            Value::Keyword(id) => Some(Self::Keyword(format!("{}", id.index()))),
            // Collections and other types are not deduplicated
            _ => None,
        }
    }
}

/// A compiled function ready for execution.
#[derive(Clone, Debug)]
pub struct CompiledFunction {
    /// Number of parameters.
    pub arity: u8,
    /// Parameter names (for debugging).
    pub params: Vec<String>,
    /// Function bytecode.
    pub code: Bytecode,
    /// Number of local variable slots needed.
    pub locals_count: u16,
    /// Names of captured variables (for closures).
    /// The order corresponds to the capture index used by `LoadCapture`.
    pub captures: Vec<String>,
}

/// Compiled program ready for execution.
#[derive(Clone, Debug, Default)]
pub struct CompiledProgram {
    /// Main bytecode to execute.
    pub code: Bytecode,
    /// Constants pool.
    pub constants: Vec<Value>,
    /// Compiled functions.
    pub functions: Vec<CompiledFunction>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    /// Creates a new compiler.
    #[must_use]
    pub fn new() -> Self {
        let mut compiler = Self {
            constants: Vec::new(),
            constant_map: HashMap::new(),
            locals: HashMap::new(),
            next_local: 0,
            globals: HashMap::new(),
            next_global: 0,
            natives: HashMap::new(),
            functions: Vec::new(),
            outer_locals: None,
            captures: HashMap::new(),
            namespace_context: NamespaceContext::new(),
            macro_registry: MacroRegistry::new(),
            interner: None,
        };

        // Register built-in native functions
        compiler.register_natives();
        compiler
    }

    /// Creates a new compiler with an interner for keyword resolution.
    #[must_use]
    pub fn with_interner(interner: Interner) -> Self {
        let mut compiler = Self {
            constants: Vec::new(),
            constant_map: HashMap::new(),
            locals: HashMap::new(),
            next_local: 0,
            globals: HashMap::new(),
            next_global: 0,
            natives: HashMap::new(),
            functions: Vec::new(),
            outer_locals: None,
            captures: HashMap::new(),
            namespace_context: NamespaceContext::new(),
            macro_registry: MacroRegistry::new(),
            interner: Some(interner),
        };

        // Register built-in native functions
        compiler.register_natives();
        compiler
    }

    /// Creates a new compiler with the given namespace context.
    #[must_use]
    pub fn with_namespace_context(namespace_context: NamespaceContext) -> Self {
        let mut compiler = Self {
            constants: Vec::new(),
            constant_map: HashMap::new(),
            locals: HashMap::new(),
            next_local: 0,
            globals: HashMap::new(),
            next_global: 0,
            natives: HashMap::new(),
            functions: Vec::new(),
            outer_locals: None,
            captures: HashMap::new(),
            namespace_context,
            macro_registry: MacroRegistry::new(),
            interner: None,
        };

        // Register built-in native functions
        compiler.register_natives();
        compiler
    }

    /// Creates a new compiler with stdlib macros pre-registered.
    #[must_use]
    pub fn new_with_stdlib() -> Self {
        Self::with_macro_registry(MacroRegistry::new_with_stdlib())
    }

    /// Creates a new compiler with a macro registry.
    #[must_use]
    pub fn with_macro_registry(macro_registry: MacroRegistry) -> Self {
        let mut compiler = Self {
            constants: Vec::new(),
            constant_map: HashMap::new(),
            locals: HashMap::new(),
            next_local: 0,
            globals: HashMap::new(),
            next_global: 0,
            natives: HashMap::new(),
            functions: Vec::new(),
            outer_locals: None,
            captures: HashMap::new(),
            namespace_context: NamespaceContext::new(),
            macro_registry,
            interner: None,
        };

        // Register built-in native functions
        compiler.register_natives();
        compiler
    }

    /// Prepares the compiler for a new compilation.
    ///
    /// This resets per-compilation state (constants, functions, locals)
    /// while preserving persistent state (globals, macros, namespace context).
    pub fn prepare_for_compilation(&mut self) {
        self.constants.clear();
        self.constant_map.clear();
        self.locals.clear();
        self.next_local = 0;
        self.functions.clear();
        self.outer_locals = None;
        self.captures.clear();
        // Note: globals, next_global, natives, macro_registry, namespace_context persist
    }

    /// Returns a mutable reference to the macro registry.
    pub fn macro_registry_mut(&mut self) -> &mut MacroRegistry {
        &mut self.macro_registry
    }

    /// Returns a reference to the macro registry.
    #[must_use]
    pub fn macro_registry(&self) -> &MacroRegistry {
        &self.macro_registry
    }

    /// Sets the namespace context for symbol resolution.
    pub fn set_namespace_context(&mut self, context: NamespaceContext) {
        self.namespace_context = context;
    }

    /// Returns a reference to the namespace context.
    #[must_use]
    pub fn namespace_context(&self) -> &NamespaceContext {
        &self.namespace_context
    }

    /// Registers built-in native functions.
    fn register_natives(&mut self) {
        let natives = [
            // Arithmetic
            "+",
            "-",
            "*",
            "/",
            "mod",
            // Comparison
            "=",
            "!=",
            "<",
            "<=",
            ">",
            ">=",
            // Logic
            "not",
            "and",
            "or",
            // Predicates
            "nil?",
            "some?",
            "int?",
            "float?",
            "string?",
            "keyword?",
            "symbol?",
            "list?",
            "vector?",
            "map?",
            "set?",
            // Collections
            "count",
            "empty?",
            "first",
            "rest",
            "nth",
            "conj",
            "cons",
            "get",
            "assoc",
            "dissoc",
            "contains?",
            "keys",
            "vals",
            // String
            "str",
            "str/len",
            "str/upper",
            "str/lower",
            // Math
            "abs",
            "min",
            "max",
            "floor",
            "ceil",
            "round",
            "sqrt",
            // Misc
            "print",
            "println",
            "type",
            // Stage S1: Critical functions
            "inc",
            "dec",
            "last",
            "range",
            // Stage S2: String functions
            "str/split",
            "str/join",
            "str/trim",
            "str/trim-left",
            "str/trim-right",
            "str/starts-with?",
            "str/ends-with?",
            "str/contains?",
            "str/replace",
            "str/replace-all",
            "str/blank?",
            "str/substring",
            "format",
            // Stage S3: Collection functions
            "take",
            "drop",
            "concat",
            "reverse",
            "vec",
            "set",
            "into",
            "sort",
            "merge",
            // Stage S4: Math functions
            "rem",
            "clamp",
            "trunc",
            "pow",
            "cbrt",
            "exp",
            "log",
            "log10",
            "log2",
            "sin",
            "cos",
            "tan",
            "asin",
            "acos",
            "atan",
            "atan2",
            "pi",
            "e",
            // Stage S5: Extended collection functions
            "flatten",
            "distinct",
            "dedupe",
            "partition",
            "partition-all",
            // Stage S6: Vector math functions
            "vec+",
            "vec-",
            "vec*",
            "vec-scale",
            "vec-dot",
            "vec-cross",
            "vec-length",
            "vec-length-sq",
            "vec-normalize",
            "vec-distance",
            "vec-lerp",
            "vec-angle",
            // Stage S7: Remaining functions
            "bool?",
            "number?",
            "coll?",
            "fn?",
            "entity?",
            "sinh",
            "cosh",
            "tanh",
            "interleave",
            "interpose",
            "zip",
            "repeat",
        ];

        for (idx, name) in natives.iter().enumerate() {
            self.natives.insert((*name).to_string(), idx as u16);
        }
    }

    /// Compiles an AST expression into bytecode.
    ///
    /// This method expands macros before compilation.
    pub fn compile_expr(&mut self, ast: &Ast) -> Result<Bytecode> {
        // Expand macros first
        let expanded = {
            let mut expander = MacroExpander::new(&mut self.macro_registry);
            expander.expand(ast)?
        };

        let mut code = Bytecode::new();
        self.compile_node(&expanded, &mut code)?;
        Ok(code)
    }

    /// Compiles an AST expression into bytecode without macro expansion.
    ///
    /// Use this when you've already expanded macros or want to skip expansion.
    pub fn compile_expr_raw(&mut self, ast: &Ast) -> Result<Bytecode> {
        let mut code = Bytecode::new();
        self.compile_node(ast, &mut code)?;
        Ok(code)
    }

    /// Compiles multiple expressions into a program.
    ///
    /// This method expands macros before compilation.
    pub fn compile(&mut self, asts: &[Ast]) -> Result<CompiledProgram> {
        // Expand macros first
        let expanded = {
            let mut expander = MacroExpander::new(&mut self.macro_registry);
            expander.expand_all(asts)?
        };

        let mut code = Bytecode::new();

        for ast in &expanded {
            self.compile_node(ast, &mut code)?;
            // Pop intermediate results except the last
            if !code.is_empty() {
                code.emit(Opcode::Pop);
            }
        }

        // Remove the last Pop if we added one
        if !code.is_empty() && matches!(code.ops.last(), Some(Opcode::Pop)) {
            code.ops.pop();
        }

        Ok(CompiledProgram {
            code,
            constants: self.constants.clone(),
            functions: self.functions.clone(),
        })
    }

    /// Compiles multiple expressions into a program without macro expansion.
    ///
    /// Use this when you've already expanded macros or want to skip expansion.
    pub fn compile_raw(&mut self, asts: &[Ast]) -> Result<CompiledProgram> {
        let mut code = Bytecode::new();

        for ast in asts {
            self.compile_node(ast, &mut code)?;
            // Pop intermediate results except the last
            if !code.is_empty() {
                code.emit(Opcode::Pop);
            }
        }

        // Remove the last Pop if we added one
        if !code.is_empty() && matches!(code.ops.last(), Some(Opcode::Pop)) {
            code.ops.pop();
        }

        Ok(CompiledProgram {
            code,
            constants: self.constants.clone(),
            functions: self.functions.clone(),
        })
    }

    /// Compiles a single AST node.
    fn compile_node(&mut self, ast: &Ast, code: &mut Bytecode) -> Result<()> {
        match ast {
            Ast::Nil(_) => {
                let idx = self.add_constant(Value::Nil);
                code.emit(Opcode::Const(idx));
            }
            Ast::Bool(b, _) => {
                let idx = self.add_constant(Value::Bool(*b));
                code.emit(Opcode::Const(idx));
            }
            Ast::Int(n, _) => {
                let idx = self.add_constant(Value::Int(*n));
                code.emit(Opcode::Const(idx));
            }
            Ast::Float(n, _) => {
                let idx = self.add_constant(Value::Float(*n));
                code.emit(Opcode::Const(idx));
            }
            Ast::String(s, _) => {
                let idx = self.add_constant(Value::String(s.as_str().into()));
                code.emit(Opcode::Const(idx));
            }
            Ast::Symbol(name, _) => {
                self.compile_symbol(name, code);
            }
            Ast::Keyword(name, _) => {
                // Keywords compile to themselves as values
                let value = if let Some(ref mut interner) = self.interner {
                    // When we have an interner, properly intern the keyword
                    let keyword_id = interner.intern_keyword(name);
                    Value::Keyword(keyword_id)
                } else {
                    // Fallback: store as string with keyword marker
                    // This is less correct but maintains backward compatibility
                    Value::String(format!(":{name}").into())
                };
                let idx = self.add_constant(value);
                code.emit(Opcode::Const(idx));
            }
            Ast::List(elements, span) => {
                self.compile_list(elements, *span, code)?;
            }
            Ast::Vector(elements, _) => {
                self.compile_vector(elements, code)?;
            }
            Ast::Set(elements, _) => {
                self.compile_set(elements, code)?;
            }
            Ast::Map(entries, _) => {
                self.compile_map(entries, code)?;
            }
            Ast::Quote(inner, _) => {
                // For now, just compile the inner value as a literal
                // Full quote semantics would create unevaluated data
                self.compile_quoted(inner, code)?;
            }
            Ast::Unquote(_, span) | Ast::UnquoteSplice(_, span) => {
                return Err(self.error(*span, "unquote outside of syntax-quote"));
            }
            Ast::SyntaxQuote(inner, _) => {
                // Simplified: treat like quote for now
                self.compile_quoted(inner, code)?;
            }
            Ast::Tagged(tag, inner, span) => {
                self.compile_tagged(tag, inner, *span, code)?;
            }
        }
        Ok(())
    }

    /// Compiles a symbol reference (variable lookup or special form).
    fn compile_symbol(&mut self, name: &str, code: &mut Bytecode) {
        // Check for local variable
        if let Some(&slot) = self.locals.get(name) {
            code.emit(Opcode::LoadLocal(slot));
            return;
        }

        // Check for captured variable (from outer scope)
        if let Some(&capture_idx) = self.captures.get(name) {
            code.emit(Opcode::LoadCapture(capture_idx));
            return;
        }

        // Check if this variable exists in outer scope and needs to be captured
        if let Some(outer) = &self.outer_locals {
            if outer.contains_key(name) {
                // Add to captures
                let capture_idx = self.captures.len() as u16;
                self.captures.insert(name.to_string(), capture_idx);
                code.emit(Opcode::LoadCapture(capture_idx));
                return;
            }
        }

        // Check for global variable (persists across compilations)
        if let Some(&slot) = self.globals.get(name) {
            code.emit(Opcode::LoadGlobal(slot));
            return;
        }

        // Check for qualified name (namespace/symbol or alias/symbol)
        if let Some((prefix, symbol)) = name.split_once('/') {
            // Try to resolve the prefix as an alias
            let resolved = self
                .namespace_context
                .resolve_alias(prefix, symbol)
                .unwrap_or_else(|| {
                    // If not an alias, it might already be fully qualified
                    name.to_string()
                });
            // Emit as a qualified symbol string for runtime lookup
            let idx = self.add_constant(Value::String(format!("'{resolved}").into()));
            code.emit(Opcode::Const(idx));
            return;
        }

        // Check for referred symbol (imported from another namespace)
        if let Some(qualified) = self.namespace_context.resolve_referred(name) {
            // Emit as a qualified symbol string for runtime lookup
            let idx = self.add_constant(Value::String(format!("'{qualified}").into()));
            code.emit(Opcode::Const(idx));
            return;
        }

        // Symbol resolves to itself (for use as data)
        let idx = self.add_constant(Value::String(format!("'{name}").into()));
        code.emit(Opcode::Const(idx));
    }

    /// Compiles a list (function call or special form).
    fn compile_list(&mut self, elements: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if elements.is_empty() {
            // Empty list is nil
            let idx = self.add_constant(Value::Nil);
            code.emit(Opcode::Const(idx));
            return Ok(());
        }

        let first = &elements[0];
        let args = &elements[1..];

        // Check for special forms
        if let Ast::Symbol(name, _) = first {
            match name.as_str() {
                "if" => return self.compile_if(args, span, code),
                "let" => return self.compile_let(args, span, code),
                "do" => return self.compile_do(args, code),
                "fn" => return self.compile_fn(args, span, code),
                "def" => return self.compile_def(args, span, code),
                "fn:" => return self.compile_fn_decl(args, span, code),
                "quote" => return self.compile_quote_form(args, span, code),
                // Macro-support special forms
                "and*" => return self.compile_and_star(args, span, code),
                "or*" => return self.compile_or_star(args, span, code),
                "cond*" => return self.compile_cond_star(args, span, code),
                "thread-first" => return self.compile_thread_first(args, span, code),
                "thread-last" => return self.compile_thread_last(args, span, code),
                "doto*" => return self.compile_doto_star(args, span, code),
                // Higher-order functions (emit special opcodes)
                "map" => return self.compile_hof_map(args, span, code),
                "filter" => return self.compile_hof_filter(args, span, code),
                "reduce" => return self.compile_hof_reduce(args, span, code),
                "every?" => return self.compile_hof_every(args, span, code),
                "some" => return self.compile_hof_some(args, span, code),
                "take-while" => return self.compile_hof_take_while(args, span, code),
                "drop-while" => return self.compile_hof_drop_while(args, span, code),
                "remove" => return self.compile_hof_remove(args, span, code),
                "group-by" => return self.compile_hof_group_by(args, span, code),
                "zip-with" => return self.compile_hof_zip_with(args, span, code),
                "repeatedly" => return self.compile_hof_repeatedly(args, span, code),
                // World/entity operations (emit special opcodes with context access)
                "get-component" => return self.compile_get_component(args, span, code),
                "get-field" => return self.compile_get_field(args, span, code),
                "with-component" => return self.compile_with_component(args, span, code),
                "find-relationships" => return self.compile_find_relationships(args, span, code),
                "targets" => return self.compile_targets(args, span, code),
                "sources" => return self.compile_sources(args, span, code),
                // Entity construction
                "entity-ref" => return self.compile_entity_ref(args, span, code),
                // World mutation operations
                "spawn" => return self.compile_spawn(args, span, code),
                "destroy" => return self.compile_destroy(args, span, code),
                "set-component" => return self.compile_set_component(args, span, code),
                "set-field" => return self.compile_set_field(args, span, code),
                "link" => return self.compile_link(args, span, code),
                "unlink" => return self.compile_unlink(args, span, code),
                // Declaration forms (compile to registration opcodes)
                "component:" => return self.compile_component_decl(elements, span, code),
                "relationship:" => return self.compile_relationship_decl(elements, span, code),
                "verb:" => return self.compile_verb_decl(elements, span, code),
                "direction:" => return self.compile_direction_decl(elements, span, code),
                "preposition:" => return self.compile_preposition_decl(elements, span, code),
                "pronoun:" => return self.compile_pronoun_decl(elements, span, code),
                "adverb:" => return self.compile_adverb_decl(elements, span, code),
                "type:" => return self.compile_type_decl(elements, span, code),
                "scope:" => return self.compile_scope_decl(elements, span, code),
                "command:" => return self.compile_command_decl(elements, span, code),
                "action:" => return self.compile_action_decl(elements, span, code),
                "rule:" => return self.compile_rule_decl(elements, span, code),
                _ => {}
            }

            // Check for native/builtin function
            if let Some(&native_idx) = self.natives.get(name.as_str()) {
                // Check for operators that map directly to opcodes
                // These handle their own argument compilation
                match name.as_str() {
                    "+" => {
                        self.compile_binary_op(args, Opcode::Add, span, code)?;
                        return Ok(());
                    }
                    "-" if args.len() == 1 => {
                        self.compile_node(&args[0], code)?;
                        code.emit(Opcode::Neg);
                        return Ok(());
                    }
                    "-" => {
                        self.compile_binary_op(args, Opcode::Sub, span, code)?;
                        return Ok(());
                    }
                    "*" => {
                        self.compile_binary_op(args, Opcode::Mul, span, code)?;
                        return Ok(());
                    }
                    "/" => {
                        self.compile_binary_op(args, Opcode::Div, span, code)?;
                        return Ok(());
                    }
                    "mod" => {
                        self.compile_binary_op(args, Opcode::Mod, span, code)?;
                        return Ok(());
                    }
                    "=" => {
                        self.compile_binary_op(args, Opcode::Eq, span, code)?;
                        return Ok(());
                    }
                    "!=" => {
                        self.compile_binary_op(args, Opcode::Ne, span, code)?;
                        return Ok(());
                    }
                    "<" => {
                        self.compile_binary_op(args, Opcode::Lt, span, code)?;
                        return Ok(());
                    }
                    "<=" => {
                        self.compile_binary_op(args, Opcode::Le, span, code)?;
                        return Ok(());
                    }
                    ">" => {
                        self.compile_binary_op(args, Opcode::Gt, span, code)?;
                        return Ok(());
                    }
                    ">=" => {
                        self.compile_binary_op(args, Opcode::Ge, span, code)?;
                        return Ok(());
                    }
                    "not" => {
                        if args.len() != 1 {
                            return Err(self.error(span, "not requires exactly 1 argument"));
                        }
                        self.compile_node(&args[0], code)?;
                        code.emit(Opcode::Not);
                        return Ok(());
                    }
                    "print" => {
                        if args.len() != 1 {
                            return Err(self.error(span, "print requires exactly 1 argument"));
                        }
                        self.compile_node(&args[0], code)?;
                        code.emit(Opcode::Print);
                        // Print returns nil
                        let idx = self.add_constant(Value::Nil);
                        code.emit(Opcode::Const(idx));
                        return Ok(());
                    }
                    _ => {
                        // Other natives: compile arguments then call
                        for arg in args {
                            self.compile_node(arg, code)?;
                        }
                        let arg_count = u8::try_from(args.len())
                            .map_err(|_| self.error(span, "too many arguments"))?;
                        code.emit(Opcode::CallNative(native_idx, arg_count));
                        return Ok(());
                    }
                }
            }
        }

        // General function call
        // Compile the function expression
        self.compile_node(first, code)?;
        // Compile arguments
        for arg in args {
            self.compile_node(arg, code)?;
        }
        // Call with argument count
        let arg_count =
            u16::try_from(args.len()).map_err(|_| self.error(span, "too many arguments"))?;
        code.emit(Opcode::Call(arg_count));

        Ok(())
    }

    /// Compiles a binary operator with proper argument handling.
    fn compile_binary_op(
        &mut self,
        args: &[Ast],
        op: Opcode,
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() < 2 {
            return Err(self.error(span, "binary operator requires at least 2 arguments"));
        }

        // Already compiled first arg, compile second
        self.compile_node(&args[0], code)?;
        self.compile_node(&args[1], code)?;
        code.emit(op.clone());

        // Chain additional arguments: (+ 1 2 3) -> (+ (+ 1 2) 3)
        for arg in &args[2..] {
            self.compile_node(arg, code)?;
            code.emit(op.clone());
        }

        Ok(())
    }

    /// Compiles an if expression.
    fn compile_if(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() < 2 || args.len() > 3 {
            return Err(self.error(span, "if requires 2 or 3 arguments"));
        }

        let condition = &args[0];
        let then_branch = &args[1];
        let else_branch = args.get(2);

        // Compile condition
        self.compile_node(condition, code)?;

        // Jump to else if false
        let jump_to_else = code.emit(Opcode::JumpIfNot(0));

        // Compile then branch
        self.compile_node(then_branch, code)?;

        if let Some(else_expr) = else_branch {
            // Jump over else branch
            let jump_over_else = code.emit(Opcode::Jump(0));

            // Patch jump to else
            let else_start = code.len();
            let offset = i16::try_from(else_start - jump_to_else - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_to_else, offset);

            // Compile else branch
            self.compile_node(else_expr, code)?;

            // Patch jump over else
            let end = code.len();
            let offset = i16::try_from(end - jump_over_else - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_over_else, offset);
        } else {
            // No else branch, result is nil
            let jump_over_nil = code.emit(Opcode::Jump(0));

            // Patch jump to else (which pushes nil)
            let nil_start = code.len();
            let offset = i16::try_from(nil_start - jump_to_else - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_to_else, offset);

            // Push nil for else case
            let idx = self.add_constant(Value::Nil);
            code.emit(Opcode::Const(idx));

            // Patch jump over nil
            let end = code.len();
            let offset = i16::try_from(end - jump_over_nil - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_over_nil, offset);
        }

        Ok(())
    }

    /// Compiles a let expression.
    ///
    /// Uses letrec-style semantics: all bindings are visible to all values,
    /// enabling recursive function definitions like `(let [f (fn [x] (f x))] ...)`.
    fn compile_let(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            return Err(self.error(span, "let requires bindings vector"));
        }

        let bindings = match &args[0] {
            Ast::Vector(bindings, _) => bindings,
            _ => return Err(self.error(span, "let bindings must be a vector")),
        };

        if bindings.len() % 2 != 0 {
            return Err(self.error(span, "let bindings must have even number of forms"));
        }

        // Save current locals state for restoration
        let saved_locals = self.locals.clone();
        let saved_next = self.next_local;

        // Phase 1: Allocate slots for ALL bindings first (letrec semantics)
        // This allows recursive references within binding values
        let mut binding_info: Vec<(String, &Ast, u16)> = Vec::new();
        let mut binding_names: Vec<String> = Vec::new();
        for chunk in bindings.chunks(2) {
            let name = match &chunk[0] {
                Ast::Symbol(name, _) => name.clone(),
                _ => return Err(self.error(span, "let binding name must be a symbol")),
            };
            let value = &chunk[1];

            // Allocate slot and register name
            let slot = self.next_local;
            self.next_local += 1;
            self.locals.insert(name.clone(), slot);
            binding_names.push(name.clone());
            binding_info.push((name, value, slot));
        }

        // Phase 2: Compile values and store them
        // Track self-referential captures that need patching
        // (slot, capture_indices_to_patch) - indices in the captures list that refer to this let
        let mut patches: Vec<(u16, Vec<(u16, u16)>)> = Vec::new(); // (slot, [(capture_idx, local_slot)])

        for (_name, value, slot) in &binding_info {
            // Compile the value - this handles both regular values and closures
            self.compile_node(value, code)?;

            // Check if this created a closure with captures that need patching
            // We detect this by seeing if the emitted code ends with MakeClosure
            // and checking if any captures are from this let scope
            if let Some(Opcode::MakeClosure(fn_index, _capture_count)) = code.ops.last().cloned() {
                let func = &self.functions[fn_index as usize];
                let mut patch_indices: Vec<(u16, u16)> = Vec::new();

                for (cap_idx, cap_name) in func.captures.iter().enumerate() {
                    // Check if this capture is from the current let scope
                    if let Some(local_slot) = binding_names
                        .iter()
                        .position(|n| n == cap_name)
                        .and_then(|_| self.locals.get(cap_name))
                    {
                        patch_indices.push((cap_idx as u16, *local_slot));
                    }
                }

                if !patch_indices.is_empty() {
                    // Store initial closure, we'll patch it later
                    code.emit(Opcode::StoreLocal(*slot));

                    // Record that we need to patch this slot later
                    patches.push((*slot, patch_indices));
                    continue;
                }
            }

            code.emit(Opcode::StoreLocal(*slot));
        }

        // Phase 3: Patch self-referential closures
        // Now all slots have their values, so we can patch closures that reference themselves
        for (slot, patch_indices) in patches {
            for (capture_idx, local_slot) in patch_indices {
                // Load the closure to patch
                code.emit(Opcode::LoadLocal(slot));
                // Load the value to patch in (the actual closure from its slot)
                code.emit(Opcode::LoadLocal(local_slot));
                // Patch the capture slot
                code.emit(Opcode::PatchCapture(capture_idx));
                // Store the patched closure back (PatchCapture leaves it on stack)
                code.emit(Opcode::StoreLocal(slot));
            }
        }

        // Compile body expressions
        let body = &args[1..];
        if body.is_empty() {
            let idx = self.add_constant(Value::Nil);
            code.emit(Opcode::Const(idx));
        } else {
            for (i, expr) in body.iter().enumerate() {
                self.compile_node(expr, code)?;
                // Pop intermediate results
                if i < body.len() - 1 {
                    code.emit(Opcode::Pop);
                }
            }
        }

        // Restore locals
        self.locals = saved_locals;
        self.next_local = saved_next;

        Ok(())
    }

    /// Compiles a do expression (sequence).
    fn compile_do(&mut self, args: &[Ast], code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            let idx = self.add_constant(Value::Nil);
            code.emit(Opcode::Const(idx));
        } else {
            for (i, expr) in args.iter().enumerate() {
                self.compile_node(expr, code)?;
                if i < args.len() - 1 {
                    code.emit(Opcode::Pop);
                }
            }
        }
        Ok(())
    }

    /// Compiles a fn expression (lambda).
    fn compile_fn(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        // (fn [params...] body...)
        if args.is_empty() {
            return Err(self.error(span, "fn requires parameters vector"));
        }

        // Extract parameters
        let params = match &args[0] {
            Ast::Vector(params, _) => params,
            _ => return Err(self.error(span, "fn parameters must be a vector")),
        };

        // Extract parameter names
        let mut param_names = Vec::new();
        for param in params {
            match param {
                Ast::Symbol(name, _) => param_names.push(name.clone()),
                _ => return Err(self.error(span, "fn parameter must be a symbol")),
            }
        }

        let arity =
            u8::try_from(param_names.len()).map_err(|_| self.error(span, "too many parameters"))?;

        // Save current compiler state
        let saved_locals = self.locals.clone();
        let saved_next = self.next_local;
        let saved_outer = self.outer_locals.take();
        let saved_captures = std::mem::take(&mut self.captures);

        // Set up outer locals for closure capture
        // Merge current locals with any existing outer locals
        let mut combined_outer = saved_outer.clone().unwrap_or_default();
        for (name, slot) in &saved_locals {
            combined_outer.insert(name.clone(), *slot);
        }
        self.outer_locals = Some(combined_outer);

        // Reset locals for function compilation
        self.locals.clear();
        self.next_local = 0;
        self.captures.clear();

        // Add parameters as locals
        for name in &param_names {
            let slot = self.next_local;
            self.next_local += 1;
            self.locals.insert(name.clone(), slot);
        }

        // Compile function body
        let mut fn_code = Bytecode::new();
        let body = &args[1..];

        if body.is_empty() {
            let idx = self.add_constant(Value::Nil);
            fn_code.emit(Opcode::Const(idx));
        } else {
            for (i, expr) in body.iter().enumerate() {
                self.compile_node(expr, &mut fn_code)?;
                if i < body.len() - 1 {
                    fn_code.emit(Opcode::Pop);
                }
            }
        }

        // Add return instruction
        fn_code.emit(Opcode::Return);

        let locals_count = self.next_local;

        // Collect captures in order (sorted by capture index)
        let mut capture_names: Vec<(String, u16)> = self.captures.drain().collect();
        capture_names.sort_by_key(|(_, idx)| *idx);
        let captures: Vec<String> = capture_names.into_iter().map(|(name, _)| name).collect();

        // Restore compiler state
        self.locals = saved_locals;
        self.next_local = saved_next;
        self.outer_locals = saved_outer;
        self.captures = saved_captures;

        // Create compiled function
        let func = CompiledFunction {
            arity,
            params: param_names,
            code: fn_code,
            locals_count,
            captures: captures.clone(),
        };

        // Add to functions table
        let fn_index = u32::try_from(self.functions.len())
            .map_err(|_| self.error(span, "too many functions"))?;
        self.functions.push(func);

        if captures.is_empty() {
            // No captures - emit as constant
            let fn_value = Value::Fn(longtable_foundation::LtFn::Compiled(
                longtable_foundation::CompiledFn::new(fn_index),
            ));
            let idx = self.add_constant(fn_value);
            code.emit(Opcode::Const(idx));
        } else {
            // Has captures - emit code to load captured values and create closure
            let capture_count =
                u16::try_from(captures.len()).map_err(|_| self.error(span, "too many captures"))?;

            // Load each captured variable onto the stack
            for name in &captures {
                // Look up the variable in current locals (after restoring)
                if let Some(&slot) = self.locals.get(name) {
                    code.emit(Opcode::LoadLocal(slot));
                } else {
                    // Should not happen - captured variable should exist
                    return Err(self.error(span, &format!("captured variable '{name}' not found")));
                }
            }

            // Emit MakeClosure to create the function with captures
            code.emit(Opcode::MakeClosure(fn_index, capture_count));
        }

        Ok(())
    }

    /// Compiles a def expression (global definition).
    fn compile_def(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        // For now, def just creates a local binding
        if args.len() != 2 {
            return Err(self.error(span, "def requires name and value"));
        }

        let name = match &args[0] {
            Ast::Symbol(name, _) => name.clone(),
            _ => return Err(self.error(span, "def name must be a symbol")),
        };

        // Compile the value
        self.compile_node(&args[1], code)?;

        // Store in local slot
        let slot = self.next_local;
        self.next_local += 1;
        code.emit(Opcode::StoreLocal(slot));
        self.locals.insert(name, slot);

        // def returns the value
        code.emit(Opcode::LoadLocal(slot));

        Ok(())
    }

    /// Compiles a `fn:` declaration (global function/value definition).
    ///
    /// Syntax:
    /// - `(fn: name value)` - Define a global value
    /// - `(fn: name [params] body...)` - Define a global function
    /// - `(fn: name "docstring" [params] body...)` - Define with docstring (ignored for now)
    fn compile_fn_decl(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            return Err(self.error(span, "fn: requires a name"));
        }

        // First argument is always the name
        let name = match &args[0] {
            Ast::Symbol(name, _) => name.clone(),
            _ => return Err(self.error(span, "fn: name must be a symbol")),
        };

        let rest = &args[1..];

        // Determine form based on remaining args
        match rest.first() {
            None => {
                return Err(self.error(span, "fn: requires a value or function definition"));
            }
            // (fn: name [params] body...) - function definition
            Some(Ast::Vector(_, _)) => {
                // Compile as (fn [params] body...)
                self.compile_fn(rest, span, code)?;
            }
            // (fn: name "docstring" [params] body...) - function with docstring
            Some(Ast::String(_, _)) if rest.len() >= 2 => {
                // Skip the docstring, compile the function part
                let fn_args = &rest[1..];
                if fn_args.is_empty() {
                    return Err(self.error(span, "fn: missing function parameters after docstring"));
                }
                match &fn_args[0] {
                    Ast::Vector(_, _) => {
                        self.compile_fn(fn_args, span, code)?;
                    }
                    _ => {
                        return Err(
                            self.error(span, "fn: expected parameter vector after docstring")
                        );
                    }
                }
            }
            // (fn: name value) - simple value definition
            Some(_) if rest.len() == 1 => {
                self.compile_node(&rest[0], code)?;
            }
            _ => {
                return Err(self.error(
                    span,
                    "fn: expected value, [params] body..., or \"docstring\" [params] body...",
                ));
            }
        }

        // Get or allocate global slot for this name
        let slot = if let Some(&existing) = self.globals.get(&name) {
            existing
        } else {
            let slot = self.next_global;
            self.next_global += 1;
            self.globals.insert(name.clone(), slot);
            slot
        };

        // Store in global slot
        code.emit(Opcode::StoreGlobal(slot));

        // fn: returns the defined value
        code.emit(Opcode::LoadGlobal(slot));

        Ok(())
    }

    /// Compiles a quote special form.
    fn compile_quote_form(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 1 {
            return Err(self.error(span, "quote requires exactly 1 argument"));
        }
        self.compile_quoted(&args[0], code)
    }

    /// Compiles `and*` - short-circuiting logical AND.
    ///
    /// - `(and*)` -> `true`
    /// - `(and* x)` -> `x`
    /// - `(and* x y ...)` -> if x is falsy, return x; else evaluate rest
    fn compile_and_star(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            // (and*) -> true
            let idx = self.add_constant(Value::Bool(true));
            code.emit(Opcode::Const(idx));
            return Ok(());
        }

        if args.len() == 1 {
            // (and* x) -> x
            return self.compile_node(&args[0], code);
        }

        // (and* x y ...) -> short-circuit evaluation
        // Compile first arg
        self.compile_node(&args[0], code)?;

        // For each subsequent arg, check if previous was falsy
        let mut jump_if_false_positions = Vec::new();

        for arg in &args[1..] {
            // Duplicate the current value to test it
            code.emit(Opcode::Dup);
            // If falsy, jump to end (keeping the falsy value)
            let jump_pos = code.emit(Opcode::JumpIfNot(0));
            jump_if_false_positions.push(jump_pos);
            // Pop the duplicated value (we'll replace with next)
            code.emit(Opcode::Pop);
            // Compile next arg
            self.compile_node(arg, code)?;
        }

        // Patch all jumps to here
        let end_pos = code.len();
        for jump_pos in jump_if_false_positions {
            let offset = i16::try_from(end_pos - jump_pos - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_pos, offset);
        }

        Ok(())
    }

    /// Compiles `or*` - short-circuiting logical OR.
    ///
    /// - `(or*)` -> `nil`
    /// - `(or* x)` -> `x`
    /// - `(or* x y ...)` -> if x is truthy, return x; else evaluate rest
    fn compile_or_star(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            // (or*) -> nil
            let idx = self.add_constant(Value::Nil);
            code.emit(Opcode::Const(idx));
            return Ok(());
        }

        if args.len() == 1 {
            // (or* x) -> x
            return self.compile_node(&args[0], code);
        }

        // (or* x y ...) -> short-circuit evaluation
        // Compile first arg
        self.compile_node(&args[0], code)?;

        // For each subsequent arg, check if previous was truthy
        let mut jump_if_true_positions = Vec::new();

        for arg in &args[1..] {
            // Duplicate the current value to test it
            code.emit(Opcode::Dup);
            // If truthy, jump to end (keeping the truthy value)
            let jump_pos = code.emit(Opcode::JumpIf(0));
            jump_if_true_positions.push(jump_pos);
            // Pop the duplicated value (we'll replace with next)
            code.emit(Opcode::Pop);
            // Compile next arg
            self.compile_node(arg, code)?;
        }

        // Patch all jumps to here
        let end_pos = code.len();
        for jump_pos in jump_if_true_positions {
            let offset = i16::try_from(end_pos - jump_pos - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_pos, offset);
        }

        Ok(())
    }

    /// Compiles `cond*` - multi-branch conditional.
    ///
    /// - `(cond*)` -> `nil`
    /// - `(cond* test expr rest...)` -> `(if test expr (cond* rest...))`
    fn compile_cond_star(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            // (cond*) -> nil
            let idx = self.add_constant(Value::Nil);
            code.emit(Opcode::Const(idx));
            return Ok(());
        }

        if args.len() % 2 != 0 {
            return Err(self.error(span, "cond requires even number of forms (test/expr pairs)"));
        }

        // Build nested if structure
        let mut jump_to_ends = Vec::new();

        for i in (0..args.len()).step_by(2) {
            let test = &args[i];
            let expr = &args[i + 1];

            // Compile test
            self.compile_node(test, code)?;

            // Jump to next clause if false
            let jump_to_next = code.emit(Opcode::JumpIfNot(0));

            // Compile expression for this clause
            self.compile_node(expr, code)?;

            // Jump to end (skip remaining clauses)
            let jump_to_end = code.emit(Opcode::Jump(0));
            jump_to_ends.push(jump_to_end);

            // Patch jump to next clause
            let next_clause_pos = code.len();
            let offset = i16::try_from(next_clause_pos - jump_to_next - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_to_next, offset);
        }

        // If no clause matched, result is nil
        let idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(idx));

        // Patch all jumps to end
        let end_pos = code.len();
        for jump_pos in jump_to_ends {
            let offset = i16::try_from(end_pos - jump_pos - 1)
                .map_err(|_| self.error(span, "jump offset too large"))?;
            code.patch_jump(jump_pos, offset);
        }

        Ok(())
    }

    /// Compiles `thread-first` - threads value through forms as first argument.
    ///
    /// - `(thread-first x)` -> `x`
    /// - `(thread-first x (f a b))` -> `(f x a b)`
    /// - `(thread-first x f)` -> `(f x)`
    fn compile_thread_first(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.is_empty() {
            return Err(self.error(span, "thread-first requires at least 1 argument"));
        }

        if args.len() == 1 {
            // (thread-first x) -> x
            return self.compile_node(&args[0], code);
        }

        // Transform and compile: thread x through forms
        let threaded = self.thread_first_transform(&args[0], &args[1..])?;
        self.compile_node(&threaded, code)
    }

    /// Recursively transforms thread-first forms.
    fn thread_first_transform(&self, x: &Ast, forms: &[Ast]) -> Result<Ast> {
        if forms.is_empty() {
            return Ok(x.clone());
        }

        let form = &forms[0];
        let rest = &forms[1..];

        // Transform the first form
        let transformed = match form {
            Ast::List(elements, span) if !elements.is_empty() => {
                // (f a b) -> (f x a b)
                let mut new_elements = vec![elements[0].clone(), x.clone()];
                new_elements.extend(elements[1..].iter().cloned());
                Ast::List(new_elements, *span)
            }
            Ast::Symbol(_, span) => {
                // f -> (f x)
                Ast::List(vec![form.clone(), x.clone()], *span)
            }
            _ => {
                // Treat as function call
                Ast::List(vec![form.clone(), x.clone()], form.span())
            }
        };

        // Continue threading through rest
        self.thread_first_transform(&transformed, rest)
    }

    /// Compiles `thread-last` - threads value through forms as last argument.
    ///
    /// - `(thread-last x)` -> `x`
    /// - `(thread-last x (f a b))` -> `(f a b x)`
    /// - `(thread-last x f)` -> `(f x)`
    fn compile_thread_last(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            return Err(self.error(span, "thread-last requires at least 1 argument"));
        }

        if args.len() == 1 {
            // (thread-last x) -> x
            return self.compile_node(&args[0], code);
        }

        // Transform and compile: thread x through forms
        let threaded = self.thread_last_transform(&args[0], &args[1..])?;
        self.compile_node(&threaded, code)
    }

    /// Recursively transforms thread-last forms.
    fn thread_last_transform(&self, x: &Ast, forms: &[Ast]) -> Result<Ast> {
        if forms.is_empty() {
            return Ok(x.clone());
        }

        let form = &forms[0];
        let rest = &forms[1..];

        // Transform the first form
        let transformed = match form {
            Ast::List(elements, span) if !elements.is_empty() => {
                // (f a b) -> (f a b x)
                let mut new_elements = elements.clone();
                new_elements.push(x.clone());
                Ast::List(new_elements, *span)
            }
            Ast::Symbol(_, span) => {
                // f -> (f x)
                Ast::List(vec![form.clone(), x.clone()], *span)
            }
            _ => {
                // Treat as function call
                Ast::List(vec![form.clone(), x.clone()], form.span())
            }
        };

        // Continue threading through rest
        self.thread_last_transform(&transformed, rest)
    }

    /// Compiles `doto*` - evaluates forms with first arg, returns first arg.
    ///
    /// `(doto* x (f a) (g b))` -> evaluates (f x a), (g x b), returns x
    fn compile_doto_star(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.is_empty() {
            return Err(self.error(span, "doto requires at least 1 argument"));
        }

        // Compile and store x in a local
        let x = &args[0];
        let forms = &args[1..];

        // Compile x
        self.compile_node(x, code)?;

        if forms.is_empty() {
            // Just return x
            return Ok(());
        }

        // Store x in a temp local
        let saved_next = self.next_local;
        let temp_slot = self.next_local;
        self.next_local += 1;
        code.emit(Opcode::StoreLocal(temp_slot));

        // Evaluate each form with x inserted as first arg
        for form in forms {
            match form {
                Ast::List(elements, form_span) if !elements.is_empty() => {
                    // (f a b) -> (f x a b)
                    // Compile function
                    self.compile_node(&elements[0], code)?;
                    // Load x
                    code.emit(Opcode::LoadLocal(temp_slot));
                    // Compile remaining args
                    for arg in &elements[1..] {
                        self.compile_node(arg, code)?;
                    }
                    // Call with x + other args
                    let arg_count = u16::try_from(elements.len())
                        .map_err(|_| self.error(*form_span, "too many arguments"))?;
                    code.emit(Opcode::Call(arg_count));
                    // Discard result
                    code.emit(Opcode::Pop);
                }
                Ast::Symbol(_, _form_span) => {
                    // f -> (f x)
                    self.compile_node(form, code)?;
                    code.emit(Opcode::LoadLocal(temp_slot));
                    code.emit(Opcode::Call(1));
                    code.emit(Opcode::Pop);
                }
                _ => {
                    return Err(self.error(span, "doto forms must be lists or symbols"));
                }
            }
        }

        // Load x to return it
        code.emit(Opcode::LoadLocal(temp_slot));

        // Restore local counter
        self.next_local = saved_next;

        Ok(())
    }

    // =========================================================================
    // Higher-Order Functions (map, filter, reduce)
    // =========================================================================

    /// Compiles `(map fn coll)` - apply function to each element.
    fn compile_hof_map(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "map requires exactly 2 arguments (fn coll)"));
        }

        // Compile function
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit Map opcode
        code.emit(Opcode::Map);

        Ok(())
    }

    /// Compiles `(filter fn coll)` - keep elements where fn returns truthy.
    fn compile_hof_filter(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "filter requires exactly 2 arguments (fn coll)"));
        }

        // Compile function (predicate)
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit Filter opcode
        code.emit(Opcode::Filter);

        Ok(())
    }

    /// Compiles `(reduce fn init coll)` or `(reduce fn coll)` - fold collection.
    fn compile_hof_reduce(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        match args.len() {
            2 => {
                // (reduce fn coll) - use first element as init, reduce rest
                // Compile function
                self.compile_node(&args[0], code)?;

                // Compile collection
                self.compile_node(&args[1], code)?;

                // Emit ReduceNoInit opcode (handles first/rest internally)
                code.emit(Opcode::ReduceNoInit);

                Ok(())
            }
            3 => {
                // (reduce fn init coll)
                // Compile function
                self.compile_node(&args[0], code)?;

                // Compile initial value
                self.compile_node(&args[1], code)?;

                // Compile collection
                self.compile_node(&args[2], code)?;

                // Emit Reduce opcode
                code.emit(Opcode::Reduce);

                Ok(())
            }
            _ => Err(self.error(span, "reduce requires 2 or 3 arguments (fn [init] coll)")),
        }
    }

    /// Compiles `(every? fn coll)` - check if all elements satisfy predicate.
    fn compile_hof_every(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "every? requires exactly 2 arguments (fn coll)"));
        }

        // Compile predicate function
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit Every opcode
        code.emit(Opcode::Every);

        Ok(())
    }

    /// Compiles `(some fn coll)` - check if any element satisfies predicate.
    fn compile_hof_some(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "some requires exactly 2 arguments (fn coll)"));
        }

        // Compile predicate function
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit Some opcode
        code.emit(Opcode::Some);

        Ok(())
    }

    /// Compiles `(take-while pred coll)` - take elements while predicate is true.
    fn compile_hof_take_while(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "take-while requires exactly 2 arguments (fn coll)"));
        }

        // Compile predicate function
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit TakeWhile opcode
        code.emit(Opcode::TakeWhile);

        Ok(())
    }

    /// Compiles `(drop-while pred coll)` - drop elements while predicate is true.
    fn compile_hof_drop_while(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "drop-while requires exactly 2 arguments (fn coll)"));
        }

        // Compile predicate function
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit DropWhile opcode
        code.emit(Opcode::DropWhile);

        Ok(())
    }

    /// Compiles `(remove pred coll)` - remove elements where predicate is true.
    fn compile_hof_remove(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "remove requires exactly 2 arguments (fn coll)"));
        }

        // Compile predicate function
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit Remove opcode
        code.emit(Opcode::Remove);

        Ok(())
    }

    /// Compiles `(group-by fn coll)` - group elements by key function.
    fn compile_hof_group_by(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "group-by requires exactly 2 arguments (fn coll)"));
        }

        // Compile key function
        self.compile_node(&args[0], code)?;

        // Compile collection
        self.compile_node(&args[1], code)?;

        // Emit GroupBy opcode
        code.emit(Opcode::GroupBy);

        Ok(())
    }

    /// Compiles `(zip-with fn coll1 coll2 ...)` - zip with combining function.
    fn compile_hof_zip_with(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() < 3 {
            return Err(self.error(
                span,
                "zip-with requires at least 3 arguments (fn coll1 coll2 ...)",
            ));
        }

        // Compile the combining function first
        self.compile_node(&args[0], code)?;

        // Compile all collections (as a vector of collections)
        code.emit(Opcode::VecNew);
        for arg in &args[1..] {
            self.compile_node(arg, code)?;
            code.emit(Opcode::VecPush);
        }

        // Emit ZipWith opcode
        code.emit(Opcode::ZipWith);

        Ok(())
    }

    /// Compiles `(repeatedly n fn)` - call zero-arg function N times.
    fn compile_hof_repeatedly(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(span, "repeatedly requires exactly 2 arguments (n fn)"));
        }

        // Compile count
        self.compile_node(&args[0], code)?;

        // Compile function
        self.compile_node(&args[1], code)?;

        // Emit Repeatedly opcode
        code.emit(Opcode::Repeatedly);

        Ok(())
    }

    // =========================================================================
    // World/Entity Operations
    // =========================================================================

    /// Compiles (get-component entity component-keyword) -> value
    fn compile_get_component(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(
                span,
                "get-component requires exactly 2 arguments (entity component)",
            ));
        }

        // Compile entity
        self.compile_node(&args[0], code)?;
        // Compile component keyword
        self.compile_node(&args[1], code)?;
        // Emit GetComponent opcode
        code.emit(Opcode::GetComponent);

        Ok(())
    }

    /// Compiles (get-field entity component-keyword field-keyword) -> value
    fn compile_get_field(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 3 {
            return Err(self.error(
                span,
                "get-field requires exactly 3 arguments (entity component field)",
            ));
        }

        // Compile entity
        self.compile_node(&args[0], code)?;
        // Compile component keyword
        self.compile_node(&args[1], code)?;
        // Compile field keyword
        self.compile_node(&args[2], code)?;
        // Emit GetField opcode
        code.emit(Opcode::GetField);

        Ok(())
    }

    /// Compiles (with-component component-keyword) -> [entities...]
    fn compile_with_component(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 1 {
            return Err(self.error(
                span,
                "with-component requires exactly 1 argument (component)",
            ));
        }

        // Compile component keyword
        self.compile_node(&args[0], code)?;
        // Emit WithComponent opcode
        code.emit(Opcode::WithComponent);

        Ok(())
    }

    /// Compiles (find-relationships rel-type-or-nil source-or-nil target-or-nil) -> [relationship-entities...]
    fn compile_find_relationships(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 3 {
            return Err(self.error(
                span,
                "find-relationships requires exactly 3 arguments (rel-type source target)",
            ));
        }

        // Compile rel-type (or nil)
        self.compile_node(&args[0], code)?;
        // Compile source (or nil)
        self.compile_node(&args[1], code)?;
        // Compile target (or nil)
        self.compile_node(&args[2], code)?;
        // Emit FindRelationships opcode
        code.emit(Opcode::FindRelationships);

        Ok(())
    }

    /// Compiles (targets source rel-type) -> [target-entities...]
    fn compile_targets(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(
                span,
                "targets requires exactly 2 arguments (source rel-type)",
            ));
        }

        // Compile source entity
        self.compile_node(&args[0], code)?;
        // Compile relationship type keyword
        self.compile_node(&args[1], code)?;
        // Emit Targets opcode
        code.emit(Opcode::Targets);

        Ok(())
    }

    /// Compiles (sources target rel-type) -> [source-entities...]
    fn compile_sources(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(
                span,
                "sources requires exactly 2 arguments (target rel-type)",
            ));
        }

        // Compile target entity
        self.compile_node(&args[0], code)?;
        // Compile relationship type keyword
        self.compile_node(&args[1], code)?;
        // Emit Sources opcode
        code.emit(Opcode::Sources);

        Ok(())
    }

    /// Compiles (entity-ref index generation) -> `EntityRef` value
    ///
    /// The index and generation must be integer literals.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn compile_entity_ref(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 2 {
            return Err(self.error(
                span,
                "entity-ref requires exactly 2 arguments (index generation)",
            ));
        }

        // Extract index and generation as integer literals
        let index = match &args[0] {
            Ast::Int(n, _) => *n as u64,
            _ => {
                return Err(self.error(span, "entity-ref index must be an integer literal"));
            }
        };

        let generation = match &args[1] {
            Ast::Int(n, _) => *n as u32,
            _ => {
                return Err(self.error(span, "entity-ref generation must be an integer literal"));
            }
        };

        // Create the EntityId and add as constant
        let entity = longtable_foundation::EntityId::new(index, generation);
        let idx = self.add_constant(Value::EntityRef(entity));
        code.emit(Opcode::Const(idx));

        Ok(())
    }

    // =========================================================================
    // World Mutation Operations
    // =========================================================================

    /// Compiles (spawn components-map) -> entity-id
    ///
    /// Creates a new entity with the given components.
    fn compile_spawn(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 1 {
            return Err(self.error(span, "spawn requires exactly 1 argument (components-map)"));
        }

        // Compile the components map
        self.compile_node(&args[0], code)?;
        // Emit Spawn opcode
        code.emit(Opcode::Spawn);

        Ok(())
    }

    /// Compiles (destroy entity) -> nil
    ///
    /// Destroys an entity.
    fn compile_destroy(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 1 {
            return Err(self.error(span, "destroy requires exactly 1 argument (entity)"));
        }

        // Compile the entity
        self.compile_node(&args[0], code)?;
        // Emit Destroy opcode
        code.emit(Opcode::Destroy);
        // Destroy returns nil
        let idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(idx));

        Ok(())
    }

    /// Compiles (set-component entity component-kw value) -> nil
    ///
    /// Sets a component value on an entity.
    fn compile_set_component(
        &mut self,
        args: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        if args.len() != 3 {
            return Err(self.error(
                span,
                "set-component requires exactly 3 arguments (entity component value)",
            ));
        }

        // Compile entity
        self.compile_node(&args[0], code)?;
        // Compile component keyword
        self.compile_node(&args[1], code)?;
        // Compile value
        self.compile_node(&args[2], code)?;
        // Emit SetComponent opcode
        code.emit(Opcode::SetComponent);
        // SetComponent returns nil
        let idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(idx));

        Ok(())
    }

    /// Compiles (set-field entity component-kw field-kw value) -> nil
    ///
    /// Sets a field value within a component on an entity.
    fn compile_set_field(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 4 {
            return Err(self.error(
                span,
                "set-field requires exactly 4 arguments (entity component field value)",
            ));
        }

        // Compile entity
        self.compile_node(&args[0], code)?;
        // Compile component keyword
        self.compile_node(&args[1], code)?;
        // Compile field keyword
        self.compile_node(&args[2], code)?;
        // Compile value
        self.compile_node(&args[3], code)?;
        // Emit SetField opcode
        code.emit(Opcode::SetField);
        // SetField returns nil
        let idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(idx));

        Ok(())
    }

    /// Compiles (link source rel-kw target) -> nil
    ///
    /// Creates a relationship between two entities.
    fn compile_link(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 3 {
            return Err(self.error(
                span,
                "link requires exactly 3 arguments (source rel-type target)",
            ));
        }

        // Compile source entity
        self.compile_node(&args[0], code)?;
        // Compile relationship type keyword
        self.compile_node(&args[1], code)?;
        // Compile target entity
        self.compile_node(&args[2], code)?;
        // Emit Link opcode
        code.emit(Opcode::Link);
        // Link returns nil
        let idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(idx));

        Ok(())
    }

    /// Compiles (unlink source rel-kw target) -> nil
    ///
    /// Removes a relationship between two entities.
    fn compile_unlink(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 3 {
            return Err(self.error(
                span,
                "unlink requires exactly 3 arguments (source rel-type target)",
            ));
        }

        // Compile source entity
        self.compile_node(&args[0], code)?;
        // Compile relationship type keyword
        self.compile_node(&args[1], code)?;
        // Compile target entity
        self.compile_node(&args[2], code)?;
        // Emit Unlink opcode
        code.emit(Opcode::Unlink);
        // Unlink returns nil
        let idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(idx));

        Ok(())
    }

    // =========================================================================
    // Declaration Compilation (to registration opcodes)
    // =========================================================================

    /// Compiles a `component:` declaration.
    ///
    /// Transforms `(component: name ...)` into a data map and emits `RegisterComponent`.
    fn compile_component_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        // Reconstruct the AST for the analyzer
        let ast = Ast::List(elements.to_vec(), span);

        // Analyze to extract typed declaration
        let decl = DeclarationAnalyzer::analyze_component(&ast)?
            .ok_or_else(|| self.error(span, "invalid component: declaration"))?;

        // Convert to Value map for the VM
        let map = self.component_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterComponent);

        // Declaration returns nil
        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a relationship: declaration.
    fn compile_relationship_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_relationship(&ast)?
            .ok_or_else(|| self.error(span, "invalid relationship: declaration"))?;

        let map = self.relationship_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterRelationship);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a verb: declaration.
    fn compile_verb_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_verb(&ast)?
            .ok_or_else(|| self.error(span, "invalid verb: declaration"))?;

        let map = self.verb_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterVerb);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a direction: declaration.
    fn compile_direction_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_direction(&ast)?
            .ok_or_else(|| self.error(span, "invalid direction: declaration"))?;

        let map = self.direction_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterDirection);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a preposition: declaration.
    fn compile_preposition_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_preposition(&ast)?
            .ok_or_else(|| self.error(span, "invalid preposition: declaration"))?;

        let map = self.preposition_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterPreposition);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a pronoun: declaration.
    fn compile_pronoun_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_pronoun(&ast)?
            .ok_or_else(|| self.error(span, "invalid pronoun: declaration"))?;

        let map = self.pronoun_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterPronoun);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles an adverb: declaration.
    fn compile_adverb_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_adverb(&ast)?
            .ok_or_else(|| self.error(span, "invalid adverb: declaration"))?;

        let map = self.adverb_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterAdverb);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a type: declaration.
    fn compile_type_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_noun_type(&ast)?
            .ok_or_else(|| self.error(span, "invalid type: declaration"))?;

        let map = self.noun_type_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterType);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a scope: declaration.
    fn compile_scope_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_scope(&ast)?
            .ok_or_else(|| self.error(span, "invalid scope: declaration"))?;

        let map = self.scope_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterScope);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a command: declaration.
    fn compile_command_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_command(&ast)?
            .ok_or_else(|| self.error(span, "invalid command: declaration"))?;

        let map = self.command_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterCommand);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles an action: declaration.
    fn compile_action_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_action(&ast)?
            .ok_or_else(|| self.error(span, "invalid action: declaration"))?;

        let map = self.action_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterAction);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    /// Compiles a rule: declaration.
    fn compile_rule_decl(
        &mut self,
        elements: &[Ast],
        span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        let ast = Ast::List(elements.to_vec(), span);
        let decl = DeclarationAnalyzer::analyze_rule(&ast)?
            .ok_or_else(|| self.error(span, "invalid rule: declaration"))?;

        let map = self.rule_decl_to_value(&decl)?;
        let idx = self.add_constant(map);
        code.emit(Opcode::Const(idx));
        code.emit(Opcode::RegisterRule);
        // RegisterRule pushes entity ID, but for now we discard it
        code.emit(Opcode::Pop);

        let nil_idx = self.add_constant(Value::Nil);
        code.emit(Opcode::Const(nil_idx));

        Ok(())
    }

    // =========================================================================
    // Declaration to Value conversions
    // =========================================================================

    /// Converts a `ComponentDecl` to a Value map.
    fn component_decl_to_value(
        &mut self,
        decl: &crate::declaration::ComponentDecl,
    ) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        // :name -> keyword
        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        // :storage -> "tag" or "sparse"
        let storage_key = self.intern_keyword("storage");
        let storage_val = if decl.is_tag { "tag" } else { "sparse" };
        map = map.insert(
            Value::Keyword(storage_key),
            Value::String(storage_val.into()),
        );

        // :fields -> vector of field maps
        if !decl.fields.is_empty() {
            let fields_key = self.intern_keyword("fields");
            let mut fields_vec: LtVec<Value> = LtVec::new();
            for field in &decl.fields {
                let field_map = self.field_decl_to_value(field)?;
                fields_vec = fields_vec.push_back(field_map);
            }
            map = map.insert(Value::Keyword(fields_key), Value::Vec(fields_vec));
        }

        Ok(Value::Map(map))
    }

    /// Converts a `FieldDecl` to a Value map.
    fn field_decl_to_value(&mut self, decl: &crate::declaration::FieldDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        let type_key = self.intern_keyword("type");
        map = map.insert(
            Value::Keyword(type_key),
            Value::String(decl.ty.clone().into()),
        );

        Ok(Value::Map(map))
    }

    /// Converts a `RelationshipDecl` to a Value map.
    fn relationship_decl_to_value(
        &mut self,
        decl: &crate::declaration::RelationshipDecl,
    ) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        let card_key = self.intern_keyword("cardinality");
        let card_str = match decl.cardinality {
            crate::declaration::Cardinality::OneToOne => "one-to-one",
            crate::declaration::Cardinality::OneToMany => "one-to-many",
            crate::declaration::Cardinality::ManyToOne => "many-to-one",
            crate::declaration::Cardinality::ManyToMany => "many-to-many",
        };
        map = map.insert(Value::Keyword(card_key), Value::String(card_str.into()));

        let on_delete_key = self.intern_keyword("on-delete");
        let on_delete_str = match decl.on_target_delete {
            crate::declaration::OnTargetDelete::Remove => "remove",
            crate::declaration::OnTargetDelete::Cascade => "cascade",
            crate::declaration::OnTargetDelete::Nullify => "nullify",
        };
        map = map.insert(
            Value::Keyword(on_delete_key),
            Value::String(on_delete_str.into()),
        );

        Ok(Value::Map(map))
    }

    /// Converts a `VerbDecl` to a Value map.
    fn verb_decl_to_value(&mut self, decl: &crate::declaration::VerbDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        if !decl.synonyms.is_empty() {
            let synonyms_key = self.intern_keyword("synonyms");
            let mut syn_vec: LtVec<Value> = LtVec::new();
            for syn in &decl.synonyms {
                let syn_kw = self.intern_keyword(syn);
                syn_vec = syn_vec.push_back(Value::Keyword(syn_kw));
            }
            map = map.insert(Value::Keyword(synonyms_key), Value::Vec(syn_vec));
        }

        Ok(Value::Map(map))
    }

    /// Converts a `DirectionDecl` to a Value map.
    fn direction_decl_to_value(
        &mut self,
        decl: &crate::declaration::DirectionDecl,
    ) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        if !decl.synonyms.is_empty() {
            let synonyms_key = self.intern_keyword("synonyms");
            let mut syn_vec: LtVec<Value> = LtVec::new();
            for syn in &decl.synonyms {
                let syn_kw = self.intern_keyword(syn);
                syn_vec = syn_vec.push_back(Value::Keyword(syn_kw));
            }
            map = map.insert(Value::Keyword(synonyms_key), Value::Vec(syn_vec));
        }

        if let Some(ref opp) = decl.opposite {
            let opposite_key = self.intern_keyword("opposite");
            let opposite_val = self.intern_keyword(opp);
            map = map.insert(Value::Keyword(opposite_key), Value::Keyword(opposite_val));
        }

        Ok(Value::Map(map))
    }

    /// Converts a `PrepositionDecl` to a Value map.
    fn preposition_decl_to_value(
        &mut self,
        decl: &crate::declaration::PrepositionDecl,
    ) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        if let Some(ref implies) = decl.implies {
            let implies_key = self.intern_keyword("implies");
            let implies_val = self.intern_keyword(implies);
            map = map.insert(Value::Keyword(implies_key), Value::Keyword(implies_val));
        }

        Ok(Value::Map(map))
    }

    /// Converts a `PronounDecl` to a Value map.
    fn pronoun_decl_to_value(&mut self, decl: &crate::declaration::PronounDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        let gender_key = self.intern_keyword("gender");
        let gender_str = match decl.gender {
            crate::declaration::PronounGender::Masculine => "masculine",
            crate::declaration::PronounGender::Feminine => "feminine",
            crate::declaration::PronounGender::Neuter => "neuter",
        };
        map = map.insert(Value::Keyword(gender_key), Value::String(gender_str.into()));

        let number_key = self.intern_keyword("number");
        let number_str = match decl.number {
            crate::declaration::PronounNumber::Singular => "singular",
            crate::declaration::PronounNumber::Plural => "plural",
        };
        map = map.insert(Value::Keyword(number_key), Value::String(number_str.into()));

        Ok(Value::Map(map))
    }

    /// Converts an `AdverbDecl` to a Value map.
    fn adverb_decl_to_value(&mut self, decl: &crate::declaration::AdverbDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        Ok(Value::Map(map))
    }

    /// Converts a `NounTypeDecl` to a Value map.
    fn noun_type_decl_to_value(
        &mut self,
        decl: &crate::declaration::NounTypeDecl,
    ) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        if !decl.extends.is_empty() {
            let extends_key = self.intern_keyword("extends");
            let mut ext_vec: LtVec<Value> = LtVec::new();
            for ext in &decl.extends {
                let ext_kw = self.intern_keyword(ext);
                ext_vec = ext_vec.push_back(Value::Keyword(ext_kw));
            }
            map = map.insert(Value::Keyword(extends_key), Value::Vec(ext_vec));
        }

        Ok(Value::Map(map))
    }

    /// Converts a `ScopeDecl` to a Value map.
    fn scope_decl_to_value(&mut self, decl: &crate::declaration::ScopeDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        Ok(Value::Map(map))
    }

    /// Converts a `CommandDecl` to a Value map.
    fn command_decl_to_value(&mut self, decl: &crate::declaration::CommandDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        let action_key = self.intern_keyword("action");
        let action_val = self.intern_keyword(&decl.action);
        map = map.insert(Value::Keyword(action_key), Value::Keyword(action_val));

        let priority_key = self.intern_keyword("priority");
        map = map.insert(
            Value::Keyword(priority_key),
            Value::Int(i64::from(decl.priority)),
        );

        Ok(Value::Map(map))
    }

    /// Converts an `ActionDecl` to a Value map.
    fn action_decl_to_value(&mut self, decl: &crate::declaration::ActionDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        // For now, store minimal info. Full action compilation is more complex.
        Ok(Value::Map(map))
    }

    /// Converts a `RuleDecl` to a Value map.
    fn rule_decl_to_value(&mut self, decl: &crate::declaration::RuleDecl) -> Result<Value> {
        let mut map: LtMap<Value, Value> = LtMap::new();

        let name_key = self.intern_keyword("name");
        let name_val = self.intern_keyword(&decl.name);
        map = map.insert(Value::Keyword(name_key), Value::Keyword(name_val));

        let salience_key = self.intern_keyword("salience");
        map = map.insert(
            Value::Keyword(salience_key),
            Value::Int(i64::from(decl.salience)),
        );

        let once_key = self.intern_keyword("once");
        map = map.insert(Value::Keyword(once_key), Value::Bool(decl.once));

        // For now, store minimal info. Full rule compilation is more complex.
        Ok(Value::Map(map))
    }

    /// Interns a keyword, using the interner if available.
    ///
    /// # Panics
    /// Panics if no interner is set. Declaration compilation requires an interner.
    fn intern_keyword(&mut self, name: &str) -> KeywordId {
        self.interner
            .as_mut()
            .expect("intern_keyword requires an interner for declaration compilation")
            .intern_keyword(name)
    }

    /// Compiles a quoted expression (as data, not evaluated).
    fn compile_quoted(&mut self, ast: &Ast, code: &mut Bytecode) -> Result<()> {
        // Convert AST to runtime value
        let value = self.ast_to_value(ast)?;
        let idx = self.add_constant(value);
        code.emit(Opcode::Const(idx));
        Ok(())
    }

    /// Converts an AST node to a runtime Value.
    fn ast_to_value(&self, ast: &Ast) -> Result<Value> {
        Ok(match ast {
            Ast::Nil(_) => Value::Nil,
            Ast::Bool(b, _) => Value::Bool(*b),
            Ast::Int(n, _) => Value::Int(*n),
            Ast::Float(n, _) => Value::Float(*n),
            Ast::String(s, _) => Value::String(s.as_str().into()),
            Ast::Symbol(s, _) => Value::String(format!("'{s}").into()),
            Ast::Keyword(s, _) => Value::String(format!(":{s}").into()),
            Ast::List(elements, _) => {
                let items: Result<Vec<_>> = elements.iter().map(|e| self.ast_to_value(e)).collect();
                Value::Vec(items?.into_iter().collect())
            }
            Ast::Vector(elements, _) => {
                let items: Result<Vec<_>> = elements.iter().map(|e| self.ast_to_value(e)).collect();
                Value::Vec(items?.into_iter().collect())
            }
            Ast::Set(elements, _) => {
                let items: Result<Vec<_>> = elements.iter().map(|e| self.ast_to_value(e)).collect();
                Value::Set(items?.into_iter().collect())
            }
            Ast::Map(entries, _) => {
                let mut map = longtable_foundation::LtMap::new();
                for (k, v) in entries {
                    let key = self.ast_to_value(k)?;
                    let val = self.ast_to_value(v)?;
                    map = map.insert(key, val);
                }
                Value::Map(map)
            }
            Ast::Quote(inner, _) => self.ast_to_value(inner)?,
            Ast::Unquote(_, span) => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: "unquote outside syntax-quote".to_string(),
                    line: span.line,
                    column: span.column,
                    context: String::new(),
                }));
            }
            Ast::UnquoteSplice(_, span) => {
                return Err(Error::new(ErrorKind::ParseError {
                    message: "unquote-splice outside syntax-quote".to_string(),
                    line: span.line,
                    column: span.column,
                    context: String::new(),
                }));
            }
            Ast::SyntaxQuote(inner, _) => self.ast_to_value(inner)?,
            Ast::Tagged(tag, inner, _) => {
                // Store tagged values as a vector [tag, value]
                let inner_val = self.ast_to_value(inner)?;
                Value::Vec(
                    vec![Value::String(format!("#{tag}").into()), inner_val]
                        .into_iter()
                        .collect(),
                )
            }
        })
    }

    /// Compiles a vector literal.
    fn compile_vector(&mut self, elements: &[Ast], code: &mut Bytecode) -> Result<()> {
        code.emit(Opcode::VecNew);
        for elem in elements {
            self.compile_node(elem, code)?;
            code.emit(Opcode::VecPush);
        }
        Ok(())
    }

    /// Compiles a set literal.
    fn compile_set(&mut self, elements: &[Ast], code: &mut Bytecode) -> Result<()> {
        code.emit(Opcode::SetNew);
        for elem in elements {
            self.compile_node(elem, code)?;
            code.emit(Opcode::SetInsert);
        }
        Ok(())
    }

    /// Compiles a map literal.
    fn compile_map(&mut self, entries: &[(Ast, Ast)], code: &mut Bytecode) -> Result<()> {
        code.emit(Opcode::MapNew);
        for (key, value) in entries {
            self.compile_node(key, code)?;
            self.compile_node(value, code)?;
            code.emit(Opcode::MapInsert);
        }
        Ok(())
    }

    /// Compiles a tagged literal.
    fn compile_tagged(
        &mut self,
        _tag: &str,
        inner: &Ast,
        _span: Span,
        code: &mut Bytecode,
    ) -> Result<()> {
        // For now, just compile as the inner value
        // Full tagged literal support would dispatch based on tag
        self.compile_node(inner, code)
    }

    /// Adds a constant to the pool and returns its index.
    fn add_constant(&mut self, value: Value) -> u16 {
        // Try to deduplicate
        if let Some(key) = ConstKey::from_value(&value) {
            if let Some(&idx) = self.constant_map.get(&key) {
                return idx;
            }
            let idx = self.constants.len() as u16;
            self.constant_map.insert(key, idx);
            self.constants.push(value);
            idx
        } else {
            let idx = self.constants.len() as u16;
            self.constants.push(value);
            idx
        }
    }

    /// Creates a compile error.
    fn error(&self, span: Span, message: &str) -> Error {
        Error::new(ErrorKind::ParseError {
            message: message.to_string(),
            line: span.line,
            column: span.column,
            context: String::new(),
        })
    }
}

/// Compiles source code to a program with stdlib macros.
pub fn compile(source: &str) -> Result<CompiledProgram> {
    let ast = crate::parser::parse(source)?;
    let mut compiler = Compiler::new_with_stdlib();
    compiler.compile(&ast)
}

/// Compiles a single expression with stdlib macros.
pub fn compile_expr(source: &str) -> Result<Bytecode> {
    let ast = crate::parser::parse_one(source)?;
    let mut compiler = Compiler::new_with_stdlib();
    compiler.compile_expr(&ast)
}

/// Compiles source code to a program without stdlib macros.
pub fn compile_without_stdlib(source: &str) -> Result<CompiledProgram> {
    let ast = crate::parser::parse(source)?;
    let mut compiler = Compiler::new();
    compiler.compile(&ast)
}

/// A compiled expression with its constants.
#[derive(Clone, Debug)]
pub struct CompiledExpr {
    /// The bytecode for this expression.
    pub code: Bytecode,
    /// Constants referenced by the bytecode.
    pub constants: Vec<Value>,
}

/// Compiles an AST expression with predefined binding variables.
///
/// Variables in `binding_vars` will be compiled to `LoadBinding(idx)` opcodes
/// rather than being treated as undefined symbols.
///
/// This is used for query compilation where pattern-matched variables need
/// to be accessible in guard, return, and aggregate expressions.
pub fn compile_expression(ast: &Ast, binding_vars: &[String]) -> Result<CompiledExpr> {
    let mut compiler = Compiler::new();
    // Pre-populate locals with binding vars - they'll be treated as locals
    // but we'll post-process to convert LoadLocal to LoadBinding
    // Register both with and without ? prefix to support pattern variable syntax
    for (idx, var) in binding_vars.iter().enumerate() {
        compiler.locals.insert(var.clone(), idx as u16);
        // Also register ?var for pattern variable syntax in return expressions
        compiler.locals.insert(format!("?{var}"), idx as u16);
    }
    compiler.next_local = binding_vars.len() as u16;

    let mut code = Bytecode::new();
    compiler.compile_node(ast, &mut code)?;

    // Convert LoadLocal(n) to LoadBinding(n) for binding vars
    let binding_count = binding_vars.len() as u16;
    for op in &mut code.ops {
        if let Opcode::LoadLocal(slot) = op {
            if *slot < binding_count {
                *op = Opcode::LoadBinding(*slot);
            }
        }
    }

    Ok(CompiledExpr {
        code,
        constants: compiler.constants,
    })
}

/// Compiles an AST expression with predefined binding variables and an interner.
///
/// This is the preferred version when you have access to an interner, as it
/// properly interns keywords as `Value::Keyword(KeywordId)` instead of strings.
///
/// Variables in `binding_vars` will be compiled to `LoadBinding(idx)` opcodes
/// rather than being treated as undefined symbols.
pub fn compile_expression_with_interner(
    ast: &Ast,
    binding_vars: &[String],
    interner: Interner,
) -> Result<CompiledExpr> {
    let mut compiler = Compiler::with_interner(interner);
    // Pre-populate locals with binding vars - they'll be treated as locals
    // but we'll post-process to convert LoadLocal to LoadBinding
    // Register both with and without ? prefix to support pattern variable syntax
    for (idx, var) in binding_vars.iter().enumerate() {
        compiler.locals.insert(var.clone(), idx as u16);
        // Also register ?var for pattern variable syntax in return expressions
        compiler.locals.insert(format!("?{var}"), idx as u16);
    }
    compiler.next_local = binding_vars.len() as u16;

    let mut code = Bytecode::new();
    compiler.compile_node(ast, &mut code)?;

    // Convert LoadLocal(n) to LoadBinding(n) for binding vars
    let binding_count = binding_vars.len() as u16;
    for op in &mut code.ops {
        if let Opcode::LoadLocal(slot) = op {
            if *slot < binding_count {
                *op = Opcode::LoadBinding(*slot);
            }
        }
    }

    Ok(CompiledExpr {
        code,
        constants: compiler.constants,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_test(source: &str) -> CompiledProgram {
        compile(source).expect("compile failed")
    }

    #[test]
    fn compile_nil() {
        let prog = compile_test("nil");
        assert_eq!(prog.code.ops.len(), 1);
        assert!(matches!(prog.code.ops[0], Opcode::Const(_)));
        assert_eq!(prog.constants[0], Value::Nil);
    }

    #[test]
    fn compile_bool() {
        let prog = compile_test("true");
        assert_eq!(prog.constants[0], Value::Bool(true));

        let prog = compile_test("false");
        assert_eq!(prog.constants[0], Value::Bool(false));
    }

    #[test]
    fn compile_int() {
        let prog = compile_test("42");
        assert_eq!(prog.constants[0], Value::Int(42));
    }

    #[test]
    fn compile_float() {
        let prog = compile_test("2.5");
        assert!(matches!(prog.constants[0], Value::Float(f) if (f - 2.5).abs() < 0.001));
    }

    #[test]
    fn compile_string() {
        let prog = compile_test(r#""hello""#);
        assert_eq!(prog.constants[0], Value::String("hello".into()));
    }

    #[test]
    fn compile_addition() {
        let prog = compile_test("(+ 1 2)");
        // Should have: Const(0), Const(1), Add
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::Add)));
    }

    #[test]
    fn compile_nested_arithmetic() {
        let prog = compile_test("(+ (* 2 3) (- 10 5))");
        // Should contain Add, Mul, Sub
        let has_add = prog.code.ops.iter().any(|op| matches!(op, Opcode::Add));
        let has_mul = prog.code.ops.iter().any(|op| matches!(op, Opcode::Mul));
        let has_sub = prog.code.ops.iter().any(|op| matches!(op, Opcode::Sub));
        assert!(has_add && has_mul && has_sub);
    }

    #[test]
    fn compile_comparison() {
        let prog = compile_test("(< 1 2)");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::Lt)));
    }

    #[test]
    fn compile_if_then_else() {
        let prog = compile_test("(if true 1 2)");
        // Should have JumpIfNot and Jump
        let has_jump_if_not = prog
            .code
            .ops
            .iter()
            .any(|op| matches!(op, Opcode::JumpIfNot(_)));
        let has_jump = prog.code.ops.iter().any(|op| matches!(op, Opcode::Jump(_)));
        assert!(has_jump_if_not && has_jump);
    }

    #[test]
    fn compile_if_without_else() {
        let prog = compile_test("(if true 1)");
        // Should still compile with nil for else case
        let has_jump_if_not = prog
            .code
            .ops
            .iter()
            .any(|op| matches!(op, Opcode::JumpIfNot(_)));
        assert!(has_jump_if_not);
    }

    #[test]
    fn compile_let() {
        let prog = compile_test("(let [x 1] x)");
        // Should have StoreLocal and LoadLocal
        let has_store = prog
            .code
            .ops
            .iter()
            .any(|op| matches!(op, Opcode::StoreLocal(_)));
        let has_load = prog
            .code
            .ops
            .iter()
            .any(|op| matches!(op, Opcode::LoadLocal(_)));
        assert!(has_store && has_load);
    }

    #[test]
    fn compile_let_multiple_bindings() {
        let prog = compile_test("(let [x 1 y 2] (+ x y))");
        let store_count = prog
            .code
            .ops
            .iter()
            .filter(|op| matches!(op, Opcode::StoreLocal(_)))
            .count();
        assert_eq!(store_count, 2);
    }

    #[test]
    fn compile_do() {
        let prog = compile_test("(do 1 2 3)");
        // Should have Pop between expressions
        let pop_count = prog
            .code
            .ops
            .iter()
            .filter(|op| matches!(op, Opcode::Pop))
            .count();
        assert_eq!(pop_count, 2); // Two pops for intermediate results
    }

    #[test]
    fn compile_vector() {
        let prog = compile_test("[1 2 3]");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::VecNew)));
        let push_count = prog
            .code
            .ops
            .iter()
            .filter(|op| matches!(op, Opcode::VecPush))
            .count();
        assert_eq!(push_count, 3);
    }

    #[test]
    fn compile_map() {
        let prog = compile_test("{:a 1 :b 2}");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::MapNew)));
        let insert_count = prog
            .code
            .ops
            .iter()
            .filter(|op| matches!(op, Opcode::MapInsert))
            .count();
        assert_eq!(insert_count, 2);
    }

    #[test]
    fn compile_set() {
        let prog = compile_test("#{1 2 3}");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::SetNew)));
        let insert_count = prog
            .code
            .ops
            .iter()
            .filter(|op| matches!(op, Opcode::SetInsert))
            .count();
        assert_eq!(insert_count, 3);
    }

    #[test]
    fn compile_constant_deduplication() {
        let prog = compile_test("(+ 42 42)");
        // Same constant should be deduplicated
        let const_42_count = prog
            .constants
            .iter()
            .filter(|c| **c == Value::Int(42))
            .count();
        assert_eq!(const_42_count, 1);
    }

    #[test]
    fn compile_chained_addition() {
        let prog = compile_test("(+ 1 2 3 4)");
        // Should have 3 Add operations for chaining
        let add_count = prog
            .code
            .ops
            .iter()
            .filter(|op| matches!(op, Opcode::Add))
            .count();
        assert_eq!(add_count, 3);
    }

    // =========================================================================
    // Namespace-aware compilation tests
    // =========================================================================

    #[test]
    fn compile_with_namespace_alias() {
        use crate::namespace::NamespaceContext;

        let mut ns_ctx = NamespaceContext::new();
        ns_ctx.add_alias("core", "game.core");

        let mut compiler = Compiler::with_namespace_context(ns_ctx);
        let ast = crate::parse("core/foo").unwrap();
        let prog = compiler.compile(&ast).unwrap();

        // Should emit a constant with the resolved qualified name
        let has_qualified = prog.constants.iter().any(|c| {
            if let Value::String(s) = c {
                s.to_string() == "'game.core/foo"
            } else {
                false
            }
        });
        assert!(has_qualified, "Should resolve alias to qualified name");
    }

    #[test]
    fn compile_with_referred_symbol() {
        use crate::namespace::NamespaceContext;

        let mut ns_ctx = NamespaceContext::new();
        ns_ctx.add_refer("distance", "game.utils/distance");

        let mut compiler = Compiler::with_namespace_context(ns_ctx);
        let ast = crate::parse("distance").unwrap();
        let prog = compiler.compile(&ast).unwrap();

        // Should emit a constant with the qualified name from refers
        let has_qualified = prog.constants.iter().any(|c| {
            if let Value::String(s) = c {
                s.to_string() == "'game.utils/distance"
            } else {
                false
            }
        });
        assert!(
            has_qualified,
            "Should resolve referred symbol to qualified name"
        );
    }

    #[test]
    fn compile_fully_qualified_symbol() {
        // Already fully qualified symbols should pass through
        let prog = compile_test("game.core/foo");

        let has_qualified = prog.constants.iter().any(|c| {
            if let Value::String(s) = c {
                s.to_string() == "'game.core/foo"
            } else {
                false
            }
        });
        assert!(has_qualified, "Fully qualified symbol should be preserved");
    }

    #[test]
    fn compile_with_macro_expansion() {
        // Test that macros are expanded during compilation
        let mut compiler = Compiler::new();

        // First, define a macro through expansion
        let def_ast = crate::parse("(defmacro double [x] (+ x x))").unwrap();
        // Compile the defmacro - this registers the macro
        let _ = compiler.compile(&def_ast).unwrap();

        // Now use the macro - it should be expanded during compilation
        let use_ast = crate::parse("(double 21)").unwrap();
        let prog = compiler.compile(&use_ast).unwrap();

        // The expanded form (+ 21 21) should have two constants: 21 and the native + index
        // The key test is that it compiled without error, meaning the macro was expanded
        assert!(
            !prog.code.ops.is_empty(),
            "Macro expansion and compilation should succeed"
        );
    }

    // =========================================================================
    // fn: declaration tests
    // =========================================================================

    #[test]
    fn compile_fn_decl_value() {
        let mut compiler = Compiler::new();

        // Define a global value
        let ast = crate::parse("(fn: answer 42)").unwrap();
        let prog = compiler.compile(&ast).unwrap();

        // Should have StoreGlobal and LoadGlobal
        let has_store = prog
            .code
            .ops
            .iter()
            .any(|op| matches!(op, Opcode::StoreGlobal(_)));
        let has_load = prog
            .code
            .ops
            .iter()
            .any(|op| matches!(op, Opcode::LoadGlobal(_)));
        assert!(has_store, "Should store to global");
        assert!(has_load, "Should load from global (return value)");

        // The global should be registered in the compiler
        assert!(compiler.globals.contains_key("answer"));
    }

    #[test]
    fn compile_fn_decl_function() {
        let mut compiler = Compiler::new();

        // Define a global function
        let ast = crate::parse("(fn: double [x] (* x 2))").unwrap();
        let prog = compiler.compile(&ast).unwrap();

        // Should have compiled a function
        assert_eq!(prog.functions.len(), 1, "Should compile one function");
        assert_eq!(
            prog.functions[0].arity, 1,
            "Function should take 1 argument"
        );

        // The global should be registered
        assert!(compiler.globals.contains_key("double"));
    }

    #[test]
    fn compile_fn_decl_with_docstring() {
        let mut compiler = Compiler::new();

        // Define a function with docstring
        let ast = crate::parse(r#"(fn: add "Adds two numbers" [a b] (+ a b))"#).unwrap();
        let prog = compiler.compile(&ast).unwrap();

        // Should have compiled a function
        assert_eq!(prog.functions.len(), 1, "Should compile one function");
        assert_eq!(
            prog.functions[0].arity, 2,
            "Function should take 2 arguments"
        );

        // The global should be registered
        assert!(compiler.globals.contains_key("add"));
    }

    #[test]
    fn compile_fn_decl_reference() {
        let mut compiler = Compiler::new();

        // Define a global and then reference it
        let ast1 = crate::parse("(fn: x 10)").unwrap();
        let _ = compiler.compile(&ast1).unwrap();

        // Reference the global - need to prepare for new compilation
        compiler.prepare_for_compilation();
        let ast2 = crate::parse("x").unwrap();
        let prog = compiler.compile(&ast2).unwrap();

        // Should have LoadGlobal
        let has_load = prog
            .code
            .ops
            .iter()
            .any(|op| matches!(op, Opcode::LoadGlobal(0)));
        assert!(has_load, "Should load global x");
    }

    #[test]
    fn compile_fn_decl_persists_across_compilations() {
        let mut compiler = Compiler::new();

        // Define multiple globals across compilations
        let ast1 = crate::parse("(fn: a 1)").unwrap();
        let _ = compiler.compile(&ast1).unwrap();

        compiler.prepare_for_compilation();
        let ast2 = crate::parse("(fn: b 2)").unwrap();
        let _ = compiler.compile(&ast2).unwrap();

        // Both globals should be registered
        assert!(compiler.globals.contains_key("a"));
        assert!(compiler.globals.contains_key("b"));

        // They should have different slots
        assert_ne!(compiler.globals.get("a"), compiler.globals.get("b"));
    }

    // =========================================================================
    // World mutation operation tests
    // =========================================================================

    #[test]
    fn compile_spawn() {
        let prog = compile_test("(spawn {:name \"test\"})");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::Spawn)));
    }

    #[test]
    fn compile_destroy() {
        let prog = compile_test("(destroy (entity-ref 1 0))");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::Destroy)));
    }

    #[test]
    fn compile_set_component() {
        let prog = compile_test("(set-component (entity-ref 1 0) :name \"test\")");
        assert!(
            prog.code
                .ops
                .iter()
                .any(|op| matches!(op, Opcode::SetComponent))
        );
    }

    #[test]
    fn compile_set_field() {
        let prog = compile_test("(set-field (entity-ref 1 0) :position :x 10)");
        assert!(
            prog.code
                .ops
                .iter()
                .any(|op| matches!(op, Opcode::SetField))
        );
    }

    #[test]
    fn compile_link() {
        let prog = compile_test("(link (entity-ref 1 0) :contains (entity-ref 2 0))");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::Link)));
    }

    #[test]
    fn compile_unlink() {
        let prog = compile_test("(unlink (entity-ref 1 0) :contains (entity-ref 2 0))");
        assert!(prog.code.ops.iter().any(|op| matches!(op, Opcode::Unlink)));
    }
}
