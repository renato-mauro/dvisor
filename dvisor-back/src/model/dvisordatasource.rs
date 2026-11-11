use serde::{Deserialize, Serialize};
use crate::{error::DvisorResult, model::tiledata::TileData};

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum DvisorType {
    I8,
    I16,
    I32,
    I64,
    Float,
    Decimal(i32),
    Double,
    String,
    Date
}

pub trait IDvisorDataSource {
    fn get_column_count(&self, row_group: usize) -> DvisorResult<usize>;
    fn get_row_group_count(&self) -> usize;
    fn get_num_rows_row_group(&self, row_group: usize) -> DvisorResult<i64>;
    fn get_num_rows(&self) -> i64;
    fn get_tile(&self, row_group: usize, column: usize) -> DvisorResult<TileData>;
    fn get_i8_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i8>>;
    fn get_i16_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i16>>;
    fn get_i32_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i32>>;
    fn get_i64_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<i64>>;
    fn get_float_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<f32>>;
    fn get_decimal_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<f32>>;
    fn get_double_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<f64>>;
    fn get_string_tile(&self, row_group: usize, column: usize) -> DvisorResult<Vec<String>>;
    fn get_column_name(&self, row_group: usize, column: usize) -> DvisorResult<String>;
    fn get_column_index(&self, row_group: usize, column_name: &str) -> DvisorResult<usize>;
    fn get_column_internal_type(&self, row_group: usize, column: usize) -> DvisorResult<DvisorType>;
    fn get_column_byte_range(&self, row_group: usize, column: usize) -> DvisorResult<(u64,u64)>;
}
