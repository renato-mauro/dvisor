use std::{format, matches};

use super::ast::{BinOp, Expr, OrderDir, ProjItem, SelectStmt};
use crate::error::DvisorResult;
use crate::model::enums::{Operator, SortOrder};
use crate::query::ql::{get_api_function_name, CalculatedExpression, Predicate, ProjectItem, Query, SortItem, Value};
use crate::{err, throw};

const AGGREGATE_NAMES: [&str; 6] = ["sum", "count", "min", "max", "avg", "first"];

fn is_aggregate_call(expr: &Expr) -> Option<&str> {
    if let Expr::Call(name, _) = expr {
        if AGGREGATE_NAMES.contains(&name.to_ascii_lowercase().as_str()) {
            return Some(name.as_str());
        }
    }
    None
}

/// Finds an aggregate call nested anywhere inside a non-aggregate
/// expression (e.g. `round(avg(x), 2)`), which dvisor cannot execute:
/// aggregates are only recognized as the sole outermost call of a
/// projection item.
fn find_nested_aggregate(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Call(name, args) => {
            if AGGREGATE_NAMES.contains(&name.to_ascii_lowercase().as_str()) {
                return Some(name.as_str());
            }
            args.iter().find_map(find_nested_aggregate)
        }
        Expr::Binary(_, l, r) => find_nested_aggregate(l).or_else(|| find_nested_aggregate(r)),
        Expr::Not(e) => find_nested_aggregate(e),
        _ => None,
    }
}

fn default_aggregate_alias(agg_name: &str, inner_rendered: &str) -> String {
    match agg_name.to_ascii_lowercase().as_str() {
        "first" => inner_rendered.to_string(),
        "sum" => format!("sum_{}", inner_rendered),
        "min" => format!("min_{}", inner_rendered),
        "max" => format!("max_{}", inner_rendered),
        "avg" => format!("avg_{}", inner_rendered),
        "count" => "count".to_string(),
        _ => inner_rendered.to_string(),
    }
}

fn binop_js(op: BinOp) -> &'static str {
    match op {
        BinOp::Or => "||",
        BinOp::And => "&&",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
    }
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn escape_js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Renders an `Expr` back into JS source text, resolving `alias.column`
/// references to bare column names along the way.
///
/// `top` controls parenthesization: nested operands are always fully
/// parenthesized (safe regardless of any precedence mismatch between SQL
/// and JS), while the outermost expression of a projection/calc body and
/// each call argument (already delimited by `(`, `,`, `)`) are rendered
/// with `top = true` to avoid redundant wrapping parens.
///
/// `resolve_api_calls` controls how a `Expr::Call` is rendered:
///   - `false` (used for `project[]`): emitted as bare `name(args)` text.
///     dvisor's own `extract_functions()` resolves such calls against the
///     `api` module for project items — we must leave the text alone so
///     that mechanism keeps working.
///   - `true` (used for the `calc[]` entries WE synthesize to hoist a
///     WHERE expression): dvisor never runs `extract_functions()` over
///     `calc[]`, so nothing else would resolve the call. We render it as
///     `api.<ResolvedName>(args)` ourselves instead, using the same
///     case-insensitive lookup (`getApiFunctionName`) the runtime uses.
fn render(expr: &Expr, table_alias: Option<&str>, top: bool, resolve_api_calls: bool) -> DvisorResult<String> {
    match expr {
        Expr::Ident(name) => Ok(name.clone()),
        Expr::Qualified(alias, col) => {
            if table_alias == Some(alias.as_str()) {
                Ok(col.clone())
            } else {
                err!("Table alias '{}' not found (in '{}.{}')", alias, alias, col)
            }
        }
        Expr::Number(n) => Ok(format_number(*n)),
        Expr::Str(s) => Ok(format!("'{}'", escape_js_string(s))),
        Expr::Call(name, args) => {
            let mut parts = Vec::with_capacity(args.len());
            for a in args {
                parts.push(render(a, table_alias, true, resolve_api_calls)?);
            }
            if resolve_api_calls {
                if AGGREGATE_NAMES.contains(&name.to_ascii_lowercase().as_str()) {
                    return err!("Aggregate function '{}' cannot be used in a WHERE clause", name);
                }
                let resolved = get_api_function_name(name)?;
                Ok(format!("api.{}({})", resolved, parts.join(",")))
            } else {
                Ok(format!("{}({})", name, parts.join(",")))
            }
        }
        Expr::Not(e) => {
            let inner = format!("!{}", render(e, table_alias, false, resolve_api_calls)?);
            Ok(if top { inner } else { format!("({inner})") })
        }
        Expr::Binary(op, l, r) => {
            let ls = render(l, table_alias, false, resolve_api_calls)?;
            let rs = render(r, table_alias, false, resolve_api_calls)?;
            let inner = format!("{}{}{}", ls, binop_js(*op), rs);
            Ok(if top { inner } else { format!("({inner})") })
        }
    }
}

