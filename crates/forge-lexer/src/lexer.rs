use crate::error::LexError;
use crate::{Span, SpannedToken, StringPart, TokenKind};

/// Transforms a `ForgeScript` source string into a flat sequence of [`SpannedToken`]s.
///
/// # Example
///
/// ```
/// use forge_lexer::{Lexer, TokenKind};
///
/// let mut lexer = Lexer::new("let x = 42");
/// let tokens = lexer.tokenise().unwrap();
/// assert_eq!(tokens[0].kind, TokenKind::Let);
/// ```
pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    #[must_use]
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// # Errors
    ///
    /// Returns a [`LexError`] if an unexpected character, unterminated string, or invalid number
    /// literal is encountered.
    pub fn tokenise(&mut self) -> Result<Vec<SpannedToken>, LexError> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();

            if self.is_at_end() {
                tokens.push(SpannedToken::new(TokenKind::Eof, self.current_span(0)));
                break;
            }

            let token = self.next_token()?;
            tokens.push(token);
        }

        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<SpannedToken, LexError> {
        let start_line = self.line;
        let start_col = self.col;
        let start_pos = self.pos;

        let ch = self.advance();

        let kind = match ch {
            '\n' => TokenKind::Newline,
            '\r' => {
                if matches!(self.peek(), Some('\n')) {
                    self.advance();
                }
                TokenKind::Newline
            }
            '"' => self.lex_string(start_line, start_col)?,
            c if c.is_ascii_digit() => self.lex_number(c)?,
            c if c.is_alphabetic() || c == '_' => self.lex_identifier_or_keyword(c),
            c => self.lex_symbol(c, start_line, start_col)?,
        };

        Ok(SpannedToken::new(
            kind,
            Span {
                start: start_pos,
                end: self.pos,
                line: start_line,
                col: start_col,
            },
        ))
    }

    fn lex_symbol(
        &mut self,
        ch: char,
        start_line: usize,
        start_col: usize,
    ) -> Result<TokenKind, LexError> {
        let kind = match ch {
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            ':' => TokenKind::Colon,
            '$' => TokenKind::Dollar,
            '@' => TokenKind::At,
            '#' => self.lex_shebang_directive(),
            '-' => {
                if self.advance_if('>') {
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '.' => {
                if self.advance_if('.') {
                    TokenKind::DotDot
                } else {
                    TokenKind::Dot
                }
            }
            '|' => {
                if self.advance_if('|') {
                    TokenKind::Or
                } else {
                    TokenKind::Pipe
                }
            }
            '&' => {
                if self.advance_if('&') {
                    TokenKind::And
                } else {
                    TokenKind::Amp
                }
            }
            '!' => {
                if self.advance_if('=') {
                    TokenKind::Ne
                } else {
                    TokenKind::Not
                }
            }
            '<' => {
                if self.advance_if('=') {
                    TokenKind::Le
                } else {
                    TokenKind::Lt
                }
            }
            '=' => {
                if self.advance_if('=') {
                    TokenKind::EqEq
                } else if self.advance_if('>') {
                    TokenKind::FatArrow
                } else {
                    TokenKind::Assign
                }
            }
            '>' => {
                if self.advance_if('=') {
                    TokenKind::Ge
                } else if self.advance_if('>') {
                    TokenKind::RedirectAppend
                } else {
                    TokenKind::Gt
                }
            }
            unexpected => {
                return Err(LexError::UnexpectedChar {
                    ch: unexpected,
                    line: start_line,
                    col: start_col,
                });
            }
        };
        Ok(kind)
    }

    fn lex_string(&mut self, start_line: usize, start_col: usize) -> Result<TokenKind, LexError> {
        let mut s = String::new();
        let mut parts: Vec<StringPart> = Vec::new();
        let mut has_interp = false;

        loop {
            match self.peek().copied() {
                None => {
                    return Err(LexError::UnterminatedString {
                        line: start_line,
                        col: start_col,
                    });
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('{') => {
                    if self.peek_next() == Some(&'{') {
                        // `{{` → literal `{`
                        self.advance();
                        self.advance();
                        s.push('{');
                    } else {
                        // start of interpolation
                        has_interp = true;
                        parts.push(StringPart::Literal(std::mem::take(&mut s)));
                        self.advance(); // consume `{`
                        let expr_tokens = self.lex_interp_part(start_line, start_col)?;
                        parts.push(StringPart::Interpolation(expr_tokens));
                    }
                }
                Some('}') => {
                    if self.peek_next() == Some(&'}') {
                        // `}}` → literal `}`
                        self.advance();
                        self.advance();
                        s.push('}');
                    } else {
                        s.push(self.advance());
                    }
                }
                Some('\\') => {
                    self.advance();
                    let escaped = match self.advance() {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '"' => '"',
                        '\\' => '\\',
                        other => other,
                    };
                    s.push(escaped);
                }
                Some(_) => s.push(self.advance()),
            }
        }

        if has_interp {
            parts.push(StringPart::Literal(s));
            Ok(TokenKind::InterpolatedStr(parts))
        } else {
            Ok(TokenKind::StringLit(s))
        }
    }

    /// Lex the expression tokens inside `{...}` until the depth-0 closing `}`.
    /// The opening `{` must already have been consumed by the caller.
    fn lex_interp_part(
        &mut self,
        str_start_line: usize,
        str_start_col: usize,
    ) -> Result<Vec<SpannedToken>, LexError> {
        let mut tokens = Vec::new();
        let mut depth = 0usize;

        loop {
            self.skip_whitespace_and_comments();

            if self.is_at_end() {
                return Err(LexError::UnterminatedInterpolation {
                    line: str_start_line,
                    col: str_start_col,
                });
            }

            match self.peek().copied() {
                Some('}') if depth == 0 => {
                    self.advance(); // consume closing `}`
                    break;
                }
                Some('{') => {
                    depth += 1;
                    tokens.push(self.next_token()?);
                }
                Some('}') => {
                    // depth > 0: closing a nested brace pair inside the expression
                    depth -= 1;
                    tokens.push(self.next_token()?);
                }
                _ => {
                    tokens.push(self.next_token()?);
                }
            }
        }

        Ok(tokens)
    }

    fn lex_number(&mut self, first: char) -> Result<TokenKind, LexError> {
        let mut num = String::new();
        num.push(first);

        while self.peek().is_some_and(char::is_ascii_digit) {
            num.push(self.advance());
        }

        // Check for float
        if matches!(self.peek(), Some('.')) && self.peek_next().is_some_and(char::is_ascii_digit) {
            num.push(self.advance()); // consume '.'
            while self.peek().is_some_and(char::is_ascii_digit) {
                num.push(self.advance());
            }
            return num
                .parse::<f64>()
                .map(TokenKind::Float)
                .map_err(|_| LexError::InvalidNumber {
                    literal: num.clone(),
                    line: self.line,
                    col: self.col,
                });
        }

        num.parse::<i64>()
            .map(TokenKind::Integer)
            .map_err(|_| LexError::InvalidNumber {
                literal: num.clone(),
                line: self.line,
                col: self.col,
            })
    }

    fn lex_identifier_or_keyword(&mut self, first: char) -> TokenKind {
        let mut ident = String::new();
        ident.push(first);

        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphabetic() || *c == '_')
        {
            ident.push(self.advance());
        }

        match ident.as_str() {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "return" => TokenKind::Return,
            "import" => TokenKind::Import,
            "export" => TokenKind::Export,
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            _ => TokenKind::Ident(ident),
        }
    }

    fn lex_shebang_directive(&mut self) -> TokenKind {
        // Consume '#' and '!'
        self.advance();
        self.advance();

        let mut content = String::new();
        while self.peek().is_some_and(|c| *c != '\n') {
            content.push(self.advance());
        }

        TokenKind::ShebangDirective(content.trim().to_string())
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(' ' | '\t') => {
                    self.advance();
                }
                Some('#') => {
                    if self.peek_next() == Some(&'!') {
                        break;
                    }
                    while self.peek().is_some_and(|c| *c != '\n') {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn advance_if(&mut self, ch: char) -> bool {
        if self.peek() == Some(&ch) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> char {
        let ch = self.source[self.pos];
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        ch
    }

    fn peek(&self) -> Option<&char> {
        self.source.get(self.pos)
    }

    fn peek_next(&self) -> Option<&char> {
        self.source.get(self.pos + 1)
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn current_span(&self, len: usize) -> Span {
        Span {
            start: self.pos,
            end: self.pos + len,
            line: self.line,
            col: self.col,
        }
    }
}
