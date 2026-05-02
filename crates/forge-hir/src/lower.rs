use std::collections::HashSet;

use forge_ast::{
    Arg, BinaryOp, Block, Expr, FnDef, ImportPath, ImportStmt, Literal, Program, Stmt, UnaryOp,
};
use forge_types::Span;

use crate::error::LowerError;
use crate::hir::{
    HirBinOp, HirExpr, HirFnDef, HirImport, HirLiteral, HirParam, HirProgram, HirStmt, HirUnaryOp,
};

/// Built-in names pre-populated into the global scope.
const BUILTINS: &[&str] = &[
    "echo", "cd", "ls", "pwd", "mkdir", "rm", "cp", "mv", "env", "export", "set", "unset", "exit",
    "which", "cat", "grep", "find", "head", "tail", "wc", "sort", "forge", "list",
];

fn import_path_key(path: &ImportPath) -> String {
    match path {
        ImportPath::Absolute(parts) => parts.join("::"),
        ImportPath::Relative(parts) => format!("./{}", parts.join("/")),
    }
}

/// Lowers a `Program` AST into a `HirProgram`.
pub struct AstLowerer {
    /// Stack of lexical scopes. Each scope maps declared names.
    /// Index 0 is the global scope, last is the innermost scope.
    scopes: Vec<HashSet<String>>,
    /// Import stack for circular import detection (normalised path keys).
    pub(crate) import_stack: Vec<String>,
    /// Function names declared at the top level.
    fn_names: HashSet<String>,
}

impl AstLowerer {
    /// Create a new lowerer with built-in names pre-declared.
    #[must_use]
    pub fn new() -> Self {
        let mut global_scope = HashSet::new();
        for name in BUILTINS {
            global_scope.insert((*name).to_string());
        }
        Self {
            scopes: vec![global_scope],
            import_stack: Vec::new(),
            fn_names: HashSet::new(),
        }
    }

    /// Lower a complete AST `Program` into a `HirProgram`.
    ///
    /// # Errors
    ///
    /// Returns [`LowerError`] when name resolution fails or the program uses a construct
    /// that is not yet supported by this lowerer.
    pub fn lower(&mut self, program: Program) -> Result<HirProgram, LowerError> {
        let mut fns = Vec::new();
        let mut imports = Vec::new();
        let mut stmts = Vec::new();

        for stmt in &program.stmts {
            if let Stmt::FnDef(FnDef { name, .. }) = stmt {
                if self.fn_names.contains(name) {
                    return Err(LowerError::DuplicateFunctionDef { name: name.clone() });
                }
                self.fn_names.insert(name.clone());
                self.declare(name);
            }
        }

        for stmt in program.stmts {
            match stmt {
                Stmt::FnDef(FnDef {
                    name, params, body, ..
                }) => {
                    fns.push(self.lower_fn_def(name, params, body)?);
                }
                Stmt::Import(ImportStmt { path, alias, .. }) => {
                    let key = import_path_key(&path);
                    if self.import_stack.contains(&key) {
                        return Err(LowerError::CircularImport { path: key });
                    }
                    imports.push(HirImport {
                        path: key,
                        alias,
                        span: Span::default(),
                    });
                }
                Stmt::StructDef(_) | Stmt::EnumDef(_) => {
                    return Err(LowerError::Unsupported {
                        reason: "struct and enum definitions are not yet lowered to HIR"
                            .to_string(),
                        line: 0,
                    });
                }
                other => stmts.push(self.lower_stmt(other)?),
            }
        }

        Ok(HirProgram {
            fns,
            imports,
            stmts,
        })
    }

    fn lower_fn_def(
        &mut self,
        name: String,
        params: Vec<forge_ast::Param>,
        body: Block,
    ) -> Result<HirFnDef, LowerError> {
        self.push_scope();

        let hir_params: Vec<HirParam> = params
            .into_iter()
            .map(|p| {
                self.declare(&p.name);
                HirParam {
                    name: p.name,
                    span: Span::default(),
                }
            })
            .collect();

        let hir_body = self.lower_block(body)?;
        self.pop_scope();

        Ok(HirFnDef {
            name,
            params: hir_params,
            body: hir_body,
            span: Span::default(),
        })
    }

