use serde::{Deserialize, Serialize};
use crate::{err, error::DvisorResult, model::{dvisordatasource::{DvisorType, IDvisorDataSource}, stats::Statistics, tiledata::{FloatOperator, IntOperator, StringOperator}}};

#[derive(Debug, Deserialize, Serialize)]
pub struct RowGroupMetadata {
    num_rows: i64,
    columns: Vec<ColumnMetadata>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Metadata {
    num_rows: i64,
    row_groups: Vec<RowGroupMetadata>,
    stats: Statistics,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ColumnMetadata {
    column_name: String,
    internal_type: DvisorType,
    pref_type: DvisorType,
    offset: u64,
    length: u64
}

impl Metadata {
    pub fn new(ds: &Box<dyn IDvisorDataSource>) -> DvisorResult<Metadata> {
        let num_rows = ds.get_num_rows();
        let num_row_groups = ds.get_row_group_count();
        let stats = Statistics::new(ds)?;
        let mut row_groups = Vec::new();
        for i in 0..num_row_groups {
            row_groups.push(RowGroupMetadata::new(ds,&stats,i)?);
        }
        Ok(Metadata { num_rows, stats, row_groups})
    }

    pub fn from_json(json_str: &str) -> DvisorResult<Metadata> {
        Ok(serde_json::from_str(json_str)?)
    }

    pub fn to_json(&self) -> DvisorResult<String> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn get_column_index(&self, row_group: usize, column_name: &str) -> DvisorResult<usize> {
        if let Some(rg) = self.row_groups.get(row_group) {
            rg.get_column_index(column_name)
        } else {
            err!("Invalid Row Group {}",row_group)
        }
    }

    pub fn get_column_pref_type(&self, row_group: usize, column: usize) -> DvisorResult<DvisorType> {
        if let Some(rg) = self.row_groups.get(row_group) {
            rg.get_column_pref_type(column)
        } else {
            err!("Invalid Row Group {}",row_group)
        }
    }

    // A `column_name` here may be a calculated column (e.g. the alias of a
    // WHERE clause hoisted into `calc[]`, such as `year(DTNASC)`), which
    // only exists once the pipeline actually computes it — never in this
    // file's physical metadata. This check is purely an optimization to
    // skip fragments that provably can't match using stored statistics;
    // when the column isn't a physical one, statistics can't rule it out,
    // so the fragment is kept (`true`) and the real, correct filtering
    // still happens later during execution.
    pub fn check_feasible_int(&self, row_group: usize, column_name: &str, oper: &IntOperator) -> DvisorResult<bool> {
        if let Ok(column_index) = self.get_column_index(row_group, column_name) {
            Ok(self.stats.check_feasible_int(row_group, column_index, oper))
        } else {
            Ok(true)
        }
    }

    pub fn check_feasible_float(&self, row_group: usize, column_name: &str, oper: &FloatOperator) -> DvisorResult<bool> {
        if let Ok(column_index) = self.get_column_index(row_group, column_name) {
            Ok(self.stats.check_feasible_float(row_group, column_index, oper))
        } else {
            Ok(true)
        }
    }

    pub fn check_feasible_string(&self, row_group: usize, column_name: &str, oper: &StringOperator) -> DvisorResult<bool> {
        if let Ok(column_index) = self.get_column_index(row_group, column_name) {
            Ok(self.stats.check_feasible_string(row_group, column_index, oper))
        } else {
            Ok(true)
        }
    }



    // pub fn get_row_group_count(&self) -> usize {
    //     self.row_groups.len()
    // }
}

impl RowGroupMetadata {

    pub fn new(ds:&Box<dyn IDvisorDataSource>, stats: &Statistics, row_group: usize) -> DvisorResult<RowGroupMetadata> {
        let num_rows = ds.get_num_rows_row_group(row_group)?;
        let nc = ds.get_column_count(row_group)?;
        let mut columns = Vec::new();
        for i in 0..nc {
            columns.push(ColumnMetadata::new(ds, stats, row_group,i)?);
        }
        Ok(RowGroupMetadata { num_rows, columns })
    }

    pub fn get_column_index(&self, column_name: &str) -> DvisorResult<usize> {
        if let Some((i,_)) = self.columns.iter().enumerate().find(|(_,cmd)|cmd.column_name.eq_ignore_ascii_case(column_name)) {
            Ok(i)
        } else {
            err!("Column {} not found in table",column_name)
        }
    }
    
    pub fn get_column_pref_type(&self, column: usize) -> DvisorResult<DvisorType> {
        if let Some(col) = self.columns.get(column) {
            Ok(col.pref_type)
        } else {
            err!("Column {} not found in table",column)
        }
    }

}

impl ColumnMetadata {

    pub fn new(ds: &Box<dyn IDvisorDataSource>, stats: &Statistics, row_group: usize, column: usize) -> DvisorResult<ColumnMetadata> {
        let column_name = ds.get_column_name(row_group, column)?;
        let internal_type = ds.get_column_internal_type(row_group, column)?;
        let (offset, length) = ds.get_column_byte_range(row_group, column)?;
        if let Some(pref_type) = stats.get_pref_type(row_group, column)? {   
            Ok(ColumnMetadata { column_name, internal_type, pref_type, offset, length})
        } else {
            Ok(ColumnMetadata { column_name, internal_type, pref_type: internal_type, offset, length })
        }
    }

}
