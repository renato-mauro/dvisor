use std::{cmp::Ordering, rc::Rc};

use crate::{ensure, err, error::DvisorResult, model::{enums::{AggFunction, SortOrder}, tiledata::{FloatOperator, IntOperator, StringOperator, TileData}}, query::{helpercalccol, ql::CalculatedExpression}};

pub struct DvisorTableFragment {
    selection: Option<Rc<Vec<usize>>>,
    group_counts: Option<Rc<TileData>>,
    columns: Vec<(String,Rc<TileData>)>,
    aggregate_functions: Option<Vec<AggFunction>>,
    group_cmp: Option<Vec<(String,SortOrder)>>,
    sort_cmp: Option<Vec<(String,SortOrder)>>,
    // The true number of physical rows backing this fragment, tracked
    // explicitly rather than inferred from a loaded column's length —
    // a fragment can legitimately have zero loaded columns (e.g. `SELECT
    // count() FROM t` needs none) while still representing many real rows.
    // Inferring row count from `columns.first()` used to make any such
    // query silently see zero rows once it stopped needlessly loading every
    // column in the file.
    row_count: usize
}

impl DvisorTableFragment {
    pub fn new(selection: Option<Rc<Vec<usize>>>, columns: Vec<(String,Rc<TileData>)>, row_count: usize) -> Self {
        Self { selection, columns, group_counts: None, aggregate_functions: None, group_cmp: None, sort_cmp: None, row_count }
    }

    fn get_column_data(&self, column_name: &str) -> DvisorResult<Rc<TileData>> {
        if let Some((_,td)) = self.columns.iter().find(|(name,_)|name.eq_ignore_ascii_case(column_name)) {
            Ok(td.clone())
        } else {
            let mut names = Vec::new();
            for (name,_) in &self.columns {
                names.push(name.clone());
            }
            if let Some(s) = crate::utils::suggest(&names, &column_name.to_lowercase()) {
                err!("Column {} not found in table. Did you mean '{}'?",column_name,s)
            } else {
                err!("Column {} not found in table.",column_name)
            }
        }
    }

    pub fn get_columns(&self) -> &Vec<(String,Rc<TileData>)> {
        &self.columns
    }

    // The JavaScript the client merges this fragment into the result so far
    // with: `obj` builds a row, `comb` combines the aggregates of two rows of
    // the same group, `cmp` compares rows by the group columns, `sort` by the
    // query's sort. The client builds its merge from the first fragment's
    // helpers and applies it to every fragment of the query, from every
    // worker, so this text must depend only on the query and the column
    // types.
    pub fn js_helpers(&self) -> DvisorResult<String> {
        let obj = self.js_helper_obj();
        let comb = self.js_helper_combine_aggregate().unwrap_or("null".to_string());
        let cmp = self.js_helper_compare_combine()?.unwrap_or("null".to_string());
        let sort = self.js_helper_compare_sort()?.unwrap_or("null".to_string());
        Ok(format!("({{'obj':({}),'comb':({}),'cmp':({}),'sort':({})}})",obj,comb,cmp,sort))
    }

    fn column_position(&self, column_name: &str) -> DvisorResult<usize> {
        match self.columns.iter().position(|(cn,_)|cn.eq_ignore_ascii_case(column_name)) {
            Some(pos) => Ok(pos),
            None => {
                let names: Vec<&str> = self.columns.iter().map(|(cn,_)| cn.as_str()).collect();
                err!("Column '{}' is not among the result columns ({})", column_name, names.join(", "))
            }
        }
    }

    fn js_helper_combine_aggregate(&self) -> Option<String> {
        if let Some(aggfs) = &self.aggregate_functions {
            let mut resp = String::new();
            let mut sep = "";
            let mut i = 0;
            let mut count_exp = String::new();
            for (name, td) in &self.columns {
                let agf = &aggfs[i];
                let e1 = format!("a[i]['{}']",name);
                // The fragment's value as the row holds it (a Date for a date
                // column, as built by `obj`), so that min/max compare values
                // of the same kind and keep one.
                let e2 = td.js_helper_obj(format!("b[{}][j]",i));
                if matches!(agf,AggFunction::COUNT) {
                    count_exp = format!("const ca={};const cb={};",e1,e2);
                }
                if let Some(combiner) = agf.js_helper_combine(e1, e2) {
                    resp.push_str(&format!("{}{}", sep, combiner));
                    sep = ",";
                }
                i+=1;
            }
            Some(format!("(a,b,i,j)=>{{{}{}}}",count_exp,resp))
        } else {
            None
        }
    }

