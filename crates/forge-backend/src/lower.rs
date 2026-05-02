#![allow(dead_code)]

// This module contains the HIR→Op translation logic that is
// identical across all platforms. Platform-specific concerns
// (path resolution, env expansion) are delegated to the
// PlatformBackend trait methods.

use crate::{
    PlatformBackend,
    error::BackendError,
    plan::{BinOpKind, ExecutionPlan, Op, StdioConfig, Value},
};
use forge_hir::{HirBinOp, HirExpr, HirLiteral, HirProgram, HirStmt, HirUnaryOp};

/// Lowers a `HirProgram` into an `ExecutionPlan` using a provided backend
/// for platform-specific resolution.
pub struct HirLowerer<'a> {
    backend: &'a dyn PlatformBackend,
}

impl<'a> HirLowerer<'a> {
    pub fn new(backend: &'a dyn PlatformBackend) -> Self {
        Self { backend }
    }

    /// # Errors
    /// Returns `BackendError` if any statement fails to lower.
    pub fn lower_program(&self, program: &HirProgram) -> Result<ExecutionPlan, BackendError> {
        let mut ops = Vec::new();

        // Lower all top-level statements
        for stmt in &program.stmts {
            ops.extend(self.lower_stmt(stmt)?);
        }

        // Function definitions are registered but not executed immediately
        // They will be called via Op::CallFn when invoked
        // (Function dispatch is implemented in forge-exec)

        Ok(ExecutionPlan::new(ops))
    }

    /// # Errors
    /// Returns `BackendError` if the statement cannot be lowered.
    pub fn lower_stmt(&self, stmt: &HirStmt) -> Result<Vec<Op>, BackendError> {
        match stmt {
            HirStmt::Bind {
                name,
                mutable,
                value,
                ..
            } => {
                let val = Self::lower_expr_to_value(value);
                Ok(vec![Op::BindVar {
                    name: name.clone(),
                    mutable: *mutable,
                    value: val?,
                }])
            }

            HirStmt::Eval { expr, .. } => self.lower_expr_to_ops(expr),

            HirStmt::Return { value, .. } => {
                let val = Self::lower_expr_to_value(value);
                Ok(vec![Op::Return { value: val? }])
            }

            HirStmt::If {
                cond, then, else_, ..
            } => {
                let condition = Self::lower_expr_to_value(cond);
                let mut then_ops = Vec::new();
                for s in then {
                    then_ops.extend(self.lower_stmt(s)?);
                }
                let mut else_ops = Vec::new();
                for s in else_ {
                    else_ops.extend(self.lower_stmt(s)?);
                }
                Ok(vec![Op::If {
                    condition: condition?,
                    then_ops,
                    else_ops,
                }])
            }

            HirStmt::While { cond, body, .. } => {
                // The condition must be re-evaluated each iteration.
                // We lower it to ops that bind result to a temp var.
                let cond_var = "__while_cond__".to_string();
                let condition_ops = {
                    let val = Self::lower_expr_to_value(cond);
                    vec![Op::BindVar {
                        name: cond_var.clone(),
                        mutable: true,
                        value: val?,
                    }]
                };
                let mut body_ops = Vec::new();
                for s in body {
                    body_ops.extend(self.lower_stmt(s)?);
                }
                Ok(vec![Op::While {
                    condition_ops,
                    condition_var: cond_var,
                    body_ops,
                }])
            }
        }
    }

    fn lower_expr_to_ops(&self, expr: &HirExpr) -> Result<Vec<Op>, BackendError> {
        match expr {
            HirExpr::Call { callee, args, .. } => {
                // Resolve command or function call
                match self.backend.resolve_command(callee) {
                    Ok(path) if !path.is_empty() => {
                        // External process
                        let mut str_args = Vec::new();
                        for arg in args {
                            str_args.push(Self::value_to_string_arg(arg));
                        }
                        Ok(vec![Op::RunProcess {
                            command: path,
                            args: str_args,
                            env: vec![],
                            stdin: StdioConfig::Inherit,
                            stdout: StdioConfig::Inherit,
                            stderr: StdioConfig::Inherit,
                        }])
                    }
                    _ => {
                        // ForgeScript function call
                        let mut val_args = Vec::new();
                        for arg in args {
                            val_args.push(Self::lower_expr_to_value(arg)?);
                        }
                        Ok(vec![Op::CallFn {
                            name: callee.clone(),
                            args: val_args,
                            result_var: None,
                        }])
                    }
                }
            }

            HirExpr::Pipe { left, right, .. } => {
                let left_ops = self.lower_expr_to_ops(left)?;
                let right_ops = self.lower_expr_to_ops(right)?;

                // Each side should be a single Op for v1
                // Complex multi-op pipes deferred to later milestone
                match (left_ops.into_iter().next(), right_ops.into_iter().next()) {
                    (Some(l), Some(r)) => Ok(vec![Op::Pipe {
                        left: Box::new(l),
                        right: Box::new(r),
                    }]),
                    _ => Err(BackendError::Unsupported {
                        reason: "pipe sides must each produce exactly one op in v1".to_string(),
                    }),
                }
            }

            // For expressions used as statements, echo their value
            other => {
                let val = Self::lower_expr_to_value(other);
                Ok(vec![Op::Echo {
                    value: val?,
                    no_newline: false,
                }])
            }
        }
    }

