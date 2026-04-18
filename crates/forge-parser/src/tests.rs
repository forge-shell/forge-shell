use forge_ast::{
    Arg, BinaryOp, Block, Expr, FnDef, ImportPath, ImportStmt, Literal, Param, Program, Stmt, Type,
    UnaryOp,
};
use forge_lexer::Lexer;

use crate::{ParseError, Parser};

fn parse(src: &str) -> Result<Program, ParseError> {
    let tokens = Lexer::new(src).tokenise().expect("lex failed");
    Parser::new(tokens).parse()
}

fn parse_ok(src: &str) -> Program {
    parse(src).expect("parse failed")
}

// --- Literal parsing ---

#[test]
fn test_parse_integer_literal() {
    let prog = parse_ok("42");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Literal(Literal::Int(42)))]
    );
}

#[test]
fn test_parse_float_literal() {
    let prog = parse_ok("1.23");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Literal(Literal::Float(1.23)))]
    );
}

#[test]
fn test_parse_bool_literal() {
    let prog = parse_ok("true");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Literal(Literal::Bool(true)))]
    );
}

#[test]
fn test_parse_string_literal() {
    let prog = parse_ok(r#""hello""#);
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Literal(Literal::Str(
            "hello".to_string()
        )))]
    );
}

// --- Binary expressions ---

#[test]
fn test_parse_addition() {
    let prog = parse_ok("1 + 2");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Expr::Literal(Literal::Int(1))),
            rhs: Box::new(Expr::Literal(Literal::Int(2))),
        })]
    );
}

#[test]
fn test_operator_precedence_mul_over_add() {
    // 1 + 2 * 3  should parse as  1 + (2 * 3)
    let prog = parse_ok("1 + 2 * 3");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::BinaryOp {
            op: BinaryOp::Add,
            lhs: Box::new(Expr::Literal(Literal::Int(1))),
            rhs: Box::new(Expr::BinaryOp {
                op: BinaryOp::Mul,
                lhs: Box::new(Expr::Literal(Literal::Int(2))),
                rhs: Box::new(Expr::Literal(Literal::Int(3))),
            }),
        })]
    );
}

#[test]
fn test_operator_precedence_parens_override() {
    // (1 + 2) * 3
    let prog = parse_ok("(1 + 2) * 3");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::BinaryOp {
            op: BinaryOp::Mul,
            lhs: Box::new(Expr::BinaryOp {
                op: BinaryOp::Add,
                lhs: Box::new(Expr::Literal(Literal::Int(1))),
                rhs: Box::new(Expr::Literal(Literal::Int(2))),
            }),
            rhs: Box::new(Expr::Literal(Literal::Int(3))),
        })]
    );
}

#[test]
fn test_comparison_expr() {
    let prog = parse_ok("a == b");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::BinaryOp {
            op: BinaryOp::Eq,
            lhs: Box::new(Expr::Ident("a".to_string())),
            rhs: Box::new(Expr::Ident("b".to_string())),
        })]
    );
}

#[test]
fn test_logical_and_or() {
    // a && b || c  →  (a && b) || c  (|| has lower precedence)
    let prog = parse_ok("a && b || c");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::BinaryOp {
            op: BinaryOp::Or,
            lhs: Box::new(Expr::BinaryOp {
                op: BinaryOp::And,
                lhs: Box::new(Expr::Ident("a".to_string())),
                rhs: Box::new(Expr::Ident("b".to_string())),
            }),
            rhs: Box::new(Expr::Ident("c".to_string())),
        })]
    );
}

// --- Unary expressions ---

#[test]
fn test_unary_neg() {
    let prog = parse_ok("-42");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand: Box::new(Expr::Literal(Literal::Int(42))),
        })]
    );
}

#[test]
fn test_unary_not() {
    let prog = parse_ok("!flag");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(Expr::Ident("flag".to_string())),
        })]
    );
}

// --- Function calls ---

#[test]
fn test_function_call_no_args() {
    let prog = parse_ok("foo()");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Call {
            callee: Box::new(Expr::Ident("foo".to_string())),
            args: vec![],
        })]
    );
}