    fn js_helper_obj(&self) -> String {
        let mut resp = String::new();
        let mut sep = "";
        let mut i = 0;
        for (name, td) in &self.columns {
            resp.push_str(&format!("{}'{}':{}",sep,name,td.js_helper_obj(format!("b[{}][j]",i))));
            i+=1;
            sep = ",";
        }
        format!("(b,j)=>({{{}}})",resp)
    }

    fn js_helper_compare_combine(&self) -> DvisorResult<Option<String>> {
        if let Some(g) = &self.group_cmp {
            let mut s1 = String::new();
            let mut s2 = String::new();
            let mut sep = "";
            for (column_name,so) in g {
                let pos = self.column_position(column_name)?;
                let td = &self.columns[pos].1;
                let e1 = format!("a[i]['{}']",self.columns[pos].0);
                let e2 = td.js_helper_combine(format!("b[{}][j]",pos));
                match so {
                    SortOrder::ASC => {
                        s1.push_str(&format!("{}{}",sep,e1));
                        s2.push_str(&format!("{}{}",sep,e2));
                    }
                    SortOrder::DESC => {
                        s1.push_str(&format!("{}{}",sep,e2));
                        s2.push_str(&format!("{}{}",sep,e1));
                    }
                }
                sep = ","
            }
            Ok(Some(format!("(a,b,i,j,cmp)=>cmp([{}],[{}])",s1,s2)))
        } else {
            Ok(None)
        }
    }

    fn js_helper_compare_sort(&self) -> DvisorResult<Option<String>> {
        if let Some(s) = &self.sort_cmp {
            let mut s1 = String::new();
            let mut s2 = String::new();
            let mut sep = "";
            for (column_name,so) in s {
                let pos = self.column_position(column_name)?;
                let e1 = format!("a['{}']",self.columns[pos].0);
                let e2 = format!("b['{}']",self.columns[pos].0);
                match so {
                    SortOrder::ASC => {
                        s1.push_str(&format!("{}{}",sep,e1));
                        s2.push_str(&format!("{}{}",sep,e2));
                    }
                    SortOrder::DESC => {
                        s1.push_str(&format!("{}{}",sep,e2));
                        s2.push_str(&format!("{}{}",sep,e1));
                    }
                }
                sep = ",";
            }
            Ok(Some(format!("(cmp)=>((a,b)=>cmp([{}],[{}]))",s1,s2)))
        } else {
            Ok(None)
        }
    }

    pub fn clone_table(&self, selection: Option<Rc<Vec<usize>>>) -> DvisorTableFragment {
        let mut columns = Vec::with_capacity(self.columns.len());
        for (column_name, td) in &self.columns {
            columns.push((column_name.clone(),td.clone()));
        }
        let group_counts = if let Some(gc) = &self.group_counts {
            Some(gc.clone())
        } else {
            None
        };
        let aggregate_functions = if let Some(a) = &self.aggregate_functions {
            Some(a.clone())
        } else {
            None
        };
        let group_cmp = if let Some(g) = &self.group_cmp {
            Some(g.clone())
        } else {
            None
        };
        let sort_cmp = if let Some(s) = &self.sort_cmp {
            Some(s.clone())
        } else {
            None
        };
        DvisorTableFragment { selection, columns, group_counts, aggregate_functions, group_cmp, sort_cmp, row_count: self.row_count }
    }

    pub fn project(&self, column_names: &Vec<(String,String)>) -> DvisorResult<DvisorTableFragment> {
        let mut columns = Vec::with_capacity(column_names.len());
        for (column_name,alias) in column_names {
            let td = self.get_column_data(column_name)?;
            columns.push((alias.clone(),td.clone()));
        }
        let group_counts = if let Some(gc) = &self.group_counts {
            Some(gc.clone())
        } else {
            None
        };
        let aggregate_functions = if let Some(a) = &self.aggregate_functions {
            Some(a.clone())
        } else {
            None
        };
        let group_cmp = if let Some(g) = &self.group_cmp {
            Some(g.clone())
        } else {
            None
        };
        let sort_cmp = if let Some(s) = &self.sort_cmp {
            Some(s.clone())
        } else {
            None
        };
        let selection = if let Some(s) = &self.selection {
            Some(s.clone())
        } else {
            None
        };
        Ok(DvisorTableFragment { selection, columns, group_counts, aggregate_functions, group_cmp, sort_cmp, row_count: self.row_count })
    }

    pub fn select_int(&self, column_name: &str, operator: &IntOperator) -> DvisorResult<DvisorTableFragment> {
        let td = self.get_column_data(&column_name)?;
        let selection = if let Some(index) = &self.selection {
            td.filter_index_int(index.as_ref(), &operator)?
        }
        else {
            td.filter_int(&operator)?
        };
        Ok(self.clone_table(Some(Rc::new(selection))))
    }

