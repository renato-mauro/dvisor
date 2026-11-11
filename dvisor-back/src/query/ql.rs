use std::collections::HashSet;
use regex::Regex;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{prelude::wasm_bindgen};
use crate::{error::{DvisorError, DvisorResult}, model::{enums::{AggFunction, CalcType, Operator, SortOrder}, tiledata::{FloatOperator, IntOperator, StringOperator}}, query::plan::{PipelineOperatorCalculateColumn, PipelineOperatorGroupBy, PipelineOperatorPack, PipelineOperatorProject, PipelineOperatorSelectFloat, PipelineOperatorSelectInt, PipelineOperatorSelectString, PipelineOperatorSort, QueryPipeline}, throw, utils::parens_balanced};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = globalThis, js_name = getApiFunctionName, catch)]
    pub(crate) fn get_api_function_name(name: &str) -> Result<String, DvisorError>;
}

// Every struct of the query document rejects fields it doesn't know, so a
// misspelled field (`filters`) or a removed one (`rename`) is an error rather
// than silently ignored.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub from: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calc: Vec<CalculatedExpression>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project: Vec<ProjectItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter: Vec<Predicate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<SortItem>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalculatedExpression {
    pub expr: String,
    pub alias: String,
    #[serde(rename = "type")]
    pub tp: Option<CalcType>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectItem {
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Predicate {
    pub expr: String,
    pub oper: Operator,
    pub value: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SortItem {
    pub expr: String,
    pub direction: Option<SortOrder>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Int(i32),
    Float(f32),
    String(String),
    List(Vec<i32>),
}

impl Query {

    pub fn new(query: &str) -> DvisorResult<Query> {
        let mut q: Query = serde_json::from_str(query)?;
        q.extract_functions()?;
        Ok(q)
    }

    fn add_select_int(w: &Predicate, i: i32, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        let int_oper = IntOperator::new(w.oper,i)?;
        pipeline.push_operator(PipelineOperatorSelectInt::new(w.expr.clone(),int_oper));
        Ok(())
    }

    fn add_select_float(w: &Predicate, f: f32, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        let float_oper = FloatOperator::new(w.oper,f)?;
        pipeline.push_operator(PipelineOperatorSelectFloat::new(w.expr.clone(),float_oper));
        Ok(())
    }

    fn add_select_string(w: &Predicate, s: String, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        let str_oper = StringOperator::new(w.oper,s)?;
        pipeline.push_operator(PipelineOperatorSelectString::new(w.expr.clone(),str_oper));
        Ok(())
    }

    fn add_select_int_list(_w: &Predicate, _items: &Vec<i32>, _pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        todo!("Int List Selection not yet implemented");
    }

    fn compute_before_filter_expression(&self, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        for calc in &self.calc {
            let a = &calc.alias;
            if self.filter.iter().any(|p|p.expr.eq_ignore_ascii_case(a)) {
                let c = CalculatedExpression { alias: calc.alias.clone(), expr: calc.expr.clone(), tp: calc.tp };
                pipeline.push_operator(PipelineOperatorCalculateColumn::new(c));
            }
        }        
        Ok(())
    }

    fn compute_after_filter_expression(&self, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        for calc in &self.calc {
            let a = &calc.alias;
            if !self.filter.iter().any(|p|p.expr.eq_ignore_ascii_case(a)) {
                let c = CalculatedExpression { alias: calc.alias.clone(), expr: calc.expr.clone(), tp: calc.tp };
                pipeline.push_operator(PipelineOperatorCalculateColumn::new(c));
            }
        }       
        Ok(())
    }

    fn compute_filter(&self, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        for w in &self.filter {
            match &w.value {
                Value::Int(i) => Query::add_select_int(w,*i,pipeline)?, 
                Value::Float(f) => Query::add_select_float(w,*f,pipeline)?,
                Value::String(s) => Query::add_select_string(w,s.clone(),pipeline)?,
                Value::List(items) => Query::add_select_int_list(w,items,pipeline)?,
            }
        }
        Ok(())
    }

    // Groups the rows by every projection item without an aggregate. Each
    // fragment's groups come out sorted by those columns, and the client
    // merges them into the result so far by interleaving sorted lists, with
    // a comparator generated from the first fragment it receives (see
    // DvisorTableFragment::js_helpers). So the order of the group columns
    // must be the same in every worker and every fragment: it is derived from
    // the query alone, by group_key_order. Returns whether that order already
    // satisfies the query's sort.
    fn compute_group(&self, pipeline: &mut QueryPipeline) -> DvisorResult<bool> {
        let mut keys = Vec::new();
        let mut agg_fns = Vec::new();
        let mut alias = Vec::new();
        for (i, c) in self.project.iter().enumerate() {
            let cn = if let Some(s) = &c.expr {
                s.trim().to_ascii_lowercase()
            } else {
                "".to_string()
            };
            let mut default_alias = cn.clone();
            if let Some(agg_name) = &c.aggregate {
                if let Some(aggf) = AggFunction::by_name(agg_name.as_str()) {
                    agg_fns.push((cn,aggf));
                    default_alias = aggf.default_field_name(&default_alias);
                } else {
                    throw!("Invalid aggregation function {} for expression {}.",agg_name,cn);
                }
            } else {
                keys.push(i);
                agg_fns.push((cn,AggFunction::FIRST));
            }
            if let Some(a) = &c.alias {
                alias.push(a.trim().to_ascii_lowercase());
            } else {
                alias.push(default_alias)
            }
        }
        let (order, sort_satisfied) = group_key_order(&keys, &alias, &self.sort)?;
        // Each group column is sorted and grouped by the name it is read by
        // (a column or a calc[] alias), and compared in the merge by the name
        // of the output column it becomes.
        let group = order.iter().map(|&(i, so)| (agg_fns[i].0.clone(), so)).collect();
        let group_output = order.iter().map(|&(i, _)| alias[i].clone()).collect();
        pipeline.push_operator(PipelineOperatorGroupBy::new(group, group_output, agg_fns, alias));
        Ok(sort_satisfied)
    }

    fn compute_project(&self, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        if self.project.len() > 0 {
            let mut col_names = Vec::with_capacity(self.project.len());
            for c in &self.project {
                if let Some(s) = &c.expr {
                    let cexp = s.trim().to_ascii_lowercase();
                    if let Some(a) = &c.alias {
                        col_names.push((cexp,a.trim().to_string()));
                    } else {
                        col_names.push((cexp.clone(),cexp));
                    }
                }
            }
            pipeline.push_operator(PipelineOperatorProject::new(col_names));
        }
        Ok(())
    }

    fn compute_sort(&self, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        if self.sort.len() > 0 {
            let mut column_order = Vec::new();
            for si in &self.sort {
                column_order.push((si.expr.to_string(),si.direction.unwrap_or_default()));
            }
            pipeline.push_operator(PipelineOperatorSort::new(column_order));
        }
        Ok(())
    }

    // The columns to read from the file, in a fixed order: filter columns,
    // then projection columns, then calc[] parameters, without repeats and
    // without the calc[] aliases, which are computed rather than read. The
    // fixed order makes a fragment's reads, and so its HTTP range requests,
    // the same on every run.
    fn get_column_names(&self) -> Vec<String> {
        fn add(names: &mut Vec<String>, name: &str) {
            if !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
        }
        let mut names = Vec::new();
        for p in &self.filter {
            add(&mut names, &p.expr);
        }
        for g in &self.project {
            if let Some(cn) = &g.expr {
                if cn.len() > 0 {
                    add(&mut names, cn);
                }
            }
        }
        for ce in &self.calc {
            for n in &ce.get_param_names() {
                add(&mut names, n);
            }
            names.retain(|n| n != &ce.alias);
        }
        names
    }

    fn is_group_by(&self) -> bool {
        for g in &self.project {
            if g.aggregate.is_some() {
                return true;
            }
        }
        false
    }

    fn extract_functions(&mut self) -> DvisorResult<()> {
        let mut exp_count = 0;
        for p in &mut self.project {
            if p.expr.is_some() {
                let mut expr = p.expr.as_ref().unwrap().clone();
                if let Some((fname,args)) = extract_aggregate_function(&expr)? {
                    p.aggregate = Some(fname);
                    if args.is_none() {
                        p.expr = None;
                        continue;
                    }
                    expr = args.unwrap().clone();
                    p.expr = Some(expr.clone());
                }
                let (funcs, ids) = extract_funcs_and_ids(&expr);
                for fname in funcs.iter() {
                    if is_aggregate(fname) {
                        throw!("Invalid use of aggregate function in expression '{}'",expr);
                    }
                }
                if !(ids.len() == 1 && ids[0] == expr.as_str().trim()) {
                    let mut f = String::new();
                    let mut sep = '(';
                    for id in ids {
                        f.push(sep);
                        f.push_str(&id);
                        sep = ',';
                    }
                    f.push_str(")=>{");
                    for fc in funcs {
                        let fc_api_name = get_api_function_name(&fc)?;
                        f.push_str(&format!("const {fc}=api.{fc_api_name};"));
                    }
                    f.push_str(&format!("return ({expr})}}"));
                    let alias = if let Some(a) = &p.alias {
                        a.clone()
                    } else {
                        throw!("Alias required in expressions in projection: {}",expr);
                    };
                    let temp_alias = if p.aggregate.is_none() {
                        p.alias = None;
                        alias.clone()
                    } else {
                        exp_count += 1;
                        format!("_calc{}",exp_count)
                    };
                    self.calc.push(CalculatedExpression { expr: f, alias: temp_alias.clone(), tp: None });
                    p.expr = Some(temp_alias);
                }
            }
        }
        Ok(())
    }

    pub fn generate_plan(&self) -> DvisorResult<QueryPipeline> {
        // `project.is_empty()` is the one case that actually means "select
        // every column" (no projection specified at all). Once there IS a
        // projection, the required columns are whatever get_column_names
        // finds — which can legitimately be empty (e.g. `count()` alone
        // needs none) without that meaning "all of them".
        let columns: Option<Vec<String>> = if self.project.len() > 0 {
            Some(self.get_column_names())
        } else {
            None
        };
        let mut pipeline = QueryPipeline::new(columns);
        self.compute_before_filter_expression(&mut pipeline)?;
        self.compute_filter(&mut pipeline)?;
        self.compute_after_filter_expression(&mut pipeline)?;
        if self.is_group_by() {
            let sort_satisfied = self.compute_group(&mut pipeline)?;
            if !sort_satisfied {
                self.compute_sort(&mut pipeline)?;
            }
        } else {
            self.compute_project(&mut pipeline)?;
            self.compute_sort(&mut pipeline)?;
            self.compute_pack(&mut pipeline)?;
        }
        Ok(pipeline)
    }
    
    fn compute_pack(&self, pipeline: &mut QueryPipeline) -> DvisorResult<()> {
        pipeline.push_operator(PipelineOperatorPack::new());
        Ok(())
    }

}

/// Orders the group columns of an aggregate query for sorting and merging.
/// `keys` are their positions in the projection, `names` the output names of
/// every projection item. The order is: the group columns `sort` starts
/// with, in its order and directions, then the remaining ones in projection
/// order, ascending. It depends on the query alone, never on the state of the
/// worker that computes it.
///
/// Returns the order, as (position, direction), and whether it satisfies
/// `sort` by itself, which is the case when `sort` names only group columns;
/// otherwise the query needs a sort of its own after grouping. A sort item
/// that names no output column is an error.
pub(crate) fn group_key_order(keys: &[usize], names: &[String], sort: &[SortItem]) -> DvisorResult<(Vec<(usize, SortOrder)>, bool)> {
    let mut order: Vec<(usize, SortOrder)> = Vec::with_capacity(keys.len());
    let mut satisfied = true;
    for si in sort {
        let Some(pos) = names.iter().position(|n| n.eq_ignore_ascii_case(si.expr.trim())) else {
            throw!("Cannot sort by '{}': the query has no output column of that name (it has {})", si.expr, names.join(", "));
        };
        if !satisfied || order.iter().any(|(p, _)| *p == pos) {
            continue;
        }
        if keys.contains(&pos) {
            order.push((pos, si.direction.unwrap_or_default()));
        } else {
            satisfied = false;
        }
    }
    for &k in keys {
        if !order.iter().any(|(p, _)| *p == k) {
            order.push((k, SortOrder::ASC));
        }
    }
    Ok((order, satisfied))
}

impl CalculatedExpression {
    pub fn get_param_names(&self) -> Vec<String> {
        let regex = Regex::new(r"(?m)(?:^\((?P<arguments_n>[^)]+)\)\s*=>.*$)|(?:(?P<arguments_1>[^=]+)\s*=>.*$)").unwrap();
        let mut param_names = Vec::new();
        if let Some(caps) = regex.captures(&self.expr) {
            if let Some(arg1) = caps.name("arguments_1").map(|m|m.as_str()) {
                param_names.push(arg1.to_string());
            } else if let Some(argn) = caps.name("arguments_n").map(|m|m.as_str()) {
                argn
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .for_each(|s|{
                    param_names.push(s.to_string());
                });
            }
        }
        param_names
    }
}

#[inline] fn is_aggregate(function_name: &str) -> bool {
    let aggs = HashSet::from(["sum", "count", "min", "max", "avg"]);
    aggs.contains(&function_name.to_ascii_lowercase().as_str())
}

fn extract_aggregate_function(expr: &str) -> DvisorResult<Option<(String,Option<String>)>> {
    let regex = Regex::new(r"(?m)^\s*([A-Za-z_]\w*)\s*\(\s*(.*)\)$").unwrap();
    let result= regex.captures_iter(expr);
    for caps in result {
        let function_name = &caps[1];
        let function_arguments = &caps[2];
        if is_aggregate(function_name) {
            if function_arguments.len() == 0 {
                    return Ok(Some((function_name.to_string(),None)))
            } else {
                if parens_balanced(function_arguments) {
                    return Ok(Some((function_name.to_string(),Some(function_arguments.to_string()))))
                } else {
                    throw!("Not allowed expressions with aggregate functions: {}",expr);
                }
            }
        } else {
            return Ok(None);
        }
    }
    Ok(None)
}

pub fn extract_funcs_and_ids(expr: &str) -> (Vec<String>, Vec<String>) {
    let str_re = Regex::new(r#"'(?:''|[^'])*'|"(?:\\.|[^"\\])*""#).unwrap();
    let masked = str_re.replace_all(expr, " ").into_owned();
    let func_re = Regex::new(r"\b([A-Za-z_]\w*)\s*\(").unwrap();
    let mut funcs: Vec<String> = func_re
        .captures_iter(&masked)
        .map(|c| c[1].to_string())
        .collect();
    let id_re = Regex::new(r"\b([A-Za-z_]\w*)\b").unwrap();
    let mut ids = Vec::new();
    for cap in id_re.captures_iter(&masked) {
        let m = cap.get(0).unwrap();
        let name = cap[1].to_string();
        let mut it = masked[m.end()..].chars().skip_while(|c| c.is_whitespace());
        if it.next() == Some('(') {
            continue;
        }
        ids.push(name);
    }
    dedup_keep_order(&mut funcs);
    dedup_keep_order(&mut ids);
    (funcs, ids)
}

fn dedup_keep_order(v: &mut Vec<String>) {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    v.retain(|s| seen.insert(s.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use SortOrder::{ASC, DESC};

    // A projection of two group columns, a and b, and one aggregate, n.
    const KEYS: [usize; 2] = [0, 1];

    fn names() -> Vec<String> {
        ["a", "b", "n"].map(String::from).to_vec()
    }

    fn sort(items: &[(&str, SortOrder)]) -> Vec<SortItem> {
        items.iter().map(|(expr, d)| SortItem { expr: expr.to_string(), direction: Some(*d) }).collect()
    }

    #[test]
    fn without_a_sort_the_group_columns_keep_projection_order() {
        let (order, satisfied) = group_key_order(&KEYS, &names(), &[]).unwrap();
        assert_eq!(order, vec![(0, ASC), (1, ASC)]);
        assert!(satisfied);
    }

    #[test]
    fn a_sort_on_group_columns_sets_their_order_and_directions() {
        let (order, satisfied) = group_key_order(&KEYS, &names(), &sort(&[("b", DESC), ("a", ASC)])).unwrap();
        assert_eq!(order, vec![(1, DESC), (0, ASC)]);
        assert!(satisfied);
    }

    #[test]
    fn group_columns_a_partial_sort_leaves_out_still_group() {
        let (order, satisfied) = group_key_order(&KEYS, &names(), &sort(&[("b", ASC)])).unwrap();
        assert_eq!(order, vec![(1, ASC), (0, ASC)]);
        assert!(satisfied);
    }

    #[test]
    fn a_sort_starting_with_an_aggregate_needs_a_sort_step_of_its_own() {
        let (order, satisfied) = group_key_order(&KEYS, &names(), &sort(&[("n", DESC), ("b", ASC)])).unwrap();
        assert_eq!(order, vec![(0, ASC), (1, ASC)]);
        assert!(!satisfied);
    }

    #[test]
    fn sort_names_match_output_names_ignoring_case() {
        let (order, _) = group_key_order(&KEYS, &names(), &sort(&[("B", ASC)])).unwrap();
        assert_eq!(order, vec![(1, ASC), (0, ASC)]);
    }

    #[test]
    fn a_sort_on_a_column_the_query_does_not_produce_is_an_error() {
        let e = group_key_order(&KEYS, &names(), &sort(&[("total", ASC)])).unwrap_err().to_string();
        assert!(e.contains("total"), "{e}");
    }

    #[test]
    fn the_query_document_rejects_unknown_and_removed_fields() {
        for json in [
            r#"{"from":"x.parquet","project":[{"expr":"a"}],"rename":["b"]}"#,
            r#"{"from":"x.parquet","filters":[]}"#,
            r#"{"from":"x.parquet","project":[{"expr":"a","as":"b"}]}"#,
            r#"{"from":"x.parquet","sort":[{"expr":"a","dir":"ASC"}]}"#,
        ] {
            assert!(Query::new(json).is_err(), "accepted {json}");
        }
    }

    #[test]
    fn columns_are_read_in_the_order_the_query_names_them() {
        let q = Query::new(r#"{
            "from": "x.parquet",
            "filter": [{"expr":"z","oper":"GT","value":1}, {"expr":"a","oper":"LT","value":9}],
            "project": [{"expr":"m"}, {"expr":"a"}, {"expr":"b","aggregate":"sum"}]
        }"#).unwrap();
        assert_eq!(q.get_column_names(), vec!["z", "a", "m", "b"]);
    }
}