#[test]
fn test_function_call_positional_args() {
    let prog = parse_ok("add(1, 2)");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Call {
            callee: Box::new(Expr::Ident("add".to_string())),
            args: vec![
                Arg::Positional(Expr::Literal(Literal::Int(1))),
                Arg::Positional(Expr::Literal(Literal::Int(2))),
            ],
        })]
    );
}

#[test]
fn test_function_call_named_arg() {
    let prog = parse_ok("greet(name: \"alice\")");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Call {
            callee: Box::new(Expr::Ident("greet".to_string())),
            args: vec![Arg::Named {
                name: "name".to_string(),
                value: Expr::Literal(Literal::Str("alice".to_string())),
            }],
        })]
    );
}

#[test]
fn test_method_call() {
    let prog = parse_ok("list.len()");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::MethodCall {
            receiver: Box::new(Expr::Ident("list".to_string())),
            method: "len".to_string(),
            args: vec![],
        })]
    );
}

#[test]
fn test_field_access() {
    let prog = parse_ok("obj.name");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Field {
            base: Box::new(Expr::Ident("obj".to_string())),
            field: "name".to_string(),
        })]
    );
}

#[test]
fn test_index_expr() {
    let prog = parse_ok("arr[0]");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::Index {
            base: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(Expr::Literal(Literal::Int(0))),
        })]
    );
}

// --- Let bindings ---

#[test]
fn test_let_binding() {
    let prog = parse_ok("let x = 10");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Let {
            name: "x".to_string(),
            mutable: false,
            ty: None,
            value: Expr::Literal(Literal::Int(10)),
        }]
    );
}

#[test]
fn test_let_immutable_default() {
    let prog = parse_ok("let x = 42\n");
    if let Stmt::Let { mutable, .. } = &prog.stmts[0] {
        assert!(!mutable);
    } else {
        panic!("expected Let");
    }
}

#[test]
fn test_let_mut_binding() {
    let prog = parse_ok("let mut count = 0");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Let {
            name: "count".to_string(),
            mutable: true,
            ty: None,
            value: Expr::Literal(Literal::Int(0)),
        }]
    );
}

#[test]
fn test_let_typed_binding() {
    let prog = parse_ok("let x: int = 5");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Let {
            name: "x".to_string(),
            mutable: false,
            ty: Some(Type::Int),
            value: Expr::Literal(Literal::Int(5)),
        }]
    );
}

// --- Function definitions ---

#[test]
fn test_fn_def_no_params() {
    let prog = parse_ok("fn greet() { }");
    assert_eq!(
        prog.stmts,
        vec![Stmt::FnDef(FnDef {
            name: "greet".to_string(),
            params: vec![],
            ret_ty: None,
            body: Block {
                stmts: vec![],
                tail: None,
            },
        })]
    );
}

#[test]
fn test_fn_def_with_params_and_return_type() {
    let prog = parse_ok("fn add(a: int, b: int) -> int { a + b }");
    assert_eq!(
        prog.stmts,
        vec![Stmt::FnDef(FnDef {
            name: "add".to_string(),
            params: vec![
                Param {
                    name: "a".to_string(),
                    ty: Type::Int,
                },
                Param {
                    name: "b".to_string(),
                    ty: Type::Int,
                },
            ],
            ret_ty: Some(Type::Int),
            body: Block {
                stmts: vec![],
                tail: Some(Box::new(Expr::BinaryOp {
                    op: BinaryOp::Add,
                    lhs: Box::new(Expr::Ident("a".to_string())),
                    rhs: Box::new(Expr::Ident("b".to_string())),
                })),
            },
        })]
    );
}

// --- Multi-statement programs ---

#[test]
fn test_multi_statement_program() {
    let prog = parse_ok("let x = 1\nlet y = 2\nx + y");
    assert_eq!(prog.stmts.len(), 3);
    assert!(matches!(prog.stmts[0], Stmt::Let { name: ref n, .. } if n == "x"));
    assert!(matches!(prog.stmts[1], Stmt::Let { name: ref n, .. } if n == "y"));
    assert!(matches!(prog.stmts[2], Stmt::ExprStmt(_)));
}

