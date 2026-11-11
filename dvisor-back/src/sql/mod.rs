mod ast;
mod compiler;
mod lexer;
mod parser;

use crate::error::DvisorResult;

/// Compiles a single SQL SELECT statement into the JSON query format
/// already understood by `crate::query::ql::Query`. This is purely a
/// frontend: execution still goes through the existing JSON pipeline
/// unchanged (see `Dvisor::exec`).
pub fn compile_sql(sql: &str) -> DvisorResult<String> {
    let stmt = parser::parse(sql)?;
    let query = compiler::compile(stmt)?;
    Ok(serde_json::to_string(&query)?)
}

#[cfg(test)]
mod tests;
