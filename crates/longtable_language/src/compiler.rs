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

use std::collections::HashMap;

use longtable_foundation::{Error, ErrorKind, Result, Value};

use crate::ast::Ast;
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
    /// Native function name -> index mapping.
    natives: HashMap<String, u16>,
    /// Compiled functions.
    functions: Vec<CompiledFunction>,
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
            natives: HashMap::new(),
            functions: Vec::new(),
        };

        // Register built-in native functions
        compiler.register_natives();
        compiler
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
        ];

        for (idx, name) in natives.iter().enumerate() {
            self.natives.insert((*name).to_string(), idx as u16);
        }
    }

    /// Compiles an AST expression into bytecode.
    pub fn compile_expr(&mut self, ast: &Ast) -> Result<Bytecode> {
        let mut code = Bytecode::new();
        self.compile_node(ast, &mut code)?;
        Ok(code)
    }

    /// Compiles multiple expressions into a program.
    pub fn compile(&mut self, asts: &[Ast]) -> Result<CompiledProgram> {
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
                // For now, store as string constant with keyword marker
                let idx = self.add_constant(Value::String(format!(":{name}").into()));
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
                "quote" => return self.compile_quote_form(args, span, code),
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

        // Process bindings
        for chunk in bindings.chunks(2) {
            let name = match &chunk[0] {
                Ast::Symbol(name, _) => name.clone(),
                _ => return Err(self.error(span, "let binding name must be a symbol")),
            };
            let value = &chunk[1];

            // Compile the value
            self.compile_node(value, code)?;

            // Store in local slot
            let slot = self.next_local;
            self.next_local += 1;
            code.emit(Opcode::StoreLocal(slot));
            self.locals.insert(name, slot);
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

        // Reset locals for function compilation
        self.locals.clear();
        self.next_local = 0;

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

        // Restore compiler state
        self.locals = saved_locals;
        self.next_local = saved_next;

        // Create compiled function
        let func = CompiledFunction {
            arity,
            params: param_names,
            code: fn_code,
            locals_count,
        };

        // Add to functions table
        let fn_index = u32::try_from(self.functions.len())
            .map_err(|_| self.error(span, "too many functions"))?;
        self.functions.push(func);

        // Create function value
        let fn_value = Value::Fn(longtable_foundation::LtFn::Compiled(
            longtable_foundation::CompiledFn {
                index: fn_index,
                captures: None, // No closures yet
            },
        ));

        let idx = self.add_constant(fn_value);
        code.emit(Opcode::Const(idx));

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

    /// Compiles a quote special form.
    fn compile_quote_form(&mut self, args: &[Ast], span: Span, code: &mut Bytecode) -> Result<()> {
        if args.len() != 1 {
            return Err(self.error(span, "quote requires exactly 1 argument"));
        }
        self.compile_quoted(&args[0], code)
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

/// Compiles source code to a program.
pub fn compile(source: &str) -> Result<CompiledProgram> {
    let ast = crate::parser::parse(source)?;
    let mut compiler = Compiler::new();
    compiler.compile(&ast)
}

/// Compiles a single expression.
pub fn compile_expr(source: &str) -> Result<Bytecode> {
    let ast = crate::parser::parse_one(source)?;
    let mut compiler = Compiler::new();
    compiler.compile_expr(&ast)
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
        let prog = compile_test("3.14");
        assert!(matches!(prog.constants[0], Value::Float(f) if (f - 3.14).abs() < 0.001));
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
}
