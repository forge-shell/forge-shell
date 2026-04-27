#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// The platform-neutral execution plan.
/// Produced by `PlatformBackend::lower` and from a `HirProgram`.
/// Consumed by `forge-exec::Executor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// The ordered list of operations to execute.
    pub ops: Vec<Op>,
}

impl ExecutionPlan {
    #[must_use]
    pub fn new(ops: Vec<Op>) -> Self {
        Self { ops }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self { ops: Vec::new() }
    }
}

/// A single operation in the execution plan.
///
/// Each variant maps to a concrete action the executor performs.
/// Operations are intentionally coarse-grained - one Op per
/// observable side effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Op {
    /// Spawn an external OS process.
    RunProcess {
        /// Resolved command path or name.
        command: String,
        /// Arguments to pass verbatim.
        args: Vec<String>,
        /// Environment variable overrides for this process only.
        env: Vec<(String, String)>,
        /// stdin configuration.
        stdin: StdioConfig,
        /// stdout configuration.
        stdout: StdioConfig,
        /// stderr configuration.
        stderr: StdioConfig,
    },

    /// Set an environment variable in the shell context.
    /// Inherited by child processes spawned after this op.
    SetEnv { key: String, value: Value },

    /// Unset an environment variable.
    UnsetEnv { key: String },

    /// Change the working directory.
    Cd {
        /// Path - may contain `~` or `$VAR` refs already expanded by the backend.
        path: String,
    },

    /// Bind a value to a variable in the shell context.
    BindVar {
        name: String,
        mutable: bool,
        value: Value,
    },

    /// Evaluate a binary operation and bind the result.
    Bin {
        result_var: String,
        op: BinOpKind,
        left: Value,
        right: Value,
    },

    /// Evaluate a unary operation and bind the result.
    Unary {
        result_var: String,
        op: UnaryOpKind,
        operand: Value,
    },

    /// Conditional execution.
    If {
        condition: Value,
        then_ops: Vec<Op>,
        else_ops: Vec<Op>,
    },

    /// While loop.
    While {
        /// Operations that compute the condition each iteration.
        condition_ops: Vec<Op>,
        /// Variable name that holds the condition result after `condition_ops` run.
        condition_var: String,
        /// Loop body.
        body_ops: Vec<Op>,
    },

    /// Pipe: stdout of left fed into stdin of right.
    Pipe { left: Box<Op>, right: Box<Op> },

    /// Redirect stdout of an op to a file.
    RedirectOut {
        op: Box<Op>,
        /// Resolved file path.
        path: String,
        /// If true, append to file; if false, truncate.
        append: bool,
    },

    /// Redirect a file into stdin of an op.
    RedirectIn { op: Box<Op>, path: String },

    /// Write a value to stdout.
    Echo {
        value: Value,
        /// If true, suppress trailing newline.
        no_newline: bool,
    },

    /// Call a `ForgeScript` function defined in the plan.
    CallFn {
        name: String,
        args: Vec<Value>,
        /// Variable to bind the return value to (if any).
        result_var: Option<String>,
    },

    /// Return from a function with a value.
    Return { value: Value },

    /// Exit the shell with a given code.
    Exit { code: i32 },

    /// Load environment variables from a file.
    /// Variables already in the environment are NOT overwritten.
    LoadEnvFile { path: String },

    /// Validate that required environment variables are set.
    /// Fails with `ExecError::RequiredEnvMissing` if any are absent.
    RequireEnv { vars: Vec<String> },
}

/// How a process's stdio stream is configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StdioConfig {
    /// Inherit from the parent shell process.
    Inherit,
    /// Capture via an OS pipe.
    Piped,
    /// Discard (equivalent to /dev/null).
    Null,
}

/// A runtime value in the execution plan.
///
/// Some values are known at the lowering time (literals).
/// Others reference variables resolved at the execution time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    List(Vec<Value>),
    Null,
    /// Reference to a `ForgeScript` variable - resolved at runtime.
    VarRef(String),
    /// Reference to an environment variable - resolved at runtime.
    EnvRef(String),
}

impl Value {
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Null => false,
            Value::VarRef(_) | Value::EnvRef(_) => true, // resolved at runtime
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(n) => write!(f, "{n}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::List(l) => write!(
                f,
                "[{}]",
                l.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Null => write!(f, "null"),
            Value::VarRef(name) => write!(f, "${{{name}}}"),
            Value::EnvRef(name) => write!(f, "${name}"),
        }
    }
}

/// Binary operation kinds in the execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Concat,
}