    fn lower_stmt(&mut self, stmt: Stmt) -> Result<HirStmt, LowerError> {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                value,
                ..
            } => {
                let hir_value = self.lower_expr(value)?;
                self.declare(&name);
                Ok(HirStmt::Bind {
                    name,
                    mutable,
                    value: hir_value,
                    span: Span::default(),
                })
            }
            Stmt::Const { name, value, .. } => {
                let hir_value = self.lower_expr(value)?;
                self.declare(&name);
                Ok(HirStmt::Bind {
                    name,
                    mutable: false,
                    value: hir_value,
                    span: Span::default(),
                })
            }
            Stmt::Assign { target, value } => {
                let Expr::Ident(name) = target else {
                    return Err(LowerError::Unsupported {
                        reason: "only simple identifier assignment is supported".to_string(),
                        line: 0,
                    });
                };
                if !self.is_declared(&name) {
                    return Err(LowerError::UndefinedVariable { name, line: 0 });
                }
                let hir_value = self.lower_expr(value)?;
                Ok(HirStmt::Bind {
                    name,
                    mutable: true,
                    value: hir_value,
                    span: Span::default(),
                })
            }
            Stmt::ExprStmt(expr) => {
                let hir_expr = self.lower_expr(expr)?;
                Ok(HirStmt::Eval {
                    expr: hir_expr,
                    span: Span::default(),
                })
            }
            Stmt::Return(val) => {
                let hir_val = match val {
                    Some(v) => self.lower_expr(v)?,
                    None => HirExpr::Literal(HirLiteral::Null),
                };
                Ok(HirStmt::Return {
                    value: hir_val,
                    span: Span::default(),
                })
            }
            Stmt::While { cond, body } => {
                let hir_cond = self.lower_expr(cond)?;
                let hir_body = self.lower_block(body)?;
                Ok(HirStmt::While {
                    cond: hir_cond,
                    body: hir_body,
                    span: Span::default(),
                })
            }
            Stmt::For { .. } | Stmt::Loop(_) => Err(LowerError::Unsupported {
                reason: "for / loop are not yet implemented in the lowerer".to_string(),
                line: 0,
            }),
            Stmt::FnDef(_) | Stmt::Import(_) => Err(LowerError::Unsupported {
                reason: "nested function definitions and imports must be at the top level"
                    .to_string(),
                line: 0,
            }),
            Stmt::StructDef(_) | Stmt::EnumDef(_) => Err(LowerError::Unsupported {
                reason: "struct / enum definitions must be at the top level".to_string(),
                line: 0,
            }),
        }
    }

    fn lower_block(&mut self, block: Block) -> Result<Vec<HirStmt>, LowerError> {
        self.push_scope();
        let mut stmts = Vec::new();
        for stmt in block.stmts {
            stmts.push(self.lower_stmt(stmt)?);
        }
        if let Some(tail) = block.tail {
            stmts.push(HirStmt::Eval {
                expr: self.lower_expr(*tail)?,
                span: Span::default(),
            });
        }
        self.pop_scope();
        Ok(stmts)
    }

    fn lower_if_else_stmts(&mut self, expr: Expr) -> Result<Vec<HirStmt>, LowerError> {
        match expr {
            Expr::Block(b) => self.lower_block(b),
            other => Ok(vec![HirStmt::Eval {
                expr: self.lower_expr(other)?,
                span: Span::default(),
            }]),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn lower_expr(&mut self, expr: Expr) -> Result<HirExpr, LowerError> {
        match expr {
            Expr::Literal(lit) => Ok(HirExpr::Literal(Self::lower_literal(lit))),

            Expr::Ident(name) => {
                if !self.is_declared(&name) {
                    return Err(LowerError::UndefinedVariable { name, line: 0 });
                }
                Ok(HirExpr::Var {
                    name,
                    span: Span::default(),
                })
            }

            Expr::EnvVar(name) => Ok(HirExpr::EnvVar {
                name,
                span: Span::default(),
            }),

            Expr::BinaryOp { op, lhs, rhs } => {
                if op == BinaryOp::Pipe {
                    let hir_left = self.lower_expr(*lhs)?;
                    let hir_right = self.lower_expr(*rhs)?;
                    return Ok(HirExpr::Pipe {
                        left: Box::new(hir_left),
                        right: Box::new(hir_right),
                        span: Span::default(),
                    });
                }
                let hir_left = self.lower_expr(*lhs)?;
                let hir_right = self.lower_expr(*rhs)?;
                Ok(HirExpr::BinOp {
                    op: Self::lower_binop(&op)?,
                    left: Box::new(hir_left),
                    right: Box::new(hir_right),
                    span: Span::default(),
                })
            }

            Expr::UnaryOp { op, operand } => {
                let hir_operand = self.lower_expr(*operand)?;
                Ok(HirExpr::UnaryOp {
                    op: match op {
                        UnaryOp::Neg => HirUnaryOp::Neg,
                        UnaryOp::Not => HirUnaryOp::Not,
                    },
                    operand: Box::new(hir_operand),
                    span: Span::default(),
                })
            }

            Expr::Call { callee, args } => {
                // Command/function names are resolved at runtime (builtin registry
                // or PATH lookup). We do NOT require the callee to be in the HIR
                // scope — that check is correct for variable references but wrong
                // for command invocations where the name is only known at runtime.
                let callee_name = match *callee {
                    Expr::Ident(name) => name,
                    other => {
                        return Err(LowerError::Unsupported {
                            reason: format!("only simple calls are supported, got: {other:?}"),
                            line: 0,
                        });
                    }
                };
                let mut hir_args = Vec::new();
                for arg in args {
                    match arg {
                        Arg::Positional(e) => hir_args.push(self.lower_expr(e)?),
                        Arg::Named { .. } | Arg::Flag(_) | Arg::NoFlag(_) => {
                            return Err(LowerError::Unsupported {
                                reason: "only positional call arguments are supported in HIR v1"
                                    .to_string(),
                                line: 0,
                            });
                        }
                    }
                }
                Ok(HirExpr::Call {
                    callee: callee_name,
                    args: hir_args,
                    span: Span::default(),
                })
            }

            Expr::Field { base, field } => {
                let hir_target = self.lower_expr(*base)?;
                Ok(HirExpr::FieldAccess {
                    target: Box::new(hir_target),
                    field,
                    span: Span::default(),
                })
            }

            Expr::Index { base, index } => {
                let hir_target = self.lower_expr(*base)?;
                let hir_index = self.lower_expr(*index)?;
                Ok(HirExpr::Index {
                    target: Box::new(hir_target),
                    index: Box::new(hir_index),
                    span: Span::default(),
                })
            }

            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let hir_cond = self.lower_expr(*cond)?;
                let hir_then = self.lower_block(then_branch)?;
                let hir_else = match else_branch {
                    None => Vec::new(),
                    Some(e) => self.lower_if_else_stmts(*e)?,
                };
                Ok(HirExpr::If {
                    cond: Box::new(hir_cond),
                    then: hir_then,
                    else_: hir_else,
                    span: Span::default(),
                })
            }

            Expr::Block(block) => {
                if block.stmts.is_empty() {
                    return match block.tail {
                        Some(t) => self.lower_expr(*t),
                        None => Ok(HirExpr::Literal(HirLiteral::Null)),
                    };
                }
                tracing::warn!(
                    "block expressions with statements do not yet propagate their value"
                );
                Ok(HirExpr::Literal(HirLiteral::Null))
            }

            Expr::Interpolated(_) => {
                tracing::warn!("string interpolation not yet lowered — treating as empty string");
                Ok(HirExpr::Literal(HirLiteral::Str(String::new())))
            }

            Expr::MethodCall { .. } => Err(LowerError::Unsupported {
                reason: "method calls are not yet lowered to HIR".to_string(),
                line: 0,
            }),

            Expr::Match { .. } | Expr::Spawn { .. } | Expr::Join(_) | Expr::Try(_) => {
                Err(LowerError::Unsupported {
                    reason: "match / spawn / join / try are not yet implemented".to_string(),
                    line: 0,
                })
            }

            Expr::Return(_) | Expr::Break(_) | Expr::Continue => Err(LowerError::Unsupported {
                reason: "return / break / continue only supported as statements".to_string(),
                line: 0,
            }),
        }
    }

    fn lower_literal(lit: Literal) -> HirLiteral {
        match lit {
            Literal::Int(n) => HirLiteral::Int(n),
            Literal::Float(f) => HirLiteral::Float(f),
            Literal::Str(s) | Literal::Path(s) | Literal::Regex(s) | Literal::Url(s) => {
                HirLiteral::Str(s)
            }
            Literal::Bool(b) => HirLiteral::Bool(b),
        }
    }

    fn lower_binop(op: &BinaryOp) -> Result<HirBinOp, LowerError> {
        match op {
            BinaryOp::Add => Ok(HirBinOp::Add),
            BinaryOp::Sub => Ok(HirBinOp::Sub),
            BinaryOp::Mul => Ok(HirBinOp::Mul),
            BinaryOp::Div => Ok(HirBinOp::Div),
            BinaryOp::Rem => Ok(HirBinOp::Rem),
            BinaryOp::Eq => Ok(HirBinOp::Eq),
            BinaryOp::Ne => Ok(HirBinOp::Ne),
            BinaryOp::Lt => Ok(HirBinOp::Lt),
            BinaryOp::Le => Ok(HirBinOp::Le),
            BinaryOp::Gt => Ok(HirBinOp::Gt),
            BinaryOp::Ge => Ok(HirBinOp::Ge),
            BinaryOp::And => Ok(HirBinOp::And),
            BinaryOp::Or => Ok(HirBinOp::Or),
            BinaryOp::PathJoin => Ok(HirBinOp::Concat),
            BinaryOp::Pipe => Err(LowerError::Unsupported {
                reason: "internal: pipe should be handled before lower_binop".to_string(),
                line: 0,
            }),
            BinaryOp::AddSat
            | BinaryOp::SubSat
            | BinaryOp::MulSat
            | BinaryOp::AddWrap
            | BinaryOp::SubWrap
            | BinaryOp::MulWrap => Err(LowerError::Unsupported {
                reason: "saturating / wrapping arithmetic is not yet lowered to HIR".to_string(),
                line: 0,
            }),
        }
    }

    /// Declare a name in the global (outermost) scope.
    ///
    /// Use this to seed the lowerer with variables that already exist in the
    /// shell context before lowering the next REPL command.
    pub fn declare_global(&mut self, name: &str) {
        if let Some(scope) = self.scopes.first_mut() {
            scope.insert(name.to_string());
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn is_declared(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|s| s.contains(name))
    }
}

impl Default for AstLowerer {
    fn default() -> Self {
        Self::new()
    }
}