fn resolve_and_render(expr: &Expr, table_alias: Option<&str>) -> DvisorResult<String> {
    render(expr, table_alias, true, false)
}

fn collect_idents(expr: &Expr, table_alias: Option<&str>, out: &mut Vec<String>) -> DvisorResult<()> {
    match expr {
        Expr::Ident(name) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::Qualified(alias, col) => {
            if table_alias == Some(alias.as_str()) {
                if !out.contains(col) {
                    out.push(col.clone());
                }
            } else {
                throw!("Table alias '{}' not found (in '{}.{}')", alias, alias, col);
            }
        }
        Expr::Number(_) | Expr::Str(_) => {}
        Expr::Call(_, args) => {
            for a in args {
                collect_idents(a, table_alias, out)?;
            }
        }
        Expr::Not(e) => collect_idents(e, table_alias, out)?,
        Expr::Binary(_, l, r) => {
            collect_idents(l, table_alias, out)?;
            collect_idents(r, table_alias, out)?;
        }
    }
    Ok(())
}

fn flatten_and(expr: &Expr, out: &mut Vec<Expr>) {
    if let Expr::Binary(BinOp::And, l, r) = expr {
        flatten_and(l, out);
        flatten_and(r, out);
    } else {
        out.push(expr.clone());
    }
}

fn invert(op: BinOp) -> BinOp {
    match op {
        BinOp::Lt => BinOp::Gt,
        BinOp::Gt => BinOp::Lt,
        BinOp::Le => BinOp::Ge,
        BinOp::Ge => BinOp::Le,
        other => other,
    }
}

fn is_literal(e: &Expr) -> bool {
    matches!(e, Expr::Number(_) | Expr::Str(_))
}

fn to_operator(op: BinOp) -> DvisorResult<Operator> {
    Ok(match op {
        BinOp::Eq => Operator::EQ,
        BinOp::Ne => Operator::NE,
        BinOp::Lt => Operator::LT,
        BinOp::Le => Operator::LE,
        BinOp::Gt => Operator::GT,
        BinOp::Ge => Operator::GE,
        _ => return err!("Unsupported operator in WHERE clause"),
    })
}

fn to_value(e: &Expr) -> DvisorResult<Value> {
    match e {
        Expr::Number(n) => {
            if n.fract() == 0.0 && *n >= i32::MIN as f64 && *n <= i32::MAX as f64 {
                Ok(Value::Int(*n as i32))
            } else {
                Ok(Value::Float(*n as f32))
            }
        }
        Expr::Str(s) => Ok(Value::String(s.clone())),
        _ => err!("Filter value must be a constant (number or string)"),
    }
}

fn to_i32_exact(n: f64) -> Option<i32> {
    if n.fract() == 0.0 && n >= i32::MIN as f64 && n <= i32::MAX as f64 {
        Some(n as i32)
    } else {
        None
    }
}