    pub fn select_float(&self, column_name: &str, operator: &FloatOperator) -> DvisorResult<DvisorTableFragment> {
        let td = self.get_column_data(&column_name)?;
        let selection = if let Some(index) = &self.selection {
            td.filter_index_float(index.as_ref(), &operator)?
        }
        else {
            td.filter_float(&operator)?
        };
        Ok(self.clone_table(Some(Rc::new(selection))))
    }

    pub fn select_string(&self, column_name: &str, operator: &StringOperator) -> DvisorResult<DvisorTableFragment> {
        let td = self.get_column_data(&column_name)?;
        let selection = if let Some(index) = &self.selection {
            td.filter_index_string(index.as_ref(), &operator)?
        }
        else {
            td.filter_string(&operator)?
        };
        Ok(self.clone_table(Some(Rc::new(selection))))
    }

    fn get_comparator(&self, column_order: &Vec<(String,SortOrder)>) -> DvisorResult<Box<dyn Fn(&usize, &usize) -> Ordering>> {
        let mut functions= Vec::new();
        for (column_name, sort_order) in column_order {
            let td = self.get_column_data(column_name)?;
            functions.push(td.get_comparator(*sort_order));
        }
        if functions.len() == 1 {
            Ok(functions.pop().unwrap())
        } else {
            Ok(Box::new(
                move |a: &usize, b: &usize| -> Ordering {
                    for f in &functions {
                        let resp = f(a,b);
                        if resp != Ordering::Equal {
                            return resp;
                        }
                    }
                    Ordering::Equal
                }
            ))
        }
    }

    pub fn len(&self) -> usize {
        self.row_count
    }

    pub fn sort(&self, column_order: &Vec<(String,SortOrder)>) -> DvisorResult<DvisorTableFragment> {
        let mut selection = if let Some(sel) = &self.selection {
            (**sel).clone()
        } else {
            (0..self.len()).collect()
        };
        if self.group_cmp.is_none() {
            let cmp = self.get_comparator(column_order)?;
            selection.sort_by(cmp);
        }
        let mut ct = self.clone_table(Some(Rc::new(selection)));
        ct.sort_cmp = Some(column_order.clone());
        Ok(ct)
    }

    fn group_counts(&self, column_order: &Vec<(String,SortOrder)>) -> DvisorResult<Vec<i32>> {
        let cmp = self.get_comparator(column_order)?;
        let mut counts = Vec::with_capacity(self.selection.as_ref().unwrap().len());
        if let Some(selection) = &self.selection {
            if let Some(first_index) = selection.first() {
                let mut group_count = 1;
                let mut current_group_head = *first_index;
                for i in &selection[1..] {
                    if cmp(&current_group_head,i).is_ne() {
                        current_group_head = *i;
                        counts.push(group_count);
                        group_count = 1;
                    } else {
                        group_count += 1;
                    }
                }
                counts.push(group_count);
            }
        }
        Ok(counts)
    }

    pub fn pack_data(&self) -> DvisorResult<DvisorTableFragment> {
        if let Some(sel) = &self.selection {
            let mut columns = Vec::with_capacity(self.columns.len());
            for (a,b) in self.columns.iter() {
                columns.push((a.clone(),Rc::new(b.pack_data(sel)?)));
            }
            let mut tf = DvisorTableFragment::new(None,columns,sel.len());
            tf.sort_cmp = self.sort_cmp.clone();
            Ok(tf)
        } else {
            Ok(self.clone_table(None))
        }
    }

    // Groups the (sorted) rows by `column_order`, the group columns by the
    // names they are read by; `output_names` are the same columns' names in
    // the result, which the merge compares by (see js_helpers).
    pub fn group_by(&self, column_order: &Vec<(String,SortOrder)>, output_names: &[String], aggregate: &Vec<(String,AggFunction)>, alias: &Vec<String>) -> DvisorResult<DvisorTableFragment> {
        ensure!(output_names.len()==column_order.len(),"GroupBy has {} group columns but {} output names",column_order.len(),output_names.len());
        if let Some(sel) = &self.selection {
            let counts = Rc::new(self.group_counts(&column_order)?);
            let mut columns_grouped = Vec::new();
            let mut i = 0;
            let mut aggfs = Vec::with_capacity(aggregate.len()+1);
            let mut needs_count = false;
            let mut has_count = false;
            for (name, agg) in aggregate {
                aggfs.push(*agg);
                let is_count = match agg {
                    AggFunction::AVG => {
                        needs_count = true;
                        false
                    },
                    AggFunction::COUNT => true,
                    _ => false
                };
                let col_name = alias[i].clone();
                if is_count {
                    has_count = true;
                    columns_grouped.push((col_name,Rc::new(TileData::I32(counts.clone()))));
                } else {
                    let td = self.get_column_data(name)?;
                    columns_grouped.push((col_name,Rc::new(td.aggregate(sel, &counts, *agg)?)));
                }
                i+=1;
            }
            let group_counts = Some(Rc::new(TileData::I32(counts.clone())));
            if !has_count && needs_count {
                columns_grouped.push(("__count__".to_string(),Rc::new(TileData::I32(counts.clone()))));
                aggfs.push(AggFunction::COUNT);
            }
            Ok(DvisorTableFragment {
                selection: None,
                columns: columns_grouped,
                group_counts,
                aggregate_functions: Some(aggfs),
                group_cmp: Some(column_order.iter().zip(output_names).map(|((_,so),name)|(name.clone(),*so)).collect()),
                sort_cmp: None,
                row_count: counts.len()
            })
        }
        else {
            err!("Invalid GroupBY Call")
        }
    }