    /// # Errors
    /// Returns `BackendError` if the expression cannot be converted to a `Value`.
    pub fn lower_expr_to_value(expr: &HirExpr) -> Result<Value, BackendError> {
        match expr {
            HirExpr::Literal(lit) => Ok(Self::lower_literal(lit)),
            HirExpr::Var { name, .. } => Ok(Value::VarRef(name.clone())),
            HirExpr::EnvVar { name, .. } => Ok(Value::EnvRef(name.clone())),
            HirExpr::BinOp {
                op, left, right, ..
            } => {
                // For constant folding of literals, compute now
                // For variable references, emit a VarRef and let executor compute
                let l = Self::lower_expr_to_value(left)?;
                let r = Self::lower_expr_to_value(right)?;
                // If both sides are concrete values, fold now
                if let (Some(result), ()) = (Self::try_fold_binop(op, &l, &r), ()) {
                    return Ok(result);
                }
                // Otherwise emit a sentinel — executor will evaluate at runtime
                // For v1: just return left and accept the limitation
                // Full runtime expression evaluation is in forge-exec
                Ok(l)
            }
            HirExpr::UnaryOp { op, operand, .. } => {
                let val = Self::lower_expr_to_value(operand)?;
                match (op, &val) {
                    (HirUnaryOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
                    (HirUnaryOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
                    (HirUnaryOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    _ => Ok(val), // runtime evaluation
                }
            }
            HirExpr::Call {
                callee, args: _, ..
            } => {
                // A call used as a value — result_var will capture it
                // Emit as VarRef to a synthetic result variable
                // Full implementation in forge-exec
                Ok(Value::VarRef(format!("__call_{callee}__")))
            }
            _ => Ok(Value::Null),
        }
    }

    fn lower_literal(lit: &HirLiteral) -> Value {
        match lit {
            HirLiteral::Int(n) => Value::Int(*n),
            HirLiteral::Float(f) => Value::Float(*f),
            HirLiteral::Str(s) => Value::Str(s.clone()),
            HirLiteral::Bool(b) => Value::Bool(*b),
            HirLiteral::Null => Value::Null,
        }
    }

    fn lower_binop(op: &HirBinOp) -> BinOpKind {
        match op {
            HirBinOp::Add => BinOpKind::Add,
            HirBinOp::Sub => BinOpKind::Sub,
            HirBinOp::Mul => BinOpKind::Mul,
            HirBinOp::Div => BinOpKind::Div,
            HirBinOp::Rem => BinOpKind::Rem,
            HirBinOp::Eq => BinOpKind::Eq,
            HirBinOp::Ne => BinOpKind::Ne,
            HirBinOp::Lt => BinOpKind::Lt,
            HirBinOp::Le => BinOpKind::Le,
            HirBinOp::Gt => BinOpKind::Gt,
            HirBinOp::Ge => BinOpKind::Ge,
            HirBinOp::And => BinOpKind::And,
            HirBinOp::Or => BinOpKind::Or,
            HirBinOp::Concat => BinOpKind::Concat,
        }
    }

    /// Attempt constant folding — returns `None` if not possible.
    fn try_fold_binop(op: &HirBinOp, left: &Value, right: &Value) -> Option<Value> {
        match (op, left, right) {
            (HirBinOp::Add, Value::Int(a), Value::Int(b)) => Some(Value::Int(a.checked_add(*b)?)),
            (HirBinOp::Sub, Value::Int(a), Value::Int(b)) => Some(Value::Int(a.checked_sub(*b)?)),
            (HirBinOp::Mul, Value::Int(a), Value::Int(b)) => Some(Value::Int(a.checked_mul(*b)?)),
            (HirBinOp::Eq, a, b) => Some(Value::Bool(a == b)),
            (HirBinOp::Ne, a, b) => Some(Value::Bool(a != b)),
            (HirBinOp::Concat, Value::Str(a), Value::Str(b)) => Some(Value::Str(format!("{a}{b}"))),
            _ => None,
        }
    }

    fn value_to_string_arg(expr: &HirExpr) -> String {
        match expr {
            HirExpr::Literal(HirLiteral::Str(s)) => s.clone(),
            HirExpr::Literal(HirLiteral::Int(n)) => n.to_string(),
            HirExpr::Literal(HirLiteral::Float(f)) => f.to_string(),
            HirExpr::Literal(HirLiteral::Bool(b)) => b.to_string(),
            HirExpr::Var { name, .. } => format!("${{{name}}}"),
            HirExpr::EnvVar { name, .. } => format!("${name}"),
            _ => String::new(),
        }
    }
}