/// If `expr` is a bare column (or `alias.column` with `alias` matching
/// `table_alias`), returns its name. Returns `Ok(None)` for anything else
/// (so callers can just decline to rewrite); still reports an *unknown*
/// table alias as a real error, consistent with every other place in this
/// file that resolves `alias.column`.
fn bare_column_name(expr: &Expr, table_alias: Option<&str>) -> DvisorResult<Option<String>> {
    match expr {
        Expr::Ident(name) => Ok(Some(name.clone())),
        Expr::Qualified(alias, col) => {
            if table_alias == Some(alias.as_str()) {
                Ok(Some(col.clone()))
            } else {
                err!("Table alias '{}' not found (in '{}.{}')", alias, alias, col)
            }
        }
        _ => Ok(None),
    }
}

/// `year(col) op Y` (or `dateTruncYear(col) op Y`, its alias) has an exact
/// range equivalent on `col` itself, since dvisor's Date columns carry no
/// time-of-day component: year(d) = Y is precisely d in [Jan 1 Y, Dec 31 Y].
/// `NE` has no single-range equivalent and is intentionally not handled
/// here (falls through to the calc[]-hoisting path, unchanged behavior).
///
/// The predicates use `Value::String("YYYY-MM-DD")`, not `Value::Int` —
/// Date columns are filtered through `StringOperator`/`filter_date` (which
/// parses the string with `dateutils::epoch` at execution time), never
/// through `IntOperator::filter_int`, which explicitly rejects
/// `TileData::Date` ("Tile is not int."). `check_feasible_string` already
/// handles a `TileStatistics::Date` column by parsing the string operand
/// the same way, so this also participates in row-group pruning correctly.
fn year_range_predicates(col: &str, op: BinOp, year: i32) -> Option<Vec<Predicate>> {
    let jan1 = format!("{:04}-01-01", year);
    let dec31 = format!("{:04}-12-31", year);
    let pred = |oper: Operator, value: String| Predicate { expr: col.to_string(), oper, value: Value::String(value) };
    Some(match op {
        BinOp::Eq => vec![pred(Operator::GE, jan1), pred(Operator::LE, dec31)],
        BinOp::Ge => vec![pred(Operator::GE, jan1)],
        BinOp::Lt => vec![pred(Operator::LT, jan1)],
        BinOp::Le => vec![pred(Operator::LE, dec31)],
        BinOp::Gt => vec![pred(Operator::GT, dec31)],
        _ => return None,
    })
}

/// `bucket(col, width) op K` (see api.js's `bucket`, `= floor(col/width)*width`)
/// has an exact range equivalent on `col` whenever `K` is itself a
/// multiple of `width` — i.e. K actually is some bucket's lower bound, not
/// an unreachable value in between. If it isn't, this declines (`None`)
/// rather than guess; the calc[]-hoisting path still handles it correctly
/// (as an always-empty-for-this-fragment condition), just without pruning.
fn bucket_range_predicates(col: &str, op: BinOp, k: f64, width: f64) -> Option<Vec<Predicate>> {
    if width <= 0.0 {
        return None;
    }
    let ratio = k / width;
    if (ratio - ratio.round()).abs() > 1e-9 {
        return None;
    }
    let k_i = to_i32_exact(k)?;
    let k_plus_w_i = to_i32_exact(k + width)?;
    Some(match op {
        BinOp::Eq => vec![
            Predicate { expr: col.to_string(), oper: Operator::GE, value: Value::Int(k_i) },
            Predicate { expr: col.to_string(), oper: Operator::LT, value: Value::Int(k_plus_w_i) },
        ],
        BinOp::Lt => vec![Predicate { expr: col.to_string(), oper: Operator::LT, value: Value::Int(k_i) }],
        BinOp::Le => vec![Predicate { expr: col.to_string(), oper: Operator::LT, value: Value::Int(k_plus_w_i) }],
        BinOp::Gt => vec![Predicate { expr: col.to_string(), oper: Operator::GE, value: Value::Int(k_plus_w_i) }],
        BinOp::Ge => vec![Predicate { expr: col.to_string(), oper: Operator::GE, value: Value::Int(k_i) }],
        _ => return None,
    })
}

