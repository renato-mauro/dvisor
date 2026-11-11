#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Ident(String),
    Qualified(String, String),
    Number(f64),
    Str(String),
    Call(String, Vec<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct ProjItem {
    pub expr: Expr,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TableRef {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SelectStmt {
    pub projection: Vec<ProjItem>,
    pub table: TableRef,
    pub where_clause: Option<Expr>,
    pub group_by: Option<Vec<Expr>>,
    pub order_by: Option<Vec<(Expr, OrderDir)>>,
}
