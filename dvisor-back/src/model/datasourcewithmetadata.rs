use std::rc::Rc;
use crate::{error::DvisorResult, model::{dvisordatasource::{DvisorType, IDvisorDataSource}, metadata::Metadata, tablefragment::DvisorTableFragment, tiledata::{FloatOperator, IntOperator, StringOperator, TileData}}};

pub trait ITableFragmentProvider {
    fn get_fragment_count(&self) -> usize;
    // `col_names: None` means "select all columns" (no projection was
    // specified at all); `Some(names)` means exactly those columns, which
    // may be an empty list — e.g. `SELECT count() FROM t` with no WHERE
    // needs zero physical columns. Conflating "zero needed" with "all
    // needed" here previously made every count()-only query read and
    // decode every column in the file — see the perf note this fixes.
    fn get_table_fragment(&self, fragment_id: usize, col_names: &Option<Vec<String>>) -> DvisorResult<DvisorTableFragment>;
    fn check_feasible_int(&self, _fragment_id: usize, _col_name: &str, _oper: &IntOperator) -> DvisorResult<bool> { Ok(true) }
    fn check_feasible_float(&self, _fragment_id: usize, _col_name: &str, _oper: &FloatOperator) -> DvisorResult<bool> { Ok(true) }
    fn check_feasible_string(&self, _fragment_id: usize, _col_name: &str, _oper: &StringOperator) -> DvisorResult<bool> { Ok(true) }
}

pub struct DataSourceWithMetadata {
    ds: Box<dyn IDvisorDataSource>,
    metadata: Option<Metadata>,
}

impl DataSourceWithMetadata {

    pub fn new(ds: Box<dyn IDvisorDataSource>, metadata: Option<Metadata>) -> DataSourceWithMetadata {
        DataSourceWithMetadata { ds, metadata }
    }

    #[inline]
    fn get_column_data(&self, row_group: usize, column_name: &str) -> DvisorResult<Rc<TileData>> {
        let index = self.ds.get_column_index(row_group, column_name)?;
        let tp = if let Some(metadata) = &self.metadata {
            metadata.get_column_pref_type(row_group, index)?
        } else {
            self.ds.get_column_internal_type(row_group, index)?
        };
        match tp {
            DvisorType::I8 => {
                Ok(Rc::new(TileData::I8(Rc::new(self.ds.get_i8_tile(row_group, index)?))))
            },
            DvisorType::I16 => {
                Ok(Rc::new(TileData::I16(Rc::new(self.ds.get_i16_tile(row_group, index)?))))
            },
            DvisorType::I32 => {
                Ok(Rc::new(TileData::I32(Rc::new(self.ds.get_i32_tile(row_group, index)?))))
            },
            DvisorType::I64 => {
                Ok(Rc::new(TileData::I64(Rc::new(self.ds.get_i64_tile(row_group, index)?))))
            },
            DvisorType::Float => {
                Ok(Rc::new(TileData::Float(Rc::new(self.ds.get_float_tile(row_group, index)?))))
            },
            DvisorType::Decimal(scale) => {
                Ok(Rc::new(TileData::Decimal(Rc::new(self.ds.get_decimal_tile(row_group, index)?),scale)))
            },
            DvisorType::Double => {
                Ok(Rc::new(TileData::Double(Rc::new(self.ds.get_double_tile(row_group, index)?))))
            }
            DvisorType::String => {
                Ok(Rc::new(TileData::String(Rc::new(self.ds.get_string_tile(row_group, index)?))))
            }
            DvisorType::Date => {
                Ok(Rc::new(TileData::Date(Rc::new(self.ds.get_i32_tile(row_group, index)?))))
            }
        }
    }
}

impl ITableFragmentProvider for DataSourceWithMetadata {
    fn get_fragment_count(&self) -> usize {
        self.ds.get_row_group_count()
    }

    fn get_table_fragment(&self, fragment_id: usize, col_names: &Option<Vec<String>>) -> DvisorResult<DvisorTableFragment> {
        let mut columns: Vec<(String, Rc<TileData>)> = Vec::new();
        match col_names {
            Some(names) => {
                for column_name in names {
                    let td: Rc<TileData> = self.get_column_data(fragment_id, column_name)?;
                    columns.push((column_name.to_string(),td));
                }
            }
            None => { // SELECT ALL COLUMNS
                for i in 0..self.ds.get_column_count(fragment_id)? {
                    let column_name = self.ds.get_column_name(fragment_id, i)?;
                    let td = self.get_column_data(fragment_id, &column_name)?;
                    columns.push((column_name,td));
                }
            }
        }
        // Always fetched independently of which/how many columns were
        // actually loaded above (a cheap, in-memory lookup on the row
        // group's already-parsed footer metadata, not a data read) — see
        // DvisorTableFragment's row_count field for why this can't be
        // inferred from `columns` itself.
        let row_count = self.ds.get_num_rows_row_group(fragment_id)? as usize;
        Ok(DvisorTableFragment::new(None,columns,row_count))
    }

    fn check_feasible_int(&self, fragment_id: usize, column_name: &str, oper: &IntOperator) -> DvisorResult<bool> {
        if let Some(metadata) = &self.metadata {
            Ok(metadata.check_feasible_int(fragment_id, column_name, oper)?)
        } else {
            Ok(true)
        }
    }    

    fn check_feasible_float(&self, fragment_id: usize, column_name: &str, oper: &FloatOperator) -> DvisorResult<bool> {
        if let Some(metadata) = &self.metadata {
            Ok(metadata.check_feasible_float(fragment_id, column_name, oper)?)
        } else {
            Ok(true)
        }
    }    