/// Entry point for the pruning-oriented WHERE rewrites — see the two
/// pattern-specific functions above. Only ever narrows to an exactly
/// equivalent condition; anything not recognized returns `Ok(None)` and
/// the caller falls back to today's unchanged calc[]-hoisting behavior.
pub(crate) fn rewrite_for_pruning(expr: &Expr, op: BinOp, value: &Expr, table_alias: Option<&str>) -> DvisorResult<Option<Vec<Predicate>>> {
    if matches!(op, BinOp::Ne) {
        return Ok(None);
    }
    let Expr::Call(fname, args) = expr else { return Ok(None) };
    let Expr::Number(value_n) = value else { return Ok(None) };
    let fname_lc = fname.to_ascii_lowercase();

    if (fname_lc == "year" || fname_lc == "datetruncyear") && args.len() == 1 {
        let Some(col) = bare_column_name(&args[0], table_alias)? else { return Ok(None) };
        if value_n.fract() != 0.0 {
            return Ok(None);
        }
        let Some(year) = to_i32_exact(*value_n) else { return Ok(None) };
        return Ok(year_range_predicates(&col, op, year));
    }

    if fname_lc == "bucket" && args.len() == 2 {
        let Some(col) = bare_column_name(&args[0], table_alias)? else { return Ok(None) };
        let Expr::Number(width) = &args[1] else { return Ok(None) };
        return Ok(bucket_range_predicates(&col, op, *value_n, *width));
    }

    Ok(None)
}

fn compile_where(where_expr: &Expr, table_alias: Option<&str>) -> DvisorResult<(Vec<Predicate>, Vec<CalculatedExpression>)> {
    let mut conjuncts = Vec::new();
    flatten_and(where_expr, &mut conjuncts);

    let mut filters = Vec::new();
    let mut calcs = Vec::new();
    let mut calc_n = 0;

    for conj in &conjuncts {
        let (op, lhs, rhs) = match conj {
            Expr::Binary(op, l, r)
                if !matches!(op, BinOp::And | BinOp::Or | BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod) =>
            {
                (*op, l.as_ref(), r.as_ref())
            }
            _ => throw!("Unsupported WHERE clause: only comparisons joined by AND are allowed (no OR/NOT)"),
        };

        let (expr_side, value_side, op) = if is_literal(lhs) && !is_literal(rhs) {
            (rhs, lhs, invert(op))
        } else if !is_literal(lhs) && is_literal(rhs) {
            (lhs, rhs, op)
        } else {
            throw!("Each WHERE comparison must be between a column/expression and a constant value");
        };

        // A handful of calculated-column patterns have an EXACT equivalent
        // range condition on the underlying physical column (not just an
        // implication) — rewriting to that range, instead of hoisting into
        // calc[], lets the row-group viability guard (check_feasible_*,
        // see dvisor-back/src/model/metadata.rs) prune fragments it
        // otherwise couldn't, and skips the calc-column machinery for this
        // predicate entirely. Anything not matching one of these exact
        // patterns falls through unchanged to the calc[]-hoisting path below.
        if let Some(rewritten) = rewrite_for_pruning(expr_side, op, value_side, table_alias)? {
            filters.extend(rewritten);
            continue;
        }

        let oper = to_operator(op)?;
        let value = to_value(value_side)?;

        let filter_expr = match expr_side {
            Expr::Ident(name) => name.clone(),
            Expr::Qualified(alias, col) => {
                if table_alias == Some(alias.as_str()) {
                    col.clone()
                } else {
                    throw!("Table alias '{}' not found (in '{}.{}')", alias, alias, col);
                }
            }
            _ => {
                calc_n += 1;
                let alias = format!("_where{calc_n}");
                let mut params = Vec::new();
                collect_idents(expr_side, table_alias, &mut params)?;
                let body = render(expr_side, table_alias, true, true)?;
                let arrow = format!("({})=>{}", params.join(","), body);
                calcs.push(CalculatedExpression { expr: arrow, alias: alias.clone(), tp: None });
                alias
            }
        };

        filters.push(Predicate { expr: filter_expr, oper, value });
    }

    Ok((filters, calcs))
}

