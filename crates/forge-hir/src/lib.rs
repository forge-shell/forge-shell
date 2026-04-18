#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod error;
pub mod hir;
pub mod lower;

pub use error::LowerError;
pub use hir::*;
pub use lower::AstLowerer;

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ast::{
        BinaryOp, Block, Expr, FnDef, ImportPath, ImportStmt, Literal, Param, Program, Stmt, Type,
    };

    fn lower(program: Program) -> HirProgram {
        AstLowerer::new().lower(program).expect("lowering failed")
    }

    fn lower_err(program: Program) -> LowerError {
        AstLowerer::new()
            .lower(program)
            .expect_err("expected lowering error")
    }

    fn empty_program() -> Program {
        Program {
            directives: vec![],
            stmts: vec![],
        }
    }

    fn program_with(stmts: Vec<Stmt>) -> Program {
        Program {
            directives: vec![],
            stmts,
        }
    }

    #[test]
    fn test_empty_program() {
        let hir = lower(empty_program());
        assert!(hir.stmts.is_empty());
        assert!(hir.fns.is_empty());
        assert!(hir.imports.is_empty());
    }

    #[test]
    fn test_let_binding_becomes_bind() {
        let prog = program_with(vec![Stmt::Let {
            name: "x".to_string(),
            mutable: false,
            ty: None,
            value: Expr::Literal(Literal::Int(42)),
        }]);
        let hir = lower(prog);
        assert_eq!(hir.stmts.len(), 1);
        assert!(matches!(
            hir.stmts[0],
            HirStmt::Bind {
                ref name,
                mutable: false,
                ..
            } if name == "x"
        ));
    }

    #[test]
    fn test_let_mut_binding() {
        let prog = program_with(vec![Stmt::Let {
            name: "x".to_string(),
            mutable: true,
            ty: None,
            value: Expr::Literal(Literal::Int(1)),
        }]);
        let hir = lower(prog);
        assert!(matches!(hir.stmts[0], HirStmt::Bind { mutable: true, .. }));
    }

    #[test]
    fn test_assign_becomes_bind_mutable() {
        let prog = program_with(vec![
            Stmt::Let {
                name: "x".to_string(),
                mutable: true,
                ty: None,
                value: Expr::Literal(Literal::Int(1)),
            },
            Stmt::Assign {
                target: Expr::Ident("x".to_string()),
                value: Expr::Literal(Literal::Int(2)),
            },
        ]);
        let hir = lower(prog);
        assert_eq!(hir.stmts.len(), 2);
        assert!(matches!(hir.stmts[1], HirStmt::Bind { mutable: true, .. }));
    }

    #[test]
    fn test_return_none_becomes_null() {
        let prog = program_with(vec![Stmt::Return(None)]);
        let hir = lower(prog);
        assert!(matches!(
            hir.stmts[0],
            HirStmt::Return {
                value: HirExpr::Literal(HirLiteral::Null),
                ..
            }
        ));
    }

    #[test]
    fn test_fn_def_lifted_to_hir_fns() {
        let prog = program_with(vec![Stmt::FnDef(FnDef {
            name: "greet".to_string(),
            params: vec![],
            ret_ty: None,
            body: Block {
                stmts: vec![],
                tail: None,
            },
        })]);
        let hir = lower(prog);
        assert_eq!(hir.fns.len(), 1);
        assert_eq!(hir.stmts.len(), 0);
        assert_eq!(hir.fns[0].name, "greet");
    }

    #[test]
    fn test_import_lifted_to_hir_imports() {
        let prog = program_with(vec![Stmt::Import(ImportStmt {
            path: ImportPath::Relative(vec!["utils".to_string()]),
            alias: None,
            items: vec![],
        })]);
        let hir = lower(prog);
        assert_eq!(hir.imports.len(), 1);
        assert_eq!(hir.stmts.len(), 0);
        assert_eq!(hir.imports[0].path, "./utils");
    }

    #[test]
    fn test_import_with_alias() {
        let prog = program_with(vec![Stmt::Import(ImportStmt {
            path: ImportPath::Relative(vec!["utils".to_string()]),
            alias: Some("utils".to_string()),
            items: vec![],
        })]);
        let hir = lower(prog);
        assert_eq!(hir.imports[0].alias, Some("utils".to_string()));
    }

    #[test]
    fn test_declared_variable_resolves() {
        let prog = program_with(vec![
            Stmt::Let {
                name: "x".to_string(),
                mutable: false,
                ty: None,
                value: Expr::Literal(Literal::Int(1)),
            },
            Stmt::ExprStmt(Expr::Ident("x".to_string())),
        ]);
        assert!(AstLowerer::new().lower(prog).is_ok());
    }

    #[test]
    fn test_undeclared_variable_is_error() {
        let prog = program_with(vec![Stmt::ExprStmt(Expr::Ident("undeclared".to_string()))]);
        let err = lower_err(prog);
        assert!(
            matches!(err, LowerError::UndefinedVariable { ref name, .. } if name == "undeclared")
        );
    }

    #[test]
    fn test_env_var_bypasses_scope_check() {
        let prog = program_with(vec![Stmt::ExprStmt(Expr::EnvVar("HOME".to_string()))]);
        assert!(AstLowerer::new().lower(prog).is_ok());
    }

    #[test]
    fn test_builtin_commands_are_predeclared() {
        let prog = program_with(vec![Stmt::ExprStmt(Expr::Ident("echo".to_string()))]);
        assert!(AstLowerer::new().lower(prog).is_ok());
    }

    #[test]
    fn test_variable_declared_in_inner_scope_not_visible_outside() {
        let prog = program_with(vec![
            Stmt::ExprStmt(Expr::If {
                cond: Box::new(Expr::Literal(Literal::Bool(true))),
                then_branch: Block {
                    stmts: vec![Stmt::Let {
                        name: "inner".to_string(),
                        mutable: false,
                        ty: None,
                        value: Expr::Literal(Literal::Int(1)),
                    }],
                    tail: None,
                },
                else_branch: None,
            }),
            Stmt::ExprStmt(Expr::Ident("inner".to_string())),
        ]);
        let err = lower_err(prog);
        assert!(matches!(err, LowerError::UndefinedVariable { ref name, .. } if name == "inner"));
    }

    #[test]
    fn test_fn_params_declared_in_fn_scope() {
        let prog = program_with(vec![Stmt::FnDef(FnDef {
            name: "greet".to_string(),
            params: vec![Param {
                name: "name".to_string(),
                ty: Type::Str,
            }],
            ret_ty: None,
            body: Block {
                stmts: vec![Stmt::Return(Some(Expr::Ident("name".to_string())))],
                tail: None,
            },
        })]);
        assert!(AstLowerer::new().lower(prog).is_ok());
    }

    #[test]
    fn test_duplicate_function_def_is_error() {
        let prog = program_with(vec![
            Stmt::FnDef(FnDef {
                name: "greet".to_string(),
                params: vec![],
                ret_ty: None,
                body: Block {
                    stmts: vec![],
                    tail: None,
                },
            }),
            Stmt::FnDef(FnDef {
                name: "greet".to_string(),
                params: vec![],
                ret_ty: None,
                body: Block {
                    stmts: vec![],
                    tail: None,
                },
            }),
        ]);
        let err = lower_err(prog);
        assert!(matches!(err, LowerError::DuplicateFunctionDef { ref name } if name == "greet"));
    }

    #[test]
    fn test_circular_import_detected() {
        let mut lowerer = AstLowerer::new();
        lowerer.import_stack.push("./a".to_string());

        let prog = program_with(vec![Stmt::Import(ImportStmt {
            path: ImportPath::Relative(vec!["a".to_string()]),
            alias: None,
            items: vec![],
        })]);
        let err = lowerer
            .lower(prog)
            .expect_err("expected circular import error");
        assert!(matches!(err, LowerError::CircularImport { ref path } if path == "./a"));
    }

    #[test]
    fn test_binary_op_lowered() {
        let prog = program_with(vec![Stmt::Let {
            name: "result".to_string(),
            mutable: false,
            ty: None,
            value: Expr::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Expr::Literal(Literal::Int(1))),
                rhs: Box::new(Expr::Literal(Literal::Int(2))),
            },
        }]);
        let hir = lower(prog);
        assert!(matches!(
            hir.stmts[0],
            HirStmt::Bind {
                value: HirExpr::BinOp {
                    op: HirBinOp::Add,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn test_while_stmt_lowered() {
        let prog = program_with(vec![Stmt::While {
            cond: Expr::Literal(Literal::Bool(true)),
            body: Block {
                stmts: vec![],
                tail: None,
            },
        }]);
        let hir = lower(prog);
        assert!(matches!(hir.stmts[0], HirStmt::While { .. }));
    }

    #[test]
    fn test_pipe_expr_lowered() {
        let prog = program_with(vec![Stmt::ExprStmt(Expr::BinaryOp {
            op: BinaryOp::Pipe,
            lhs: Box::new(Expr::Ident("ls".to_string())),
            rhs: Box::new(Expr::Ident("grep".to_string())),
        })]);
        let hir = lower(prog);
        assert!(matches!(
            hir.stmts[0],
            HirStmt::Eval {
                expr: HirExpr::Pipe { .. },
                ..
            }
        ));
    }
}