    fn check_feasible_string(&self, fragment_id: usize, column_name: &str, oper: &StringOperator) -> DvisorResult<bool> {
        if let Some(metadata) = &self.metadata {
            Ok(metadata.check_feasible_string(fragment_id, column_name, oper)?)
        } else {
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::err;
    use crate::model::enums::AggFunction;

    // Minimal in-memory IDvisorDataSource — three I32 columns, one row
    // group — just enough to drive get_table_fragment without any wasm or
    // network dependency (ParquetDataSource can't run under plain `cargo
    // test`, since it ultimately calls the wasm-bindgen-imported
    // js_fetch_range/js_get_content_length).
    struct MockDataSource;
    const MOCK_COLUMNS: [&str; 3] = ["a", "b", "c"];

    impl IDvisorDataSource for MockDataSource {
        fn get_column_count(&self, _row_group: usize) -> DvisorResult<usize> { Ok(MOCK_COLUMNS.len()) }
        fn get_row_group_count(&self) -> usize { 1 }
        fn get_num_rows_row_group(&self, _row_group: usize) -> DvisorResult<i64> { Ok(2) }
        fn get_num_rows(&self) -> i64 { 2 }
        fn get_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<TileData> { err!("not used by this test") }
        fn get_i8_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<i8>> { err!("column is I32 in this mock") }
        fn get_i16_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<i16>> { err!("column is I32 in this mock") }
        fn get_i32_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<i32>> { Ok(vec![1, 2]) }
        fn get_i64_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<i64>> { err!("column is I32 in this mock") }
        fn get_float_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<f32>> { err!("column is I32 in this mock") }
        fn get_decimal_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<f32>> { err!("column is I32 in this mock") }
        fn get_double_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<f64>> { err!("column is I32 in this mock") }
        fn get_string_tile(&self, _row_group: usize, _column: usize) -> DvisorResult<Vec<String>> { err!("column is I32 in this mock") }
        fn get_column_name(&self, _row_group: usize, column: usize) -> DvisorResult<String> { Ok(MOCK_COLUMNS[column].to_string()) }
        fn get_column_index(&self, _row_group: usize, column_name: &str) -> DvisorResult<usize> {
            MOCK_COLUMNS.iter().position(|c| c.eq_ignore_ascii_case(column_name)).ok_or_else(|| format!("Column {} not found", column_name).into())
        }
        fn get_column_internal_type(&self, _row_group: usize, _column: usize) -> DvisorResult<DvisorType> { Ok(DvisorType::I32) }
        fn get_column_byte_range(&self, _row_group: usize, _column: usize) -> DvisorResult<(u64, u64)> { Ok((0, 0)) }
    }

    fn provider() -> DataSourceWithMetadata {
        DataSourceWithMetadata::new(Box::new(MockDataSource), None)
    }

    fn fetched_names(fragment: &DvisorTableFragment) -> Vec<String> {
        let mut names: Vec<String> = fragment.get_columns().iter().map(|(n, _)| n.clone()).collect();
        names.sort();
        names
    }

    // The bug this guards against: `SELECT count() FROM t` (no WHERE, no
    // other projected column) legitimately needs zero physical columns —
    // that must not be read as "no columns specified, fetch all of them"
    // (which is what `Some(vec![])` used to be conflated with, back when
    // this parameter was a bare `Vec<String>` and emptiness was the only
    // signal available).
    #[test]
    fn some_empty_columns_fetches_nothing_but_keeps_the_true_row_count() {
        let fragment = provider().get_table_fragment(0, &Some(vec![])).unwrap();
        assert!(fetched_names(&fragment).is_empty(), "expected zero columns fetched, got {:?}", fetched_names(&fragment));
        assert_eq!(fragment.len(), 2, "row count must come from row-group metadata, not from a loaded column's length");
    }

    #[test]
    fn some_named_columns_fetches_exactly_those() {
        let fragment = provider().get_table_fragment(0, &Some(vec!["b".to_string()])).unwrap();
        assert_eq!(fetched_names(&fragment), vec!["b".to_string()]);
        assert_eq!(fragment.len(), 2);
    }

    #[test]
    fn none_fetches_every_column() {
        let fragment = provider().get_table_fragment(0, &None).unwrap();
        assert_eq!(fetched_names(&fragment), vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        assert_eq!(fragment.len(), 2);
    }

    // End-to-end reproduction of the actual bug: `SELECT count() FROM t`
    // with no WHERE goes through generate_plan() -> QueryPipeline with
    // columns=Some(vec![]) (count() needs no physical column) -> sort([])
    // -> group_by([], [COUNT], [alias]), exactly like
    // PipelineOperatorGroupBy::execute does. Before the row_count fix,
    // sort()'s `(0..self.len()).collect()` built an EMPTY selection from a
    // columnless fragment, so the "whole table" count silently came back
    // as 0 instead of the row group's real row count.
    #[test]
    fn count_with_no_columns_needed_still_counts_every_row() {
        let fragment = provider().get_table_fragment(0, &Some(vec![])).unwrap();
        let sorted = fragment.sort(&vec![]).unwrap();
        let grouped = sorted.group_by(&vec![], &[], &vec![("".to_string(), AggFunction::COUNT)], &vec!["n".to_string()]).unwrap();
        let counts = &grouped.get_columns()[0].1;
        assert_eq!(counts.len(), 1, "no GROUP BY columns means the whole table is one group");
        match counts.as_ref() {
            TileData::I32(v) => assert_eq!(v[0], 2, "count() must reflect the row group's real row count, not the number of loaded columns"),
            other => panic!("expected TileData::I32 for a COUNT column, got {:?}", other),
        }
    }
}