fn compile_projection(items: &[ProjItem], table_alias: Option<&str>) -> DvisorResult<Vec<ProjectItem>> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        // Aggregate calls are compiled to the `aggregate` field directly
        // rather than left as `"name(args)"` text for dvisor to detect on
        // its own: its text-based detector only recognizes sum/count/min/
        // max/avg, not `first`, and this is more direct anyway since we
        // already have the parsed call shape.
        if let (Some(agg_name), Expr::Call(_, args)) = (is_aggregate_call(&item.expr), &item.expr) {
            let expr = match args.as_slice() {
                [] => None,
                [single] => Some(render(single, table_alias, true, false)?),
                _ => throw!("Aggregate function '{}' takes at most one argument", agg_name),
            };
            out.push(ProjectItem { expr, aggregate: Some(agg_name.to_string()), alias: item.alias.clone() });
            continue;
        }
        if let Some(agg_name) = find_nested_aggregate(&item.expr) {
            throw!(
                "Aggregate function '{}' can only be the outermost function of a SELECT item (e.g. `sum(x)`), not nested inside another call like `round({}(...))`; apply the outer function after querying instead",
                agg_name, agg_name
            );
        }
        let text = resolve_and_render(&item.expr, table_alias)?;
        out.push(ProjectItem { expr: Some(text), aggregate: None, alias: item.alias.clone() });
    }
    Ok(out)
}

// A bare positive integer in GROUP BY/ORDER BY is always positional in SQL
// (never a literal constant value — no real engine treats `GROUP BY 5` as
// "group by the constant 5"), so any numeric literal there is resolved as a
// 1-based index into the SELECT list. Shared by both clauses so the error
// wording and range-checking stay identical between them.
fn ordinal_index(n: f64, len: usize, clause: &str) -> DvisorResult<usize> {
    if n.fract() != 0.0 || n < 1.0 || n as usize > len {
        throw!(
            "{} position {} is not in the SELECT list (it has {} item{})",
            clause, format_number(n), len, if len == 1 { "" } else { "s" }
        );
    }
    Ok(n as usize - 1)
}

/// Replaces a bare numeric literal in a GROUP BY list with the actual
/// expression at that position in the SELECT list, so the rest of GROUP BY
/// validation (below) never has to know ordinals exist — it just sees the
/// real expression, exactly as if the user had spelled it out instead of
/// repeating it (the point of supporting ordinals at all: not having to
/// retype a computed expression, only a bare column name).
fn resolve_group_by_ordinals(group_by: &[Expr], projection: &[ProjItem]) -> DvisorResult<Vec<Expr>> {
    group_by
        .iter()
        .map(|e| match e {
            Expr::Number(n) => {
                let idx = ordinal_index(*n, projection.len(), "GROUP BY")?;
                Ok(projection[idx].expr.clone())
            }
            other => Ok(other.clone()),
        })
        .collect()
}

fn validate_group_by(group_by: &Option<Vec<Expr>>, projection: &[ProjItem], table_alias: Option<&str>) -> DvisorResult<()> {
    let Some(raw_group_exprs) = group_by else { return Ok(()) };
    let group_exprs = resolve_group_by_ordinals(raw_group_exprs, projection)?;

    let plain_exprs: Vec<String> = projection
        .iter()
        .filter(|p| is_aggregate_call(&p.expr).is_none())
        .map(|p| resolve_and_render(&p.expr, table_alias))
        .collect::<DvisorResult<Vec<_>>>()?;

    let rendered: Vec<String> = group_exprs.iter().map(|e| resolve_and_render(e, table_alias)).collect::<DvisorResult<Vec<_>>>()?;

    if rendered.len() != plain_exprs.len() || !rendered.iter().all(|g| plain_exprs.contains(g)) {
        throw!(
            "GROUP BY ({}) does not match the non-aggregated projection columns ({})",
            rendered.join(", "),
            plain_exprs.join(", ")
        );
    }
    Ok(())
}