    pub fn create_calculated_field(&self, expression: &CalculatedExpression) -> DvisorResult<DvisorTableFragment> {
        let col = helpercalccol::create_calc_col(expression, &self.selection, &self.columns)?;
        let sel_clone = if let Some(sel) = &self.selection {
            Some(sel.clone())
        } else {
            None
        };
        let mut resp = self.clone_table(sel_clone);
        resp.columns.push((expression.alias.clone(),Rc::new(col)));
        Ok(resp)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use SortOrder::ASC;

    // Rows of a fragment: a region, a sex column read as `sexo` (and renamed
    // `sex` in the output), and a date.
    fn fragment(region: TileData, sexo: Vec<i32>, dates: Vec<i32>) -> DvisorTableFragment {
        let rows = sexo.len();
        DvisorTableFragment::new(None, vec![
            ("region".to_string(), Rc::new(region)),
            ("sexo".to_string(), Rc::new(TileData::I32(Rc::new(sexo)))),
            ("dt".to_string(), Rc::new(TileData::Date(Rc::new(dates)))),
        ], rows)
    }

    // SELECT region, sexo AS sex, min(dt) AS d0, max(dt) AS d1, count() AS n
    fn grouped(f: &DvisorTableFragment, output: [&str; 2]) -> DvisorTableFragment {
        let keys = vec![("region".to_string(), ASC), ("sexo".to_string(), ASC)];
        let aggregates = vec![
            ("region".to_string(), AggFunction::FIRST),
            ("sexo".to_string(), AggFunction::FIRST),
            ("dt".to_string(), AggFunction::MIN),
            ("dt".to_string(), AggFunction::MAX),
            ("".to_string(), AggFunction::COUNT),
        ];
        let alias = ["region", "sex", "d0", "d1", "n"].map(String::from).to_vec();
        f.sort(&keys).unwrap().group_by(&keys, &output.map(String::from), &aggregates, &alias).unwrap()
    }

    fn helpers(region: TileData, sexo: Vec<i32>, dates: Vec<i32>) -> String {
        grouped(&fragment(region, sexo, dates), ["region", "sex"]).js_helpers().unwrap()
    }

    #[test]
    fn the_merge_compares_group_columns_by_their_output_names() {
        let h = helpers(TileData::I8(Rc::new(vec![1, 1, 2])), vec![1, 2, 1], vec![17532, 17533, 17534]);
        assert!(h.contains("cmp([a[i]['region'],a[i]['sex']],[b[0][j],b[1][j]])"), "{h}");
    }

    #[test]
    fn every_fragment_of_a_query_gets_the_same_helpers() {
        // Different rows, groups and integer widths, as different row groups have.
        let first = helpers(TileData::I8(Rc::new(vec![1, 1, 2])), vec![1, 2, 1], vec![17532, 17533, 17534]);
        let other = helpers(TileData::I16(Rc::new(vec![300, 7])), vec![0, 2], vec![18000, 18001]);
        assert_eq!(first, other);
    }

    #[test]
    fn min_and_max_of_a_date_merge_dates_with_dates() {
        let h = helpers(TileData::I8(Rc::new(vec![1, 1])), vec![1, 1], vec![17532, 17600]);
        assert!(h.contains("a[i]['d0']=a[i]['d0']<new Date(86400000*b[2][j])?a[i]['d0']:new Date(86400000*b[2][j])"), "{h}");
        assert!(h.contains("a[i]['d1']=a[i]['d1']>new Date(86400000*b[3][j])?a[i]['d1']:new Date(86400000*b[3][j])"), "{h}");
    }

    #[test]
    fn a_group_column_missing_from_the_result_is_an_error() {
        let f = grouped(&fragment(TileData::I8(Rc::new(vec![1])), vec![1], vec![17532]), ["region", "gender"]);
        let e = f.js_helpers().unwrap_err().to_string();
        assert!(e.contains("gender"), "{e}");
    }
}
