use crate::error::ParseError;
use forge_ast::{
    Arg, BinaryOp, Block, Directive, DirectiveKind, Expr, FnDef, ImportPath, ImportStmt,
    InterpolatedPart, JobLimit, Literal, OverflowMode, Param, Platform, Program, Stmt, Type,
    UnaryOp,
};
use forge_lexer::{SpannedToken, StringPart, TokenKind};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    #[must_use]
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Self { tokens, pos: 0 }
    }

    /// # Errors
    /// Returns `ParseError` if the token stream does not form a valid program.
    pub fn parse(&mut self) -> Result<Program, ParseError> {
        let directives = self.parse_directives()?;
        let mut stmts = Vec::new();

        while !self.is_at_end() {
            self.skip_newlines();
            if self.is_at_end() {
                break;
            }

            // Guard must be here — before parse_statement
            if let TokenKind::ShebangDirective(raw) = self.peek_kind().clone() {
                let key = raw
                    .strip_prefix("forge:")
                    .and_then(|r| r.split_once(" = ").map(|(k, _)| k.trim().to_string()))
                    .unwrap_or_else(|| raw.clone());
                return Err(ParseError::DirectiveAfterStatement {
                    key,
                    line: self.current_line(),
                });
            }

            stmts.push(self.parse_statement()?);
        }

        Ok(Program { directives, stmts })
    }

    // --- Token helpers ---

    fn peek_kind(&self) -> &TokenKind {
        self.tokens
            .get(self.pos)
            .map_or(&TokenKind::Eof, |t| &t.kind)
    }

    fn peek_kind_at(&self, offset: usize) -> &TokenKind {
        self.tokens
            .get(self.pos + offset)
            .map_or(&TokenKind::Eof, |t| &t.kind)
    }

    fn advance(&mut self) -> TokenKind {
        let kind = self
            .tokens
            .get(self.pos)
            .map_or(TokenKind::Eof, |t| t.kind.clone());
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        kind
    }

    fn current_line(&self) -> usize {
        self.tokens.get(self.pos).map_or(0, |t| t.span.line)
    }

    fn check_kind(&self, kind: &TokenKind) -> bool {
        self.peek_kind() == kind
    }

    fn check_ident(&self, name: &str) -> bool {
        matches!(self.peek_kind(), TokenKind::Ident(n) if n == name)
    }

    fn expect_kind(&mut self, expected: &TokenKind) -> Result<(), ParseError> {
        if self.peek_kind() == expected {
            self.advance();
            Ok(())
        } else {
            let got = self.peek_kind().clone();
            let line = self.current_line();
            if got == TokenKind::Eof {
                Err(ParseError::UnexpectedEof {
                    expected: expected.to_string(),
                })
            } else {
                Err(ParseError::UnexpectedToken {
                    got,
                    expected: expected.to_string(),
                    line,
                })
            }
        }
    }

    fn expect_identifier(&mut self) -> Result<String, ParseError> {
        match self.peek_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            TokenKind::Eof => Err(ParseError::UnexpectedEof {
                expected: "identifier".to_string(),
            }),
            other => Err(ParseError::UnexpectedToken {
                got: other,
                expected: "identifier".to_string(),
                line: self.current_line(),
            }),
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn skip_newlines(&mut self) {
        while self.check_kind(&TokenKind::Newline) {
            self.advance();
        }
    }

    fn is_newline_or_eof(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Newline | TokenKind::Eof | TokenKind::Semicolon
        )
    }

    fn consume_statement_end(&mut self) -> Result<(), ParseError> {
        match self.peek_kind() {
            TokenKind::Newline | TokenKind::Semicolon => {
                self.advance();
                Ok(())
            }
            TokenKind::Eof | TokenKind::RBrace => Ok(()),
            _ => {
                let got = self.peek_kind().clone();
                let line = self.current_line();
                Err(ParseError::UnexpectedToken {
                    got,
                    expected: "newline or semicolon".to_string(),
                    line,
                })
            }
        }
    }

    // --- Type annotations ---

    fn parse_type_annotation(&mut self) -> Result<Type, ParseError> {
        // Unit type: ()
        if self.check_kind(&TokenKind::LParen) {
            self.advance();
            self.expect_kind(&TokenKind::RParen)?;
            return Ok(Type::Unit);
        }

        let name = self.expect_identifier()?;

        // Generic types: Name<T> or Name<K, V>
        if self.check_kind(&TokenKind::Lt) {
            self.advance();
            let inner = self.parse_type_annotation()?;
            return match name.as_str() {
                "Option" => {
                    self.expect_kind(&TokenKind::Gt)?;
                    Ok(Type::Option(Box::new(inner)))
                }
                "List" => {
                    self.expect_kind(&TokenKind::Gt)?;
                    Ok(Type::List(Box::new(inner)))
                }
                "Task" => {
                    self.expect_kind(&TokenKind::Gt)?;
                    Ok(Type::Task(Box::new(inner)))
                }
                "Result" => {
                    self.expect_kind(&TokenKind::Comma)?;
                    let err_ty = self.parse_type_annotation()?;
                    self.expect_kind(&TokenKind::Gt)?;
                    Ok(Type::Result(Box::new(inner), Box::new(err_ty)))
                }
                "Map" => {
                    self.expect_kind(&TokenKind::Comma)?;
                    let val_ty = self.parse_type_annotation()?;
                    self.expect_kind(&TokenKind::Gt)?;
                    Ok(Type::Map(Box::new(inner), Box::new(val_ty)))
                }
                _ => {
                    self.expect_kind(&TokenKind::Gt)?;
                    Ok(Type::Named(name))
                }
            };
        }

        Ok(match name.as_str() {
            "int" => Type::Int,
            "float" => Type::Float,
            "str" => Type::Str,
            "bool" => Type::Bool,
            "path" => Type::Path,
            "regex" => Type::Regex,
            "url" => Type::Url,
            _ => Type::Named(name),
        })
    }

    // --- Parameters ---

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        self.skip_newlines();
        while !self.check_kind(&TokenKind::RParen) && !self.check_kind(&TokenKind::Eof) {
            let name = self.expect_identifier()?;
            self.expect_kind(&TokenKind::Colon)?;
            let ty = self.parse_type_annotation()?;
            params.push(Param { name, ty });
            if self.check_kind(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }
        Ok(params)
    }

    // --- Block ---

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        self.expect_kind(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut stmts = Vec::new();
        while !self.check_kind(&TokenKind::RBrace) && !self.check_kind(&TokenKind::Eof) {
            stmts.push(self.parse_statement()?);
            self.skip_newlines();
        }

        // The last ExprStmt becomes the block's tail (implicit return value).
        let tail = if matches!(stmts.last(), Some(Stmt::ExprStmt(_))) {
            if let Some(Stmt::ExprStmt(expr)) = stmts.pop() {
                Some(Box::new(expr))
            } else {
                unreachable!()
            }
        } else {
            None
        };

        self.expect_kind(&TokenKind::RBrace)?;
        Ok(Block { stmts, tail })
    }

    // --- Call arguments ---

    fn parse_call_args(&mut self) -> Result<Vec<Arg>, ParseError> {
        let mut args = Vec::new();
        self.skip_newlines();
        while !self.check_kind(&TokenKind::RParen) && !self.check_kind(&TokenKind::Eof) {
            // Named argument: `name: value` (peek one ahead)
            let arg = if matches!(self.peek_kind(), TokenKind::Ident(_))
                && self.peek_kind_at(1) == &TokenKind::Colon
            {
                let name = self.expect_identifier()?;
                self.advance(); // consume ':'
                let value = self.parse_expression(0)?;
                Arg::Named { name, value }
            } else {
                Arg::Positional(self.parse_expression(0)?)
            };
            args.push(arg);
            if self.check_kind(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }
        Ok(args)
    }

    // --- List items ---

    fn parse_list_items(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut items = Vec::new();
        self.skip_newlines();
        while !self.check_kind(&TokenKind::RBracket) && !self.check_kind(&TokenKind::Eof) {
            items.push(self.parse_expression(0)?);
            if self.check_kind(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }
        Ok(items)
    }

    // --- Directives ---

    fn parse_directives(&mut self) -> Result<Vec<Directive>, ParseError> {
        let mut directives = Vec::new();

        while let TokenKind::ShebangDirective(_) = self.peek_kind().clone() {
            let line = self.current_line();
            let span = self.tokens.get(self.pos).map_or_else(
                || forge_lexer::Span {
                    start: 0,
                    end: 0,
                    line: 0,
                    col: 0,
                },
                |t| t.span.clone(),
            );
            let TokenKind::ShebangDirective(raw) = self.advance() else {
                unreachable!()
            };
            let kind = Self::parse_directive_kind(&raw, line)?;
            directives.push(Directive { kind, span });
            self.skip_newlines();
        }

        Ok(directives)
    }

    fn parse_directive_kind(raw: &str, line: usize) -> Result<DirectiveKind, ParseError> {
        if raw.starts_with('/') {
            return Ok(DirectiveKind::UnixShebang(raw.to_string()));
        }

        if let Some(rest) = raw.strip_prefix("forge:") {
            return Self::parse_forge_directive(rest, line);
        }

        tracing::warn!(
            "unknown directive form '{}' at line {} - ignored",
            raw,
            line
        );

        Ok(DirectiveKind::Unknown {
            key: raw.to_string(),
            value: String::new(),
        })
    }

    #[allow(clippy::too_many_lines)]
    fn parse_forge_directive(rest: &str, line: usize) -> Result<DirectiveKind, ParseError> {
        let (key, value) =
            rest.split_once(" = ")
                .ok_or_else(|| ParseError::InvalidDirectiveValue {
                    key: rest.to_string(),
                    value: String::new(),
                    reason: "expected '= value' after directive key".to_string(),
                    line,
                })?;

        let key = key.trim();
        let value = value.trim().trim_matches('"');

        match key {
            "description" => Ok(DirectiveKind::Description(value.to_string())),
            "author" => Ok(DirectiveKind::Author(value.to_string())),
            "min-version" => Ok(DirectiveKind::MinVersion(value.to_string())),
            "env-file" => Ok(DirectiveKind::EnvFile(value.to_string())),
            "override" => Ok(DirectiveKind::Override(value.to_string())),
            "timeout" => Ok(DirectiveKind::Timeout(value.to_string())),
            "require-env" => {
                let vars = value
                    .split(',')
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .collect();
                Ok(DirectiveKind::RequireEnv(vars))
            }
            "strict" => match value {
                "true" => Ok(DirectiveKind::Strict(true)),
                "false" => Ok(DirectiveKind::Strict(false)),
                _ => Err(ParseError::InvalidDirectiveValue {
                    key: key.to_string(),
                    value: value.to_string(),
                    reason: "expected true or false".to_string(),
                    line,
                }),
            },
            "overflow" => match value {
                "panic" => Ok(DirectiveKind::Overflow(OverflowMode::Panic)),
                "saturate" => Ok(DirectiveKind::Overflow(OverflowMode::Saturate)),
                "wrap" => Ok(DirectiveKind::Overflow(OverflowMode::Wrap)),
                _ => Err(ParseError::InvalidDirectiveValue {
                    key: key.to_string(),
                    value: value.to_string(),
                    reason: "expected one of: panic, saturate, wrap".to_string(),
                    line,
                }),
            },
            "platform" => {
                let platforms = value
                    .split(',')
                    .map(|p| match p.trim() {
                        "all" => Ok(Platform::All),
                        "unix" => Ok(Platform::Unix),
                        "linux" => Ok(Platform::Linux),
                        "macos" => Ok(Platform::MacOs),
                        "windows" => Ok(Platform::Windows),
                        unknown => Err(ParseError::InvalidDirectiveValue {
                            key: key.to_string(),
                            value: unknown.to_string(),
                            reason: "expected one of: all, unix, linux, macos, windows".to_string(),
                            line,
                        }),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(DirectiveKind::Platform(platforms))
            }
            "jobs" => match value {
                "auto" => Ok(DirectiveKind::Jobs(JobLimit::Auto)),
                n => n
                    .parse::<u32>()
                    .map(|c| DirectiveKind::Jobs(JobLimit::Count(c)))
                    .map_err(|_| ParseError::InvalidDirectiveValue {
                        key: key.to_string(),
                        value: n.to_string(),
                        reason: "expected a positive integer or 'auto'".to_string(),
                        line,
                    }),
            },
            "abi" => Err(ParseError::InvalidDirectiveValue {
                key: key.to_string(),
                value: value.to_string(),
                reason: "'abi' belongs in forge-plugin.toml, not in script directives".to_string(),
                line,
            }),
            "plugin" => Err(ParseError::InvalidDirectiveValue {
                key: key.to_string(),
                value: value.to_string(),
                reason: "'plugin' belongs in forge-plugin.toml, not in script directives"
                    .to_string(),
                line,
            }),
            unknown => {
                tracing::warn!(
                    "unknown directive '#!forge:{}' at line {} — ignored",
                    unknown,
                    line
                );
                Ok(DirectiveKind::Unknown {
                    key: unknown.to_string(),
                    value: value.to_string(),
                })
            }
        }
    }

    // --- Statements ---

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        self.skip_newlines();
        match self.peek_kind().clone() {
            TokenKind::Let => self.parse_let(),
            TokenKind::Fn => self.parse_fn_def(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Import => self.parse_import(),
            TokenKind::If => {
                let expr = self.parse_if_expr()?;
                self.consume_statement_end()?;
                Ok(Stmt::ExprStmt(expr))
            }
            TokenKind::While => self.parse_while(),
            _ => {
                let expr = self.parse_expression(0)?;

                // Assignment: `target = value`
                if self.check_kind(&TokenKind::Assign) {
                    self.advance();
                    let value = self.parse_expression(0)?;
                    self.consume_statement_end()?;
                    return Ok(Stmt::Assign {
                        target: expr,
                        value,
                    });
                }

                self.consume_statement_end()?;
                Ok(Stmt::ExprStmt(expr))
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kind(&TokenKind::Let)?;

        let mutable = if self.check_kind(&TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };

        let name = self.expect_identifier()?;

        let ty = if self.check_kind(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        self.expect_kind(&TokenKind::Assign)?;
        let value = self.parse_expression(0)?;
        self.consume_statement_end()?;
        Ok(Stmt::Let {
            name,
            mutable,
            ty,
            value,
        })
    }

    fn parse_fn_def(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kind(&TokenKind::Fn)?;
        let name = self.expect_identifier()?;
        self.expect_kind(&TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect_kind(&TokenKind::RParen)?;

        let ret_ty = if self.check_kind(&TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type_annotation()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        Ok(Stmt::FnDef(FnDef {
            name,
            params,
            ret_ty,
            body,
        }))
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kind(&TokenKind::While)?;
        let cond = self.parse_expression(0)?;
        let body = self.parse_block()?;
        Ok(Stmt::While { cond, body })
    }

    fn parse_return(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kind(&TokenKind::Return)?;
        let value = if self.is_newline_or_eof() {
            None
        } else {
            Some(self.parse_expression(0)?)
        };
        self.consume_statement_end()?;
        Ok(Stmt::Return(value))
    }

    fn parse_import(&mut self) -> Result<Stmt, ParseError> {
        self.expect_kind(&TokenKind::Import)?;

        let path = self.parse_import_path()?;

        // Optional `::{ item1, item2 }` import list
        let items = if self.check_kind(&TokenKind::Colon)
            && self.peek_kind_at(1) == &TokenKind::Colon
            && self.peek_kind_at(2) == &TokenKind::LBrace
        {
            self.advance(); // :
            self.advance(); // :
            self.advance(); // {
            let mut items = Vec::new();
            self.skip_newlines();
            while !self.check_kind(&TokenKind::RBrace) && !self.check_kind(&TokenKind::Eof) {
                items.push(self.expect_identifier()?);
                if self.check_kind(&TokenKind::Comma) {
                    self.advance();
                    self.skip_newlines();
                } else {
                    break;
                }
            }
            self.expect_kind(&TokenKind::RBrace)?;
            items
        } else {
            Vec::new()
        };

        let alias = if self.check_ident("as") {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        self.consume_statement_end()?;
        Ok(Stmt::Import(ImportStmt { path, alias, items }))
    }

    fn parse_import_path(&mut self) -> Result<ImportPath, ParseError> {
        if self.check_kind(&TokenKind::Dot) {
            // Relative path: ./segment::segment
            self.advance(); // consume '.'
            if self.check_kind(&TokenKind::Slash) {
                self.advance(); // consume '/'
            }
            let mut segments = Vec::new();
            loop {
                segments.push(self.expect_identifier()?);
                // `::` separator — stop before `::{ items }`
                if self.check_kind(&TokenKind::Colon)
                    && self.peek_kind_at(1) == &TokenKind::Colon
                    && self.peek_kind_at(2) != &TokenKind::LBrace
                {
                    self.advance();
                    self.advance();
                } else {
                    break;
                }
            }
            Ok(ImportPath::Relative(segments))
        } else {
            // Absolute path: segment::segment
            let mut segments = Vec::new();
            loop {
                segments.push(self.expect_identifier()?);
                if self.check_kind(&TokenKind::Colon)
                    && self.peek_kind_at(1) == &TokenKind::Colon
                    && self.peek_kind_at(2) != &TokenKind::LBrace
                {
                    self.advance();
                    self.advance();
                } else {
                    break;
                }
            }
            Ok(ImportPath::Absolute(segments))
        }
    }

    // --- Expressions — Pratt / precedence climbing ---
    //
    // Precedence table (higher = tighter binding):
    //   1–2   |   pipe
    //   3–4   ||  logical or
    //   5–6   &&  logical and
    //   7–8   == !=  equality
    //   9–10  < <= > >=  comparison
    //   11–12 + -  additive
    //   13–14 * / %  multiplicative

    fn parse_expression(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        while let Some(op) = self.peek_binary_op() {
            let (l_bp, r_bp) = Self::binding_power(&op);
            if l_bp < min_bp {
                break;
            }
            self.advance();
            let right = self.parse_expression(r_bp)?;
            left = Expr::BinaryOp {
                op,
                lhs: Box::new(left),
                rhs: Box::new(right),
            };
        }

        Ok(left)
    }

    fn binding_power(op: &BinaryOp) -> (u8, u8) {
        match op {
            BinaryOp::Pipe => (1, 2),
            BinaryOp::Or => (3, 4),
            BinaryOp::And => (5, 6),
            BinaryOp::Eq | BinaryOp::Ne => (7, 8),
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => (9, 10),
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::AddSat
            | BinaryOp::SubSat
            | BinaryOp::AddWrap
            | BinaryOp::SubWrap => (11, 12),
            BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::MulSat
            | BinaryOp::MulWrap
            | BinaryOp::PathJoin => (13, 14),
        }
    }

    fn peek_binary_op(&self) -> Option<BinaryOp> {
        match self.peek_kind() {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            TokenKind::Percent => Some(BinaryOp::Rem),
            TokenKind::EqEq => Some(BinaryOp::Eq),
            TokenKind::Ne => Some(BinaryOp::Ne),
            TokenKind::Lt => Some(BinaryOp::Lt),
            TokenKind::Le => Some(BinaryOp::Le),
            TokenKind::Gt => Some(BinaryOp::Gt),
            TokenKind::Ge => Some(BinaryOp::Ge),
            TokenKind::And => Some(BinaryOp::And),
            TokenKind::Or => Some(BinaryOp::Or),
            TokenKind::Pipe => Some(BinaryOp::Pipe),
            _ => None,
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek_kind().clone() {
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                })
            }
            TokenKind::Not => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                })
            }
            _ => {
                let primary = self.parse_primary()?;
                self.parse_postfix(primary)
            }
        }
    }

    /// Handle postfix: function calls, method calls, field access, indexing.
    fn parse_postfix(&mut self, mut expr: Expr) -> Result<Expr, ParseError> {
        loop {
            match self.peek_kind().clone() {
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.expect_kind(&TokenKind::RParen)?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                    };
                }
                TokenKind::Dot => {
                    self.advance();
                    let name = self.expect_identifier()?;
                    // Method call if followed by `(`
                    if self.check_kind(&TokenKind::LParen) {
                        self.advance();
                        let args = self.parse_call_args()?;
                        self.expect_kind(&TokenKind::RParen)?;
                        expr = Expr::MethodCall {
                            receiver: Box::new(expr),
                            method: name,
                            args,
                        };
                    } else {
                        expr = Expr::Field {
                            base: Box::new(expr),
                            field: name,
                        };
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.parse_expression(0)?;
                    self.expect_kind(&TokenKind::RBracket)?;
                    expr = Expr::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let line = self.current_line();

        match self.peek_kind().clone() {
            TokenKind::Integer(n) => {
                self.advance();
                Ok(Expr::Literal(Literal::Int(n)))
            }
            TokenKind::Float(f) => {
                self.advance();
                Ok(Expr::Literal(Literal::Float(f)))
            }
            TokenKind::Bool(b) => {
                self.advance();
                Ok(Expr::Literal(Literal::Bool(b)))
            }
            TokenKind::StringLit(s) => {
                self.advance();
                Ok(Expr::Literal(Literal::Str(s)))
            }
            TokenKind::InterpolatedStr(parts) => {
                self.advance();
                let mut interp_parts = Vec::new();
                for part in parts {
                    match part {
                        StringPart::Literal(s) => {
                            interp_parts.push(InterpolatedPart::Literal(s));
                        }
                        StringPart::Interpolation(tokens) => {
                            let mut sub = Parser::new(tokens);
                            let expr = sub.parse_expression(0)?;
                            interp_parts.push(InterpolatedPart::Expr(Box::new(expr)));
                        }
                    }
                }
                Ok(Expr::Interpolated(interp_parts))
            }
            TokenKind::EnvVar(name) => {
                self.advance();
                Ok(Expr::EnvVar(name))
            }
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Expr::Ident(name))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expression(0)?;
                self.expect_kind(&TokenKind::RParen)?;
                Ok(expr)
            }
            TokenKind::LBracket => {
                self.advance();
                let items = self.parse_list_items()?;
                self.expect_kind(&TokenKind::RBracket)?;
                // Represent a list literal as a call to the built-in `list` constructor
                Ok(Expr::Call {
                    callee: Box::new(Expr::Ident("list".to_string())),
                    args: items.into_iter().map(Arg::Positional).collect(),
                })
            }
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Eof => Err(ParseError::UnexpectedEof {
                expected: "expression".to_string(),
            }),
            other => Err(ParseError::InvalidExpression { got: other, line }),
        }
    }

    fn parse_if_expr(&mut self) -> Result<Expr, ParseError> {
        self.expect_kind(&TokenKind::If)?;
        let cond = self.parse_expression(0)?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.check_kind(&TokenKind::Else) {
            self.advance();
            if self.check_kind(&TokenKind::If) {
                // else-if chain
                Some(Box::new(self.parse_if_expr()?))
            } else {
                let block = self.parse_block()?;
                Some(Box::new(Expr::Block(block)))
            }
        } else {
            None
        };
        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch,
            else_branch,
        })
    }
}