/// Unary operation kinds in the execution plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOpKind {
    Neg,
    Not,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // ExecutionPlan construction
    // -------------------------------------------------------------------------

    #[test]
    fn test_empty_plan() {
        let plan = ExecutionPlan::empty();
        assert!(plan.ops.is_empty());
    }

    #[test]
    fn test_plan_new() {
        let plan = ExecutionPlan::new(vec![Op::Echo {
            value: Value::Str("hello".to_string()),
            no_newline: false,
        }]);
        assert_eq!(plan.ops.len(), 1);
    }

    // -------------------------------------------------------------------------
    // Value::is_truthy
    // -------------------------------------------------------------------------

    #[test]
    fn test_truthy_bool() {
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Bool(false).is_truthy());
    }

    #[test]
    fn test_truthy_int() {
        assert!(Value::Int(1).is_truthy());
        assert!(Value::Int(-1).is_truthy());
        assert!(!Value::Int(0).is_truthy());
    }

    #[test]
    fn test_truthy_str() {
        assert!(Value::Str("x".to_string()).is_truthy());
        assert!(!Value::Str(String::new()).is_truthy());
    }

    #[test]
    fn test_truthy_null() {
        assert!(!Value::Null.is_truthy());
    }

    #[test]
    fn test_truthy_list() {
        assert!(Value::List(vec![Value::Int(1)]).is_truthy());
        assert!(!Value::List(vec![]).is_truthy());
    }

    #[test]
    fn test_truthy_var_ref_always_true() {
        // VarRef truthiness is deferred to runtime — conservative true
        assert!(Value::VarRef("x".to_string()).is_truthy());
        assert!(Value::EnvRef("HOME".to_string()).is_truthy());
    }

    // -------------------------------------------------------------------------
    // Value::Display
    // -------------------------------------------------------------------------

    #[test]
    fn test_display_int() {
        assert_eq!(Value::Int(42).to_string(), "42");
    }

    #[test]
    fn test_display_float() {
        assert_eq!(Value::Float(3.14).to_string(), "3.14");
    }

    #[test]
    fn test_display_str() {
        assert_eq!(Value::Str("hello".to_string()).to_string(), "hello");
    }

    #[test]
    fn test_display_bool() {
        assert_eq!(Value::Bool(true).to_string(), "true");
        assert_eq!(Value::Bool(false).to_string(), "false");
    }

    #[test]
    fn test_display_null() {
        assert_eq!(Value::Null.to_string(), "null");
    }

    #[test]
    fn test_display_list() {
        let list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        assert_eq!(list.to_string(), "[1, 2, 3]");
    }

    #[test]
    fn test_display_var_ref() {
        assert_eq!(Value::VarRef("x".to_string()).to_string(), "${x}");
    }

    #[test]
    fn test_display_env_ref() {
        assert_eq!(Value::EnvRef("HOME".to_string()).to_string(), "$HOME");
    }

    // -------------------------------------------------------------------------
    // Serialisation round-trips
    // -------------------------------------------------------------------------

    #[test]
    fn test_value_round_trips() {
        let values = vec![
            Value::Int(42),
            Value::Float(3.14),
            Value::Str("hello".to_string()),
            Value::Bool(true),
            Value::Null,
            Value::VarRef("x".to_string()),
            Value::EnvRef("HOME".to_string()),
            Value::List(vec![Value::Int(1), Value::Int(2)]),
        ];
        for val in values {
            let json = serde_json::to_string(&val).unwrap();
            let back: Value = serde_json::from_str(&json).unwrap();
            assert_eq!(val, back);
        }
    }

    #[test]
    fn test_op_echo_round_trips() {
        let op = Op::Echo {
            value: Value::Str("hello".to_string()),
            no_newline: false,
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            back,
            Op::Echo {
                no_newline: false,
                ..
            }
        ));
    }

    #[test]
    fn test_op_bind_var_round_trips() {
        let op = Op::BindVar {
            name: "x".to_string(),
            mutable: true,
            value: Value::Int(99),
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Op::BindVar { mutable: true, .. }));
    }

    #[test]
    fn test_op_if_round_trips() {
        let op = Op::If {
            condition: Value::Bool(true),
            then_ops: vec![Op::Echo {
                value: Value::Str("yes".to_string()),
                no_newline: false,
            }],
            else_ops: vec![],
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Op::If { .. }));
    }

    #[test]
    fn test_op_pipe_round_trips() {
        let op = Op::Pipe {
            left: Box::new(Op::Echo {
                value: Value::Str("left".to_string()),
                no_newline: false,
            }),
            right: Box::new(Op::Echo {
                value: Value::Str("right".to_string()),
                no_newline: false,
            }),
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: Op = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Op::Pipe { .. }));
    }

    #[test]
    fn test_complete_plan_round_trips() {
        let plan = ExecutionPlan::new(vec![
            Op::BindVar {
                name: "x".to_string(),
                mutable: false,
                value: Value::Int(42),
            },
            Op::SetEnv {
                key: "MY_VAR".to_string(),
                value: Value::Str("hello".to_string()),
            },
            Op::If {
                condition: Value::VarRef("x".to_string()),
                then_ops: vec![Op::Echo {
                    value: Value::Str("truthy".to_string()),
                    no_newline: false,
                }],
                else_ops: vec![Op::Echo {
                    value: Value::Str("falsy".to_string()),
                    no_newline: false,
                }],
            },
            Op::Exit { code: 0 },
        ]);

        let json = serde_json::to_string_pretty(&plan).unwrap();
        let back: ExecutionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ops.len(), plan.ops.len());
    }

    // -------------------------------------------------------------------------
    // StdioConfig
    // -------------------------------------------------------------------------

    #[test]
    fn test_stdio_config_variants_round_trip() {
        let configs = vec![StdioConfig::Inherit, StdioConfig::Piped, StdioConfig::Null];
        for cfg in configs {
            let json = serde_json::to_string(&cfg).unwrap();
            let back: StdioConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(cfg, back);
        }
    }

    // -------------------------------------------------------------------------
    // Op type tags in JSON (verify serde tag attribute works correctly)
    // -------------------------------------------------------------------------

    #[test]
    fn test_op_json_has_type_tag() {
        let op = Op::Echo {
            value: Value::Str("hi".to_string()),
            no_newline: false,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"type\":\"echo\""));
    }

    #[test]
    fn test_bind_var_json_has_type_tag() {
        let op = Op::BindVar {
            name: "x".to_string(),
            mutable: false,
            value: Value::Null,
        };
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains("\"type\":\"bind_var\""));
    }
}