#[test]
fn test_assignment_statement() {
    let prog = parse_ok("x = 42");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Assign {
            target: Expr::Ident("x".to_string()),
            value: Expr::Literal(Literal::Int(42)),
        }]
    );
}

// --- Return ---

#[test]
fn test_return_with_value() {
    let prog = parse_ok("return 99");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Return(Some(Expr::Literal(Literal::Int(99))))]
    );
}

#[test]
fn test_return_unit() {
    let prog = parse_ok("return");
    assert_eq!(prog.stmts, vec![Stmt::Return(None)]);
}

// --- While ---

#[test]
fn test_while_loop() {
    let prog = parse_ok("while true { }");
    assert!(matches!(
        prog.stmts[0],
        Stmt::While {
            cond: Expr::Literal(Literal::Bool(true)),
            ..
        }
    ));
}

// --- If expression ---

#[test]
fn test_if_expr_no_else() {
    let prog = parse_ok("if x { }");
    assert!(matches!(prog.stmts[0], Stmt::ExprStmt(Expr::If { .. })));
}

#[test]
fn test_if_else_expr() {
    let prog = parse_ok("if a { 1 } else { 2 }");
    if let Stmt::ExprStmt(Expr::If {
        else_branch: Some(else_),
        ..
    }) = &prog.stmts[0]
    {
        assert!(matches!(else_.as_ref(), Expr::Block(_)));
    } else {
        panic!("expected if-else expr");
    }
}

// --- Imports ---

#[test]
fn test_import_absolute_path() {
    let prog = parse_ok("import forge::fs");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Import(ImportStmt {
            path: ImportPath::Absolute(vec!["forge".to_string(), "fs".to_string()]),
            alias: None,
            items: vec![],
        })]
    );
}

#[test]
fn test_import_relative_path() {
    let prog = parse_ok("import ./utils");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Import(ImportStmt {
            path: ImportPath::Relative(vec!["utils".to_string()]),
            alias: None,
            items: vec![],
        })]
    );
}

#[test]
fn test_import_with_alias() {
    let prog = parse_ok("import forge::fs as fs");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Import(ImportStmt {
            path: ImportPath::Absolute(vec!["forge".to_string(), "fs".to_string()]),
            alias: Some("fs".to_string()),
            items: vec![],
        })]
    );
}

#[test]
fn test_import_with_items() {
    let prog = parse_ok("import ./utils::{ read, write }");
    assert_eq!(
        prog.stmts,
        vec![Stmt::Import(ImportStmt {
            path: ImportPath::Relative(vec!["utils".to_string()]),
            alias: None,
            items: vec!["read".to_string(), "write".to_string()],
        })]
    );
}

// --- Pipe operator ---

#[test]
fn test_pipe_operator() {
    let prog = parse_ok("ls | grep(\"foo\")");
    assert_eq!(
        prog.stmts,
        vec![Stmt::ExprStmt(Expr::BinaryOp {
            op: BinaryOp::Pipe,
            lhs: Box::new(Expr::Ident("ls".to_string())),
            rhs: Box::new(Expr::Call {
                callee: Box::new(Expr::Ident("grep".to_string())),
                args: vec![Arg::Positional(Expr::Literal(Literal::Str(
                    "foo".to_string()
                )))],
            }),
        })]
    );
}

#[test]
fn test_env_var_expression() {
    let prog = parse_ok("let x = $HOME\n");
    if let Stmt::Let {
        value: Expr::EnvVar(name),
        ..
    } = &prog.stmts[0]
    {
        assert_eq!(name, "HOME");
    } else {
        panic!("expected EnvVar");
    }
}

#[test]
fn test_env_var_in_binary_expr() {
    let prog = parse_ok("let x = $HOME\n");
    assert!(matches!(
        prog.stmts[0],
        Stmt::Let {
            value: Expr::EnvVar(_),
            ..
        }
    ));
}

// --- Error cases ---

#[test]
fn test_unexpected_eof() {
    let err = parse("let x =").unwrap_err();
    assert!(matches!(err, ParseError::UnexpectedEof { .. }));
}

#[test]
fn test_invalid_expression() {
    let err = parse("let x = )").unwrap_err();
    assert!(matches!(err, ParseError::InvalidExpression { .. }));
}