/// The output name a single projection item is known by, if one can be
/// determined without actually running the query: its explicit alias, or —
/// for a bare `agg(column)` with no alias — the same default alias dvisor
/// itself would derive. `None` means the item needs an explicit alias to be
/// referenced at all (e.g. `sum(a + b)`, whose synthetic internal name
/// isn't predictable here).
fn projection_item_name(item: &ProjItem, table_alias: Option<&str>) -> DvisorResult<Option<String>> {
    if let Some(a) = &item.alias {
        return Ok(Some(a.clone()));
    }
    if let Some(agg_name) = is_aggregate_call(&item.expr) {
        if let Expr::Call(_, args) = &item.expr {
            if args.len() == 1 && matches!(args[0], Expr::Ident(_) | Expr::Qualified(_, _)) {
                let inner = resolve_and_render(&args[0], table_alias)?;
                return Ok(Some(default_aggregate_alias(agg_name, &inner)));
            }
        }
        return Ok(None);
    }
    Ok(Some(resolve_and_render(&item.expr, table_alias)?))
}

fn known_output_names(items: &[ProjItem], table_alias: Option<&str>) -> DvisorResult<Vec<String>> {
    let mut names = Vec::with_capacity(items.len());
    for item in items {
        if let Some(n) = projection_item_name(item, table_alias)? {
            names.push(n);
        }
    }
    Ok(names)
}

fn compile_order_by(
    order_by: &Option<Vec<(Expr, OrderDir)>>,
    projection: &[ProjItem],
    table_alias: Option<&str>,
) -> DvisorResult<Vec<SortItem>> {
    let Some(items) = order_by else { return Ok(vec![]) };
    let known = known_output_names(projection, table_alias)?;

    let mut out = Vec::with_capacity(items.len());
    for (expr, dir) in items {
        let name = match expr {
            Expr::Number(n) => {
                let idx = ordinal_index(*n, projection.len(), "ORDER BY")?;
                projection_item_name(&projection[idx], table_alias)?.ok_or_else(|| {
                    format!(
                        "ORDER BY {} refers to a SELECT item with no determinable name; give it an explicit alias",
                        format_number(*n)
                    )
                })?
            }
            Expr::Ident(n) => n.clone(),
            Expr::Qualified(alias, col) if table_alias == Some(alias.as_str()) => col.clone(),
            _ => throw!("ORDER BY only supports a column/alias already present in SELECT, got a general expression"),
        };
        if !known.iter().any(|k| k.eq_ignore_ascii_case(&name)) {
            throw!("ORDER BY references '{}', which is not in the SELECT projection", name);
        }
        let direction = Some(match dir {
            OrderDir::Asc => SortOrder::ASC,
            OrderDir::Desc => SortOrder::DESC,
        });
        out.push(SortItem { expr: name, direction });
    }
    Ok(out)
}

pub fn compile(stmt: SelectStmt) -> DvisorResult<Query> {
    let table_alias = stmt.table.alias.as_deref();

    let project = compile_projection(&stmt.projection, table_alias)?;
    validate_group_by(&stmt.group_by, &stmt.projection, table_alias)?;
    let sort = compile_order_by(&stmt.order_by, &stmt.projection, table_alias)?;

    let (filter, calc) = match &stmt.where_clause {
        Some(w) => compile_where(w, table_alias)?,
        None => (Vec::new(), Vec::new()),
    };

    Ok(Query { from: stmt.table.name, calc, project, filter, sort })
}
