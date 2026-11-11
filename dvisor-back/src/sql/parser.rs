use super::ast::*;
use super::lexer::{Lexer, Tok, Token};
use crate::error::{DvisorError, DvisorResult};
use crate::throw;

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn pos_at(&self) -> usize {
        self.tokens[self.pos].pos
    }

    fn accept_sym(&mut self, c: char) -> bool {
        if matches!(self.peek(), Tok::Sym(x) if *x == c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn accept_tok(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn accept_relop(&mut self) -> Option<String> {
        if let Tok::RelOp(s) = self.peek() {
            let s = s.clone();
            self.pos += 1;
            Some(s)
        } else {
            None
        }
    }

    fn accept_ident(&mut self) -> Option<String> {
        if let Tok::Ident(s) = self.peek() {
            let s = s.clone();
            self.pos += 1;
            Some(s)
        } else {
            None
        }
    }

    fn accept_str(&mut self) -> Option<String> {
        if let Tok::Str(s) = self.peek() {
            let s = s.clone();
            self.pos += 1;
            Some(s)
        } else {
            None
        }
    }

    fn accept_num(&mut self) -> Option<f64> {
        if let Tok::Num(n) = self.peek() {
            let n = *n;
            self.pos += 1;
            Some(n)
        } else {
            None
        }
    }

    fn expect_ident(&mut self) -> DvisorResult<String> {
        self.accept_ident().ok_or_else(|| DvisorError::Msg(format!("Expected identifier at position {}", self.pos_at())))
    }

    fn expect_sym(&mut self, c: char) -> DvisorResult<()> {
        if self.accept_sym(c) {
            Ok(())
        } else {
            throw!("Expected '{}' at position {}", c, self.pos_at());
        }
    }

    fn expect_tok(&mut self, t: Tok, label: &str) -> DvisorResult<()> {
        if self.accept_tok(&t) {
            Ok(())
        } else {
            throw!("Expected {} at position {}", label, self.pos_at());
        }
    }

    fn expression(&mut self) -> DvisorResult<Expr> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> DvisorResult<Expr> {
        let mut left = self.and_expr()?;
        while self.accept_tok(&Tok::Or) {
            let right = self.and_expr()?;
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn and_expr(&mut self) -> DvisorResult<Expr> {
        let mut left = self.comp_expr()?;
        while self.accept_tok(&Tok::And) {
            let right = self.comp_expr()?;
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn comp_expr(&mut self) -> DvisorResult<Expr> {
        if self.accept_tok(&Tok::Not) {
            return Ok(Expr::Not(Box::new(self.expression()?)));
        }
        let mut left = self.sum_expr()?;
        while let Some(op) = self.accept_relop() {
            let binop = match op.as_str() {
                "=" | "==" | "*=" => BinOp::Eq,
                "<>" => BinOp::Ne,
                ">" => BinOp::Gt,
                "<" => BinOp::Lt,
                ">=" => BinOp::Ge,
                "<=" => BinOp::Le,
                _ => throw!("Unknown relational operator '{}'", op),
            };
            let right = self.sum_expr()?;
            left = Expr::Binary(binop, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn sum_expr(&mut self) -> DvisorResult<Expr> {
        let mut left = self.term()?;
        loop {
            if self.accept_sym('+') {
                let right = self.term()?;
                left = Expr::Binary(BinOp::Add, Box::new(left), Box::new(right));
            } else if self.accept_sym('-') {
                let right = self.term()?;
                left = Expr::Binary(BinOp::Sub, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn term(&mut self) -> DvisorResult<Expr> {
        let mut left = self.factor()?;
        loop {
            if self.accept_sym('*') {
                let right = self.factor()?;
                left = Expr::Binary(BinOp::Mul, Box::new(left), Box::new(right));
            } else if self.accept_sym('/') {
                let right = self.factor()?;
                left = Expr::Binary(BinOp::Div, Box::new(left), Box::new(right));
            } else if self.accept_sym('%') || self.accept_tok(&Tok::Mod) {
                let right = self.factor()?;
                left = Expr::Binary(BinOp::Mod, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn expression_list(&mut self) -> DvisorResult<Vec<Expr>> {
        let mut list = vec![self.expression()?];
        while self.accept_sym(',') {
            list.push(self.expression()?);
        }
        Ok(list)
    }

    fn factor(&mut self) -> DvisorResult<Expr> {
        if let Some(id) = self.accept_ident() {
            if self.accept_sym('.') {
                let col = self.expect_ident()?;
                return Ok(Expr::Qualified(id, col));
            }
            if self.accept_sym('(') {
                if self.accept_sym(')') {
                    return Ok(Expr::Call(id, vec![]));
                }
                // `count(*)` is sugar for `count()` — `*` isn't a general
                // wildcard argument (`sum(*)` etc. still isn't valid), just
                // this one conventional form most SQL dialects accept and
                // that's more common in practice than the bare `count()`
                // this engine otherwise expects. Compiles to the exact same
                // AST node as `count()`, so every downstream check (only
                // valid as the outermost projection call, can't appear in
                // WHERE, ...) already applies unchanged.
                if id.eq_ignore_ascii_case("count") && matches!(self.peek(), Tok::Sym('*')) {
                    self.pos += 1;
                    self.expect_sym(')')?;
                    return Ok(Expr::Call(id, vec![]));
                }
                let args = self.expression_list()?;
                self.expect_sym(')')?;
                return Ok(Expr::Call(id, args));
            }
            return Ok(Expr::Ident(id));
        }
        if self.accept_sym('-') {
            let n = self
                .accept_num()
                .ok_or_else(|| DvisorError::Msg(format!("Expected number after unary '-' at position {}", self.pos_at())))?;
            return Ok(Expr::Number(-n));
        }
        if let Some(n) = self.accept_num() {
            return Ok(Expr::Number(n));
        }
        if let Some(s) = self.accept_str() {
            return Ok(Expr::Str(s));
        }
        if self.accept_sym('(') {
            let e = self.expression()?;
            self.expect_sym(')')?;
            return Ok(e);
        }
        throw!("Expression expected at position {}", self.pos_at());
    }

    fn select_projection(&mut self) -> DvisorResult<Vec<ProjItem>> {
        let mut resp = Vec::new();
        loop {
            let expr = self.expression()?;
            if self.accept_tok(&Tok::As) {
                let alias = self.expect_ident()?;
                resp.push(ProjItem { expr, alias: Some(alias) });
            } else if let Some(alias) = self.accept_ident() {
                resp.push(ProjItem { expr, alias: Some(alias) });
            } else {
                resp.push(ProjItem { expr, alias: None });
            }
            if !self.accept_sym(',') {
                return Ok(resp);
            }
        }
    }

    fn select_table(&mut self) -> DvisorResult<TableRef> {
        let name = match self.accept_ident().or_else(|| self.accept_str()) {
            Some(n) => n,
            None => throw!("Expected table name (identifier or string) at position {}", self.pos_at()),
        };
        if self.accept_tok(&Tok::As) {
            let alias = self.expect_ident()?;
            return Ok(TableRef { name, alias: Some(alias) });
        }
        if let Some(alias) = self.accept_ident() {
            return Ok(TableRef { name, alias: Some(alias) });
        }
        Ok(TableRef { name, alias: None })
    }

    fn select_order(&mut self) -> DvisorResult<Vec<(Expr, OrderDir)>> {
        let mut resp = Vec::new();
        loop {
            let expr = self.expression()?;
            let dir = if self.accept_tok(&Tok::Asc) {
                OrderDir::Asc
            } else if self.accept_tok(&Tok::Desc) {
                OrderDir::Desc
            } else {
                OrderDir::Asc
            };
            resp.push((expr, dir));
            if !self.accept_sym(',') {
                return Ok(resp);
            }
        }
    }

    fn select_stmt(&mut self) -> DvisorResult<SelectStmt> {
        self.expect_tok(Tok::Select, "SELECT")?;
        let projection = self.select_projection()?;
        self.expect_tok(Tok::From, "FROM")?;
        let table = self.select_table()?;
        let where_clause = if self.accept_tok(&Tok::Where) { Some(self.expression()?) } else { None };
        let group_by = if self.accept_tok(&Tok::Group) {
            self.expect_tok(Tok::By, "BY")?;
            Some(self.expression_list()?)
        } else {
            None
        };
        let order_by = if self.accept_tok(&Tok::Order) {
            self.expect_tok(Tok::By, "BY")?;
            Some(self.select_order()?)
        } else {
            None
        };
        self.accept_sym(';');
        self.expect_tok(Tok::Eof, "end of input")?;
        Ok(SelectStmt { projection, table, where_clause, group_by, order_by })
    }
}

pub fn parse(sql: &str) -> DvisorResult<SelectStmt> {
    let tokens = Lexer::new(sql).tokenize()?;
    let mut p = Parser { tokens, pos: 0 };
    p.select_stmt()
}
