use std::{collections::BTreeMap, io::{Error, ErrorKind}};
use serde::{Deserialize, Serialize};
use crate::{error::DvisorResult, model::{castfrommacros::{CmpF32, CmpI32}, dateutils::epoch, dvisordatasource::{DvisorType, IDvisorDataSource}, tiledata::{FloatOperator, IntOperator, StringOperator, TileData}}};

#[derive(Debug, Deserialize, Serialize)]
pub struct Statistics {
    tile_stats: Vec<Vec<Option<TileStatistics>>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TileStatisticsData<T> {
    range: Option<(T,T)>,
    exact: bool,
    histogram: Option<Vec<(T,usize)>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum TileStatistics {
    I8(Option<TileStatisticsData<i8>>),
    I16(Option<TileStatisticsData<i16>>),
    I32(Option<TileStatisticsData<i32>>),
    I64(Option<TileStatisticsData<i64>>),
    Float(Option<TileStatisticsData<f32>>),
    Decimal(Option<TileStatisticsData<f32>>,i32),
    Double(Option<TileStatisticsData<f64>>),
    String(Option<TileStatisticsData<String>>),
    Date(Option<TileStatisticsData<i32>>)
}

fn get_min_max<T>(data: &[T]) -> Option<(T,T)> 
    where T: PartialOrd + Copy
{
    if let Some((&first,rest)) = data.split_first() {
        let (mut min, mut max) = (first,first);
        for &val in rest {
            if val < min { min = val; }
            if val > max { max = val; }
        }
        Some((min,max))
    } else {
        None
    }
}

fn get_min_max_string(data: &[String]) -> Option<(String,String)> 
{
    if data.len() > 0 {
        let first = &data[0];
        let (mut min, mut max) = (first,first);
        for i in 1..data.len() {
            let val = &data[i];
            if val < min { min = val; } 
            if val > max { max = val; } 
        }
        Some((min.clone(),max.clone()))
    } else {
        None
    }
}

fn histogram_exact<T>(data: &[T]) -> Vec<(T, usize)> 
where T: Ord + Copy
{
    let mut m = BTreeMap::<T, usize>::new();
    for &x in data {
        *m.entry(x).or_insert(0) += 1;
    }
    m.into_iter().collect()
}

fn histogram_exact_string(data: &[String]) -> Vec<(String, usize)> 
{
    let mut m = BTreeMap::<String,usize>::new();
    for x in data.iter() {
        if let Some(v) = m.get_mut(x) {
            *v += 1;
        } else {
            m.insert(x.clone(), 1);
        }
    }
    m.into_iter().collect()
}

fn collect_stats_integers<T>(data: &[T]) -> Option<TileStatisticsData<T>>
where T: Ord + Copy 
{
    let limit = 16;
    let range = get_min_max(data);
    let mut exact = true;
    let mut histogram = histogram_exact(data);
    if histogram.len() > limit {
        let mut new_histogram = Vec::with_capacity(limit);
        exact = false;
        let fold_size = histogram.len() / limit;
        let fold_rem = histogram.len() % limit;
        let mut i = 0;
        for g in 0..limit {
            let size = fold_size + if g < fold_rem { 1 } else { 0 };
            let first = histogram[i].0;
            let mut sum = 0;
            for j in 0..size {
                sum += histogram[i+j].1;
            }
            new_histogram.push((first,sum));
            i += size;
        }
        histogram = new_histogram;
    }
    Some(TileStatisticsData { 
        range,
        exact, 
        histogram: Some(histogram), 
    })
}

fn collect_stats_floats<T>(data: &[T]) -> Option<TileStatisticsData<T>>
where T: PartialOrd + Copy
{
    let range = get_min_max(data);
    Some(TileStatisticsData { 
        range,
        exact: false, 
        histogram: None
    })
}

fn collect_stats_strings(data: &[String]) -> Option<TileStatisticsData<String>>
{
    let range = get_min_max_string(data);
    let histogram = histogram_exact_string(data);
    if histogram.len() < 256 {
        Some(TileStatisticsData { 
            range,
            exact: true, 
            histogram: Some(histogram)
        })
    } else {
        Some(TileStatisticsData { 
            range,
            exact: false, 
            histogram: None
        })
    }
}

fn check_feasible_int<T>(op_tsd: &Option<TileStatisticsData<T>>, oper: &IntOperator) -> bool 
where T: CmpI32 + Copy
{
    if let Some(tsd) = op_tsd {
        if let Some((min,max)) = &tsd.range {
            match oper {
                IntOperator::EQ(value) => {
                    (*min).le(*value) && (*max).ge(*value)
                },
                IntOperator::LT(value) => {
                    (*min).lt(*value)
                },
                IntOperator::GT(value) => {
                    (*max).gt(*value)
                },
                IntOperator::LE(value) => {
                    (*min).le(*value)
                },
                IntOperator::GE(value) => {
                    (*max).ge(*value)
                },
                _ => {
                    true
                }
            }
        } else {
            true
        }
    } else {
        true
    }
}

fn check_feasible_float<T>(op_tsd: &Option<TileStatisticsData<T>>, oper: &FloatOperator) -> bool 
where T: CmpF32 + Copy
{
    if let Some(tsd) = op_tsd {
        if let Some((min,max)) = &tsd.range {
            match oper {
                FloatOperator::EQ(value) => {
                    (*min).le(*value) && (*max).ge(*value)
                },
                FloatOperator::LT(value) => {
                    (*min).lt(*value)
                },
                FloatOperator::GT(value) => {
                    (*max).gt(*value)
                },
                FloatOperator::LE(value) => {
                    (*min).le(*value)
                },
                FloatOperator::GE(value) => {
                    (*max).ge(*value)
                },
                _ => {
                    true
                }
            }
        } else {
            true
        }
    } else {
        true
    }
}

impl Statistics {
    pub fn new(ds: &Box<dyn IDvisorDataSource>) -> DvisorResult<Statistics> {
        let nrg = ds.get_row_group_count();
        let mut tile_stats = Vec::new();
        for i in 0..nrg {
            let ncol = ds.get_column_count(i)?;
            let mut rgstats = Vec::new();
            for j in 0..ncol {
                let tile = ds.get_tile(i,j)?;
                match tile {
                    TileData::I8(itens) => {
                        rgstats.push(Some(TileStatistics::I8(collect_stats_integers(&itens))));
                    },
                    TileData::I16(itens) => {
                        rgstats.push(Some(TileStatistics::I16(collect_stats_integers(&itens))));
                    },
                    TileData::I32(itens) => {
                        rgstats.push(Some(TileStatistics::I32(collect_stats_integers(&itens))));
                    },
                    TileData::I64(itens) => {
                        rgstats.push(Some(TileStatistics::I64(collect_stats_integers(&itens))));
                    },
                    TileData::Float(itens) => {
                        rgstats.push(Some(TileStatistics::Float(collect_stats_floats(&itens))));
                    },
                    TileData::Decimal(itens,scale) => {
                        rgstats.push(Some(TileStatistics::Decimal(collect_stats_floats(&itens),scale)));
                    },
                    TileData::Double(itens) => {
                        rgstats.push(Some(TileStatistics::Double(collect_stats_floats(&itens))));
                    },
                    TileData::String(itens) => {
                        rgstats.push(Some(TileStatistics::String(collect_stats_strings(&itens))));
                    },
                    TileData::Date(itens) => {
                        rgstats.push(Some(TileStatistics::Date(collect_stats_integers(&itens))));
                    },
                }
            }
            tile_stats.push(rgstats);
        }
        Ok(Statistics { tile_stats })
    }

    pub fn get_pref_type(&self, row_group: usize, column: usize) -> Result<Option<DvisorType>,Error> {
        if row_group < self.tile_stats.len() {
            if column < self.tile_stats[row_group].len() {
                match &self.tile_stats[row_group][column] {
                    Some(ts) => {
                        match ts {
                            TileStatistics::I8(_) => {
                                Ok(Some(DvisorType::I8))
                            }
                            TileStatistics::I16(_) => {
                                Ok(Some(DvisorType::I8))
                            }
                            TileStatistics::I32(tsd) => {
                                if let Some(tsi32) = tsd {
                                    if let Some((min,max)) = tsi32.range {
                                        if min >= i8::MIN as i32 && max <= i8::MAX as i32 {
                                            Ok(Some(DvisorType::I8))
                                        }
                                        else if min >= i16::MIN as i32 && max <= i16::MAX as i32 {
                                            Ok(Some(DvisorType::I16))
                                        }
                                        else {
                                            Ok(Some(DvisorType::I32))
                                        }

                                    } else {
                                        Ok(Some(DvisorType::I32))
                                    }
                                } else {
                                    Ok(Some(DvisorType::I32))
                                }                                
                            }
                            TileStatistics::I64(tsd) => {
                                if let Some(tsi64) = tsd {
                                    if let Some((min,max)) = tsi64.range {
                                        if min >= i8::MIN as i64 && max <= i8::MAX as i64 {
                                            Ok(Some(DvisorType::I8))
                                        }
                                        else if min >= i16::MIN as i64 && max <= i16::MAX as i64 {
                                            Ok(Some(DvisorType::I16))
                                        }
                                        else if min >= i32::MIN as i64 && max <= i32::MAX as i64 {
                                            Ok(Some(DvisorType::I32))
                                        }
                                        else {
                                            Ok(Some(DvisorType::I64))
                                        }

                                    } else {
                                        Ok(Some(DvisorType::I64))
                                    }
                                } else {
                                    Ok(Some(DvisorType::I64))
                                }                                
                            }
                            TileStatistics::Float(_) => {
                                Ok(Some(DvisorType::Float))
                            }
                            TileStatistics::Decimal(_,scale) => {
                                Ok(Some(DvisorType::Decimal(*scale)))
                            }
                            TileStatistics::Double(_) => {
                                Ok(Some(DvisorType::Double))
                            }
                            TileStatistics::String(_) => {
                                Ok(Some(DvisorType::String))
                            }
                            TileStatistics::Date(_) => {
                                Ok(Some(DvisorType::Date))
                            }
                        }
                    },
                    None => {
                        Ok(None)
                    },
                }
            }
            else {
                Err(Error::new(ErrorKind::Other,format!("Column {} not defined for Row Group {}",column,row_group)))
            }
        } else {
            Err(Error::new(ErrorKind::Other,format!("Row Group {} not defined",row_group)))
        }
    }

    pub fn check_feasible_int(&self, row_group: usize, column: usize, oper: &IntOperator) -> bool {
        if row_group >= self.tile_stats.len() {
            false
        } else {
            let row_group_stat = &self.tile_stats[row_group];
            if column >= row_group_stat.len() {
                false
            } else {
                if let Some(ts) = &row_group_stat[column] {
                    match ts {
                        TileStatistics::I8(op_tsd) => {
                            check_feasible_int(op_tsd, oper)
                        }
                        TileStatistics::I16(op_tsd) => {
                            check_feasible_int(op_tsd, oper)
                        }
                        TileStatistics::I32(op_tsd) => {
                            check_feasible_int(op_tsd, oper)
                        }
                        TileStatistics::I64(op_tsd) => {
                            check_feasible_int(op_tsd, oper)
                        }
                        TileStatistics::Float(op_tsd) => {
                            check_feasible_int(op_tsd, oper)
                        }
                        TileStatistics::Decimal(op_tsd,_) => {
                            check_feasible_int(op_tsd, oper)
                        }
                        TileStatistics::Double(op_tsd) => {
                            check_feasible_int(op_tsd, oper)
                        }
                        TileStatistics::String(_) => {
                            // TODO: Tentar converter string para int??
                            true
                        }
                        TileStatistics::Date(_) => {
                            // TODO: Tratamento de Datas
                            true
                        }
                    }
                } else {
                    true
                }
            }
        }
    }

    pub fn check_feasible_float(&self, row_group: usize, column: usize, oper: &FloatOperator) -> bool {
        if row_group >= self.tile_stats.len() {
            false
        } else {
            let row_group_stat = &self.tile_stats[row_group];
            if column >= row_group_stat.len() {
                false
            } else {
                if let Some(ts) = &row_group_stat[column] {
                    match ts {
                        TileStatistics::I8(op_tsd) => {
                            check_feasible_float(op_tsd, oper)
                        }
                        TileStatistics::I16(op_tsd) => {
                            check_feasible_float(op_tsd, oper)
                        }
                        TileStatistics::I32(op_tsd) => {
                            check_feasible_float(op_tsd, oper)
                        }
                        TileStatistics::I64(op_tsd) => {
                            check_feasible_float(op_tsd, oper)
                        }
                        TileStatistics::Float(op_tsd) => {
                            check_feasible_float(op_tsd, oper)
                        }
                        TileStatistics::Decimal(op_tsd,_) => {
                            check_feasible_float(op_tsd, oper)
                        }
                        TileStatistics::Double(op_tsd) => {
                            check_feasible_float(op_tsd, oper)
                        }
                        TileStatistics::String(_) => {
                            // TODO: Tentar converter string para int??
                            true
                        }
                        TileStatistics::Date(_) => {
                            // TODO: Tratamento de Datas
                            true
                        }
                    }
                } else {
                    true
                }
            }
        }
    }

    pub fn check_feasible_string(&self, row_group: usize, column: usize, oper: &StringOperator) -> bool {
        if row_group >= self.tile_stats.len() {
            false
        } else {
            let row_group_stat = &self.tile_stats[row_group];
            if column >= row_group_stat.len() {
                false
            } else {
                if let Some(ts) = &row_group_stat[column] {
                    match ts {
                        TileStatistics::String(_) => {
                            true
                        }
                        TileStatistics::Date(op_tsd) => {
                            let op_oper = match oper {
                                StringOperator::EQ(operand) => {
                                    if let Ok(int_value) = epoch(operand) {
                                        Some(IntOperator::EQ(int_value))
                                    } else {
                                        None
                                    }
                                }
                                StringOperator::NE(operand) => {
                                    if let Ok(int_value) = epoch(operand) {
                                        Some(IntOperator::NE(int_value))
                                    } else {
                                        None
                                    }
                                }
                                StringOperator::LT(operand) => {
                                    if let Ok(int_value) = epoch(operand) {
                                        Some(IntOperator::LT(int_value))
                                    } else {
                                        None
                                    }
                                }
                                StringOperator::GT(operand) => {
                                    if let Ok(int_value) = epoch(operand) {
                                        Some(IntOperator::GT(int_value))
                                    } else {
                                        None
                                    }
                                }
                                StringOperator::LE(operand) => {
                                    if let Ok(int_value) = epoch(operand) {
                                        Some(IntOperator::LE(int_value))
                                    } else {
                                        None
                                    }
                                }
                                StringOperator::GE(operand) => {
                                    if let Ok(int_value) = epoch(operand) {
                                        Some(IntOperator::GE(int_value))
                                    } else {
                                        None
                                    }
                                }
                            };
                            if let Some(oper) = op_oper {
                                check_feasible_int(op_tsd, &oper)
                            } else {
                                true
                            }
                        }
                        _ => {
                            false
                        }
                    }
                } else {
                    true
                }
            }
        }
    }

}

