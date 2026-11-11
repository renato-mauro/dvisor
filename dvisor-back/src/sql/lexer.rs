use crate::error::DvisorResult;
use crate::throw;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Select,
    From,
    Where,
    Group,
    By,
    Order,
    Asc,
    Desc,
    As,
    Not,
    Or,
    And,
    Mod,
    Ident(String),
    Str(String),
    Num(f64),
    RelOp(String),
    Sym(char),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub pos: usize,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer { chars: input.chars().collect(), pos: 0 }
    }

    pub fn tokenize(mut self) -> DvisorResult<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            self.skip_ws();
            let start = self.pos;
            if self.pos >= self.chars.len() {
                tokens.push(Token { tok: Tok::Eof, pos: start });
                break;
            }
            let c = self.chars[self.pos];

            if c == '`' || c == '"' || c == '[' {
                let closing = if c == '[' { ']' } else { c };
                self.pos += 1;
                let s = self.scan_quoted(closing, start)?;
                tokens.push(Token { tok: Tok::Ident(s), pos: start });
                continue;
            }

            if c == '\'' {
                self.pos += 1;
                let s = self.scan_quoted('\'', start)?;
                tokens.push(Token { tok: Tok::Str(s), pos: start });
                continue;
            }

            if c.is_ascii_digit() {
                tokens.push(Token { tok: Tok::Num(self.scan_number()), pos: start });
                continue;
            }

            if c.is_ascii_alphabetic() || c == '_' {
                tokens.push(Token { tok: self.scan_ident_or_keyword(), pos: start });
                continue;
            }

            if c == '>' || c == '<' || c == '=' {
                let two: String = self.chars[self.pos..(self.pos + 2).min(self.chars.len())].iter().collect();
                if two == ">=" || two == "<=" || two == "<>" {
                    self.pos += 2;
                    tokens.push(Token { tok: Tok::RelOp(two), pos: start });
                } else {
                    self.pos += 1;
                    let op = if c == '=' { "==".to_string() } else { c.to_string() };
                    tokens.push(Token { tok: Tok::RelOp(op), pos: start });
                }
                continue;
            }

            if c == '*' && self.chars.get(self.pos + 1) == Some(&'=') {
                self.pos += 2;
                tokens.push(Token { tok: Tok::RelOp("*=".to_string()), pos: start });
                continue;
            }

            // single-char symbols: ( ) , . + - * / % ;
            self.pos += 1;
            tokens.push(Token { tok: Tok::Sym(c), pos: start });
        }
        Ok(tokens)
    }

    fn scan_quoted(&mut self, closing: char, start: usize) -> DvisorResult<String> {
        let mut s = String::new();
        loop {
            if self.pos >= self.chars.len() {
                throw!("Unterminated quoted text starting at position {}", start);
            }
            let ch = self.chars[self.pos];
            if ch == '\\' && self.pos + 1 < self.chars.len() {
                s.push(self.chars[self.pos + 1]);
                self.pos += 2;
                continue;
            }
            if ch == closing {
                self.pos += 1;
                return Ok(s);
            }
            s.push(ch);
            self.pos += 1;
        }
    }

    fn scan_number(&mut self) -> f64 {
        let mut s = String::new();
        while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
            s.push(self.chars[self.pos]);
            self.pos += 1;
        }
        if self.chars.get(self.pos) == Some(&'.') && self.chars.get(self.pos + 1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
            s.push('.');
            self.pos += 1;
            while self.pos < self.chars.len() && self.chars[self.pos].is_ascii_digit() {
                s.push(self.chars[self.pos]);
                self.pos += 1;
            }
        }
        s.parse().unwrap_or(0.0)
    }

    fn scan_ident_or_keyword(&mut self) -> Tok {
        let mut s = String::new();
        while self.pos < self.chars.len() && (self.chars[self.pos].is_ascii_alphanumeric() || self.chars[self.pos] == '_') {
            s.push(self.chars[self.pos]);
            self.pos += 1;
        }
        let upper = s.to_ascii_uppercase();
        // `mod` is both the infix keyword ("a MOD b") and a common SQL
        // function name (`mod(a,b)`, mirroring api.js's `mod`); only treat
        // it as the keyword when it isn't immediately followed by `(`.
        if upper == "MOD" && self.peek_is_open_paren() {
            return Tok::Ident(s);
        }
        match upper.as_str() {
            "SELECT" => Tok::Select,
            "FROM" => Tok::From,
            "WHERE" => Tok::Where,
            "GROUP" => Tok::Group,
            "BY" => Tok::By,
            "ORDER" => Tok::Order,
            "ASC" => Tok::Asc,
            "DESC" => Tok::Desc,
            "AS" => Tok::As,
            "NOT" => Tok::Not,
            "OR" => Tok::Or,
            "AND" => Tok::And,
            "MOD" => Tok::Mod,
            _ => Tok::Ident(s),
        }
    }

    fn peek_is_open_paren(&self) -> bool {
        let mut i = self.pos;
        while i < self.chars.len() && self.chars[i].is_whitespace() {
            i += 1;
        }
        self.chars.get(i) == Some(&'(')
    }

    fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }
}
