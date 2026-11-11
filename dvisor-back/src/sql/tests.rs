use super::compile_sql;
use serde_json::Value as Json;

fn compile(sql: &str) -> Json {
    let json = compile_sql(sql).unwrap_or_else(|e| panic!("compile_sql failed for `{sql}`: {e}"));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("invalid JSON produced for `{sql}`: {e}\n{json}"))
}

fn compile_err(sql: &str) -> String {
    match compile_sql(sql) {
        Ok(json) => panic!("expected `{sql}` to fail to compile, got: {json}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn simple_select_with_where_and_plain_projection() {
    let q = compile("SELECT l_orderkey, l_quantity FROM 'https://example.com/data/lineitem.parquet' WHERE l_quantity > 10");

    assert_eq!(q["from"], "https://example.com/data/lineitem.parquet");
    assert_eq!(q["project"][0]["expr"], "l_orderkey");
    assert_eq!(q["project"][1]["expr"], "l_quantity");
    assert_eq!(q["filter"][0]["expr"], "l_quantity");
    assert_eq!(q["filter"][0]["oper"], "GT");
    assert_eq!(q["filter"][0]["value"], 10);
}

#[test]
fn table_name_accepts_double_quoted_url_like_original_js_parser() {
    let q = compile(r#"SELECT x FROM "https://example.com/d.parquet""#);
    assert_eq!(q["from"], "https://example.com/d.parquet");
}

#[test]
fn tpch_style_aggregate_query_matches_the_json_query_dsl() {
    let sql = "
        SELECT
            dateTruncMonth(l_shipdate) AS month,
            sum(l_extendedprice*(1-l_discount)) AS revenue,
            sum(l_quantity) AS total_qty,
            count() AS rows_scanned
        FROM 'https://example.com/data/lineitem-sf1.parquet'
        WHERE l_shipdate >= '1993-01-01' AND l_shipdate <= '1994-12-31'
        ORDER BY month ASC
    ";
    let q = compile(sql);

    assert_eq!(q["from"], "https://example.com/data/lineitem-sf1.parquet");

    assert_eq!(q["project"][0]["expr"], "dateTruncMonth(l_shipdate)");
    assert_eq!(q["project"][0]["alias"], "month");
    assert_eq!(q["project"][1]["expr"], "l_extendedprice*(1-l_discount)");
    assert_eq!(q["project"][1]["aggregate"], "sum");
    assert_eq!(q["project"][1]["alias"], "revenue");
    assert_eq!(q["project"][2]["expr"], "l_quantity");
    assert_eq!(q["project"][2]["aggregate"], "sum");
    assert_eq!(q["project"][3]["expr"], Json::Null);
    assert_eq!(q["project"][3]["aggregate"], "count");
    assert_eq!(q["project"][3]["alias"], "rows_scanned");

    assert_eq!(q["filter"][0]["expr"], "l_shipdate");
    assert_eq!(q["filter"][0]["oper"], "GE");
    assert_eq!(q["filter"][0]["value"], "1993-01-01");
    assert_eq!(q["filter"][1]["expr"], "l_shipdate");
    assert_eq!(q["filter"][1]["oper"], "LE");
    assert_eq!(q["filter"][1]["value"], "1994-12-31");

    assert_eq!(q["sort"][0]["expr"], "month");
    assert_eq!(q["sort"][0]["direction"], "ASC");
}

#[test]
fn aggregate_over_a_compound_expression_sets_the_aggregate_field_directly() {
    let q = compile("SELECT first(length(name)) AS lx FROM 'https://example.com/d.parquet'");
    assert_eq!(q["project"][0]["expr"], "length(name)");
    assert_eq!(q["project"][0]["aggregate"], "first");
    assert_eq!(q["project"][0]["alias"], "lx");
}

#[test]
fn aggregate_nested_inside_another_function_is_rejected() {
    let err = compile_err("SELECT round(avg(price), 2) AS avg_price FROM 'https://example.com/d.parquet'");
    assert!(err.contains("outermost"), "unexpected error: {err}");
}

#[test]
fn mod_keyword_compiles_to_the_percent_operator() {
    let q = compile("SELECT (a MOD 3) AS r FROM 'https://example.com/d.parquet'");
    assert_eq!(q["project"][0]["expr"], "a%3");
}

#[test]
fn mod_function_call_is_not_swallowed_by_the_mod_keyword() {
    let q = compile("SELECT mod(a, 3) AS r FROM 'https://example.com/d.parquet'");
    assert_eq!(q["project"][0]["expr"], "mod(a,3)");
}

#[test]
fn qualified_columns_are_stripped_of_their_table_alias() {
    let q = compile("SELECT s.peso AS peso FROM 'https://example.com/d.parquet' AS s WHERE s.codmunres = 355030");
    assert_eq!(q["project"][0]["expr"], "peso");
    assert_eq!(q["filter"][0]["expr"], "codmunres");
    assert_eq!(q["filter"][0]["value"], 355030);
}

#[test]
fn quoted_identifiers_with_spaces_are_supported() {
    let q = compile(r#"SELECT s.peso AS "Peso ao Nascer" FROM 'https://example.com/d.parquet' AS s"#);
    assert_eq!(q["project"][0]["alias"], "Peso ao Nascer");
}

#[test]
fn where_clause_on_a_compound_expression_is_hoisted_into_a_calc_column() {
    let q = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE (a + b) > 10");
    assert_eq!(q["filter"][0]["oper"], "GT");
    let calc_alias = q["filter"][0]["expr"].as_str().unwrap().to_string();
    assert_eq!(q["calc"][0]["alias"], calc_alias);
    assert_eq!(q["calc"][0]["expr"], "(a,b)=>a+b");
}

#[test]
fn group_by_must_match_the_non_aggregated_projection_columns() {
    let err = compile_err(
        "SELECT month, sum(revenue) AS total FROM 'https://example.com/d.parquet' GROUP BY wrong_column",
    );
    assert!(err.contains("GROUP BY"), "unexpected error: {err}");
}

#[test]
fn order_by_must_reference_a_projected_column() {
    let err = compile_err("SELECT a FROM 'https://example.com/d.parquet' ORDER BY not_selected");
    assert!(err.contains("ORDER BY"), "unexpected error: {err}");
}

#[test]
fn or_in_where_is_rejected() {
    let err = compile_err("SELECT a FROM 'https://example.com/d.parquet' WHERE a = 1 OR a = 2");
    assert!(err.contains("Unsupported WHERE"), "unexpected error: {err}");
}

#[test]
fn comparing_two_non_constant_expressions_is_rejected() {
    let err = compile_err("SELECT a FROM 'https://example.com/d.parquet' WHERE a > b");
    assert!(err.contains("constant"), "unexpected error: {err}");
}

#[test]
fn unknown_table_alias_in_qualified_column_is_rejected() {
    let err = compile_err("SELECT x.a FROM 'https://example.com/d.parquet' AS s");
    assert!(err.contains("alias"), "unexpected error: {err}");
}

#[test]
fn syntax_errors_are_reported() {
    let err = compile_err("SELECT FROM 'https://example.com/d.parquet'");
    assert!(!err.is_empty());
}

// ---- pruning-oriented WHERE rewrites (year()/bucket() -> range on the
// underlying physical column, for the row-group viability guard) ----

use super::ast::{BinOp, Expr};
use super::compiler::rewrite_for_pruning;

fn ident(name: &str) -> Expr {
    Expr::Ident(name.to_string())
}

fn num(n: f64) -> Expr {
    Expr::Number(n)
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call(name.to_string(), args)
}

#[test]
fn year_equality_rewrites_to_a_ge_le_range_with_no_calc_column() {
    let q = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE year(DTNASC) = 2013");
    assert_eq!(q["calc"].as_array().map(|a| a.len()).unwrap_or(0), 0, "expected no calc[] entry: {q}");
    assert_eq!(q["filter"][0]["expr"], "DTNASC");
    assert_eq!(q["filter"][0]["oper"], "GE");
    assert_eq!(q["filter"][0]["value"], "2013-01-01");
    assert_eq!(q["filter"][1]["expr"], "DTNASC");
    assert_eq!(q["filter"][1]["oper"], "LE");
    assert_eq!(q["filter"][1]["value"], "2013-12-31");
}

#[test]
fn year_datetruncyear_alias_also_rewrites() {
    let q = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE dateTruncYear(DTNASC) = 2013");
    assert_eq!(q["filter"][0]["oper"], "GE");
    assert_eq!(q["filter"][1]["oper"], "LE");
}

#[test]
fn year_inequalities_rewrite_to_a_single_bound() {
    let ge = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE year(DTNASC) >= 2013");
    assert_eq!(ge["filter"].as_array().unwrap().len(), 1);
    assert_eq!(ge["filter"][0]["oper"], "GE");
    assert_eq!(ge["filter"][0]["value"], "2013-01-01");

    let lt = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE year(DTNASC) < 2013");
    assert_eq!(lt["filter"][0]["oper"], "LT");
    assert_eq!(lt["filter"][0]["value"], "2013-01-01");

    let le = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE year(DTNASC) <= 2013");
    assert_eq!(le["filter"][0]["oper"], "LE");
    assert_eq!(le["filter"][0]["value"], "2013-12-31");

    let gt = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE year(DTNASC) > 2013");
    assert_eq!(gt["filter"][0]["oper"], "GT");
    assert_eq!(gt["filter"][0]["value"], "2013-12-31");
}

#[test]
fn year_not_equal_is_not_rewritten() {
    // NE has no single-range equivalent — must decline, so the caller falls
    // through to the ordinary calc[]-hoisting path (unit-tested directly
    // here rather than through the full compile_sql pipeline, since
    // rendering that fallback's calc body needs to resolve "year" via
    // getApiFunctionName, a wasm-bindgen import unavailable outside a real
    // JS/wasm runtime — see the doc_check panics earlier this session).
    let expr = call("year", vec![ident("DTNASC")]);
    let result = rewrite_for_pruning(&expr, BinOp::Ne, &num(2013.0), None).unwrap();
    assert!(result.is_none());
}

#[test]
fn year_literal_with_a_qualified_column_also_rewrites() {
    let q = compile("SELECT s.a FROM 'https://example.com/d.parquet' AS s WHERE year(s.DTNASC) = 2013");
    assert_eq!(q["filter"][0]["expr"], "DTNASC");
    assert_eq!(q["filter"][1]["expr"], "DTNASC");
}

#[test]
fn bucket_equality_rewrites_to_a_ge_lt_range_with_no_calc_column() {
    let q = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE bucket(PESO, 500) = 3000");
    assert_eq!(q["calc"].as_array().map(|a| a.len()).unwrap_or(0), 0, "expected no calc[] entry: {q}");
    assert_eq!(q["filter"][0]["expr"], "PESO");
    assert_eq!(q["filter"][0]["oper"], "GE");
    assert_eq!(q["filter"][0]["value"], 3000);
    assert_eq!(q["filter"][1]["expr"], "PESO");
    assert_eq!(q["filter"][1]["oper"], "LT");
    assert_eq!(q["filter"][1]["value"], 3500);
}

#[test]
fn bucket_inequalities_rewrite_to_a_single_bound() {
    let ge = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE bucket(PESO, 500) >= 3000");
    assert_eq!(ge["filter"][0]["oper"], "GE");
    assert_eq!(ge["filter"][0]["value"], 3000);

    let lt = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE bucket(PESO, 500) < 3000");
    assert_eq!(lt["filter"][0]["oper"], "LT");
    assert_eq!(lt["filter"][0]["value"], 3000);

    let le = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE bucket(PESO, 500) <= 3000");
    assert_eq!(le["filter"][0]["oper"], "LT");
    assert_eq!(le["filter"][0]["value"], 3500);

    let gt = compile("SELECT a FROM 'https://example.com/d.parquet' WHERE bucket(PESO, 500) > 3000");
    assert_eq!(gt["filter"][0]["oper"], "GE");
    assert_eq!(gt["filter"][0]["value"], 3500);
}

#[test]
fn bucket_with_a_non_multiple_literal_is_not_rewritten() {
    // 3200 isn't a multiple of the 500-wide bucket, i.e. not actually
    // reachable as a bucket() output — declining (rather than guessing a
    // range) leaves it to the calc[]-hoisting path, which correctly
    // evaluates it as real JS (and will simply match nothing). See the note
    // on year_not_equal_is_not_rewritten for why this is unit-tested at the
    // rewrite_for_pruning level instead of through full compile_sql.
    let expr = call("bucket", vec![ident("PESO"), num(500.0)]);
    let result = rewrite_for_pruning(&expr, BinOp::Eq, &num(3200.0), None).unwrap();
    assert!(result.is_none());
}

#[test]
fn bucket_not_equal_is_not_rewritten() {
    let expr = call("bucket", vec![ident("PESO"), num(500.0)]);
    let result = rewrite_for_pruning(&expr, BinOp::Ne, &num(3000.0), None).unwrap();
    assert!(result.is_none());
}

#[test]
fn year_and_bucket_rewrites_compose_with_a_plain_predicate() {
    let q = compile(
        "SELECT a FROM 'https://example.com/d.parquet' WHERE year(DTNASC) = 2013 AND bucket(PESO, 500) = 3000 AND SEXO = 1",
    );
    assert_eq!(q["calc"].as_array().map(|a| a.len()).unwrap_or(0), 0, "expected no calc[] entries: {q}");
    assert_eq!(q["filter"].as_array().unwrap().len(), 5);
    assert_eq!(q["filter"][4]["expr"], "SEXO");
    assert_eq!(q["filter"][4]["oper"], "EQ");
}

// `iif` needs no compiler support of its own: it is a plain function call
// like any other, so it already goes through the same "render as bare text,
// resolved against api.js at runtime by dvisor's own calc-column machinery"
// path used for `sum(l_extendedprice*(1-l_discount))` above. The interesting
// part is only that a comparison (`PARTO = 2`) renders correctly as a JS
// boolean expression when it's an aggregate's argument, not a WHERE/calc[]
// predicate.
#[test]
fn iif_inside_an_aggregate_renders_as_a_plain_ternary_call() {
    let q = compile("SELECT UF, SUM(iif(PARTO = 2, 1, 0)) AS cesareas, count() AS total FROM sinasc WHERE PARTO < 9 GROUP BY UF");
    assert_eq!(q["project"][0]["expr"], "UF");
    assert_eq!(q["project"][1]["expr"], "iif(PARTO==2,1,0)");
    assert_eq!(q["project"][1]["aggregate"], "SUM");
    assert_eq!(q["project"][1]["alias"], "cesareas");
    assert_eq!(q["project"][2]["aggregate"], "count");
}

// ---- COUNT(*) sugar ----

#[test]
fn count_star_compiles_identically_to_count() {
    // Same spelling/case on both sides — only the `(*)` vs `()` differs —
    // since the aggregate name field preserves the casing the user typed,
    // by existing (unrelated) design.
    let with_star = compile("SELECT UF, count(*) AS n FROM sinasc GROUP BY UF");
    let without_star = compile("SELECT UF, count() AS n FROM sinasc GROUP BY UF");
    assert_eq!(with_star, without_star);
    assert_eq!(with_star["project"][1]["aggregate"], "count");
    assert_eq!(with_star["project"][1]["expr"], Json::Null);
}

#[test]
fn count_star_is_case_insensitive_and_only_sugar_for_count() {
    // Lowercase/mixed-case spelling of the function name.
    let q = compile("SELECT count(*) AS n FROM sinasc");
    assert_eq!(q["project"][0]["aggregate"], "count");
    // `*` is not a general wildcard argument — it isn't accepted by any
    // other function.
    let err = compile_err("SELECT sum(*) AS n FROM sinasc");
    assert!(err.contains("Expression expected"), "unexpected error: {err}");
}

// ---- GROUP BY / ORDER BY ordinal positions ----

#[test]
fn group_by_ordinal_is_equivalent_to_spelling_out_the_expression() {
    // Position 1 is a calculated expression (bucket(...)) — the whole point
    // of supporting ordinals is not having to repeat that in GROUP BY.
    let by_ordinal = compile("SELECT bucket(PESO,500) AS faixa, count() AS n FROM sinasc GROUP BY 1");
    let by_expr = compile("SELECT bucket(PESO,500) AS faixa, count() AS n FROM sinasc GROUP BY bucket(PESO,500)");
    assert_eq!(by_ordinal, by_expr);
}

#[test]
fn group_by_ordinal_referencing_multiple_positions() {
    let q = compile("SELECT SEXO, PARTO, count() AS n FROM sinasc GROUP BY 1, 2");
    assert_eq!(q["project"][0]["expr"], "SEXO");
    assert_eq!(q["project"][1]["expr"], "PARTO");
}

#[test]
fn group_by_ordinal_out_of_range_is_a_clear_error() {
    let err = compile_err("SELECT SEXO, count() AS n FROM sinasc GROUP BY 3");
    assert!(err.contains("GROUP BY position 3 is not in the SELECT list"), "unexpected error: {err}");
}

#[test]
fn group_by_ordinal_pointing_at_an_aggregate_is_still_rejected() {
    // Position 2 is count() itself — grouping by an aggregate makes no
    // sense, and must fail the same way spelling it out would.
    let err = compile_err("SELECT SEXO, count() AS n FROM sinasc GROUP BY 2");
    assert!(err.contains("does not match"), "unexpected error: {err}");
}

#[test]
fn order_by_ordinal_referencing_an_aliased_column() {
    let by_ordinal = compile("SELECT SEXO, count() AS n FROM sinasc GROUP BY SEXO ORDER BY 2 DESC");
    let by_alias = compile("SELECT SEXO, count() AS n FROM sinasc GROUP BY SEXO ORDER BY n DESC");
    assert_eq!(by_ordinal, by_alias);
}

#[test]
fn order_by_ordinal_referencing_a_default_aggregate_alias() {
    // No explicit alias on sum(PESO) — the ordinal must resolve to the same
    // default name dvisor itself derives ("sum_PESO").
    let by_ordinal = compile("SELECT SEXO, sum(PESO) FROM sinasc GROUP BY SEXO ORDER BY 2");
    assert_eq!(by_ordinal["sort"][0]["expr"], "sum_PESO");
}

#[test]
fn order_by_ordinal_zero_or_out_of_range_is_a_clear_error() {
    let zero = compile_err("SELECT SEXO, count() AS n FROM sinasc ORDER BY 0");
    assert!(zero.contains("ORDER BY position 0 is not in the SELECT list"), "unexpected error: {zero}");
    let too_big = compile_err("SELECT SEXO, count() AS n FROM sinasc ORDER BY 5");
    assert!(too_big.contains("ORDER BY position 5 is not in the SELECT list"), "unexpected error: {too_big}");
}

#[test]
fn order_by_ordinal_with_no_determinable_name_is_a_clear_error() {
    // sum(a + b) has no alias and no predictable default name.
    let err = compile_err("SELECT SEXO, sum(PESO + APGAR5) FROM sinasc GROUP BY SEXO ORDER BY 2");
    assert!(err.contains("no determinable name"), "unexpected error: {err}");
}
