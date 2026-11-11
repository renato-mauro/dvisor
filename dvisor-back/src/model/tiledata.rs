use std::{cmp::Ordering, rc::Rc};
use crate::{err, error::DvisorResult, model::{castfrommacros::{CmpF32, CmpI32}, dateutils::epoch, enums::{AggFunction, Operator, SortOrder}}};

#[derive(Debug)]
pub enum TileData {
    I8(Rc<Vec<i8>>),
    I16(Rc<Vec<i16>>),
    I32(Rc<Vec<i32>>),
    I64(Rc<Vec<i64>>),
    Float(Rc<Vec<f32>>),
    Decimal(Rc<Vec<f32>>,i32),
    Double(Rc<Vec<f64>>),
    String(Rc<Vec<String>>),
    Date(Rc<Vec<i32>>),
}

#[derive(Debug)]
pub enum IntOperator {
    EQ(i32),
    NE(i32),
    LT(i32),
    GT(i32),
    LE(i32),
    GE(i32),
    // Range(i32,i32)
}

#[derive(Debug)]
pub enum FloatOperator {
    EQ(f32),
    NE(f32),
    LT(f32),
    GT(f32),
    LE(f32),
    GE(f32),
    // Range(i32,i32)
}

#[derive(Debug)]
pub enum StringOperator {
    EQ(String),
    NE(String),
    LT(String),
    GT(String),
    LE(String),
    GE(String),
    // Range(i32,i32)
}

fn pack_data<T:Copy>(index: &Vec<usize>, data: &Vec<T>) -> Rc<Vec<T>> {
    let mut resp = Vec::with_capacity(index.len());
    for i in index.iter() {
        resp.push(data[*i]);
    }
    Rc::new(resp)
}

fn pack_data_str(index: &Vec<usize>, data: &Vec<String>) -> Rc<Vec<String>> {
    let mut resp = Vec::with_capacity(index.len());
    for i in index.iter() {
        resp.push(data[*i].clone());
    }
    Rc::new(resp)
}


impl TileData {

    pub fn js_helper_obj(&self, expr: String) -> String {
        match self {
            TileData::Date(_) => format!("new Date(86400000*{})",expr),
            _ => expr
        }
    }

    pub fn js_helper_combine(&self, expr: String) -> String {
        match self {
            TileData::Date(_) => format!("(86400000*{})",expr),
            _ => expr
        }
    }

    pub fn js_helper_calc(&self, expr: String) -> String {
        match self {
            TileData::Date(_) => format!("new Date(86400000*{})",expr),
            _ => expr
        }
    }
    
    pub fn js_helper_test(&self) -> String {
        match self {
            TileData::Date(_) => format!("new Date(138585600000)"),
            TileData::I8(_) => "1".to_string(),
            TileData::I16(_) => "1".to_string(),
            TileData::I32(_) => "1".to_string(),
            TileData::I64(_) => "1".to_string(),
            TileData::Float(_) => "3.1415".to_string(),
            TileData::Decimal(_, _) => "1.99".to_string(),
            TileData::Double(_) => "2.71828".to_string(),
            TileData::String(_) => "'DVISOR'".to_string(),
        }
    }

    pub fn filter_int(&self, oper: &IntOperator) -> DvisorResult<Vec<usize>> {
        match self {
            TileData::I8(data) => Ok(oper.filter(data)),
            TileData::I16(data) => Ok(oper.filter(data)),
            TileData::I32(data) => Ok(oper.filter(data)),
            TileData::I64(data) => Ok(oper.filter(data)),
            TileData::Decimal(data,_) => Ok(oper.filter(data)),
            TileData::Float(data) => Ok(oper.filter(data)),
            TileData::Double(data) => Ok(oper.filter(data)),
            _ => err!("Tile is not int.")
        }
    }

    pub fn filter_index_int(&self, index: &Vec<usize>, oper: &IntOperator) -> DvisorResult<Vec<usize>> {
        match self {
            TileData::I8(data) => Ok(oper.filter_index(index, data)),
            TileData::I16(data) => Ok(oper.filter_index(index, data)),
            TileData::I32(data) => Ok(oper.filter_index(index, data)),
            TileData::I64(data) => Ok(oper.filter_index(index, data)),
            TileData::Decimal(data,_) => Ok(oper.filter_index(index, data)),
            TileData::Float(data) => Ok(oper.filter_index(index, data)),
            TileData::Double(data) => Ok(oper.filter_index(index, data)),
            _ => err!("Tile is not int.")
        }
    }

    pub fn filter_float(&self, oper: &FloatOperator) -> DvisorResult<Vec<usize>> {
        match self {
            TileData::I8(data) => Ok(oper.filter(data)),
            TileData::I16(data) => Ok(oper.filter(data)),
            TileData::I32(data) => Ok(oper.filter(data)),
            TileData::I64(data) => Ok(oper.filter(data)),
            TileData::Decimal(data,_) => Ok(oper.filter(data)),
            TileData::Float(data) => Ok(oper.filter(data)),
            TileData::Double(data) => Ok(oper.filter(data)),
            _ => err!("Tile is not float.")
        }
    }

    pub fn filter_index_float(&self, index: &Vec<usize>, oper: &FloatOperator) -> DvisorResult<Vec<usize>> {
        match self {
            TileData::I8(data) => Ok(oper.filter_index(index, data)),
            TileData::I16(data) => Ok(oper.filter_index(index, data)),
            TileData::I32(data) => Ok(oper.filter_index(index, data)),
            TileData::I64(data) => Ok(oper.filter_index(index, data)),
            TileData::Decimal(data,_) => Ok(oper.filter_index(index, data)),
            TileData::Float(data) => Ok(oper.filter_index(index, data)),
            TileData::Double(data) => Ok(oper.filter_index(index, data)),
            _ => err!("Tile is not int.")
        }
    }

    pub fn filter_string(&self, oper: &StringOperator) -> DvisorResult<Vec<usize>> {
        match self {
            TileData::String(data) => Ok(oper.filter_str(data)),
            TileData::Date(data) => Ok(oper.filter_date(data)?),
            _ => err!("Type mismatch. Expected a String column.")
        }
    }

    pub fn filter_index_string(&self, index: &Vec<usize>, oper: &StringOperator) -> DvisorResult<Vec<usize>> {
        match self {
            TileData::String(data) => Ok(oper.filter_index_str(index,data)),
            TileData::Date(data) => Ok(oper.filter_index_date(index,data)?),
            _ => err!("Type mismatch. Expected a String column.")
        }
    }

    pub fn len(&self) -> usize {
        match self {
            TileData::I8(items) => items.len(),
            TileData::I16(items) => items.len(),
            TileData::I32(items) => items.len(),
            TileData::I64(items) => items.len(),
            TileData::Float(items) => items.len(),
            TileData::Decimal(items,_) => items.len(),
            TileData::Double(items) => items.len(),
            TileData::String(items) => items.len(),
            TileData::Date(items) => items.len(),
        }
    }
    
    pub fn get_comparator(&self, order: SortOrder) -> Box<dyn Fn(&usize, &usize) -> Ordering> {
        macro_rules! cmp_int {
            ($v:expr) => {{
                let data = $v.clone();
                match order {
                    SortOrder::ASC => {
                        Box::new(move |a: &usize, b: &usize| data[*a].cmp(&data[*b]))
                    },
                    SortOrder::DESC => {
                        Box::new(move |a: &usize, b: &usize| data[*b].cmp(&data[*a]))
                    },
                }
            }};
        }
        macro_rules! cmp_float {
            ($v:expr) => {{
                let data = $v.clone();
                match order {
                    SortOrder::ASC => {
                        Box::new(move |a: &usize, b: &usize| data[*a].partial_cmp(&data[*b]).unwrap_or(Ordering::Equal))
                    },
                    SortOrder::DESC => {
                        Box::new(move |a: &usize, b: &usize| data[*b].partial_cmp(&data[*a]).unwrap_or(Ordering::Equal))
                    },
                }
            }};
        }
        macro_rules! cmp_string {
            ($v:expr) => {{
                let data = $v.clone();
                match order {
                    SortOrder::ASC => {
                        Box::new(move |a: &usize, b: &usize| data[*a].cmp(&data[*b]))
                    },
                    SortOrder::DESC => {
                        Box::new(move |a: &usize, b: &usize| data[*b].cmp(&data[*a]))
                    },
                }
            }};
        }
        match self {
            TileData::I8(items) => cmp_int!(items),
            TileData::I16(items) => cmp_int!(items),
            TileData::I32(items) => cmp_int!(items),
            TileData::I64(items) => cmp_int!(items),
            TileData::Float(items) => cmp_float!(items),
            TileData::Decimal(items,_) => cmp_float!(items),
            TileData::Double(items) => cmp_float!(items),
            TileData::String(items) => cmp_string!(items),
            TileData::Date(items) => cmp_int!(items),
        }
    }

    pub fn pack_data(&self, index: &Vec<usize>) -> DvisorResult<TileData> {
        match self {
            TileData::I8(items) => Ok(TileData::I8(pack_data(index, items.as_ref()))),
            TileData::I16(items) => Ok(TileData::I16(pack_data(index, items.as_ref()))),
            TileData::I32(items) => Ok(TileData::I32(pack_data(index, items.as_ref()))),
            TileData::I64(items) => Ok(TileData::I64(pack_data(index, items.as_ref()))),
            TileData::Float(items) => Ok(TileData::Float(pack_data(index, items.as_ref()))),
            TileData::Decimal(items, scale) => Ok(TileData::Decimal(pack_data(index, items.as_ref()),*scale)),
            TileData::Double(items) => Ok(TileData::Double(pack_data(index, items.as_ref()))),
            TileData::String(items) => Ok(TileData::String(pack_data_str(index, items.as_ref()))),
            TileData::Date(items) => Ok(TileData::Date(pack_data(index, items.as_ref()))),
        }
    }

    fn agg_min(&self, index: &Vec<usize>, counts: &Vec<i32>) -> DvisorResult<TileData> {
        macro_rules! minf {
            ($items:expr) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    let mut min = $items[index[i]];
                    i += 1;
                    for _ in 1..*c {
                        let e = $items[index[i]];
                        i += 1;
                        if e < min {
                            min = e;
                        }
                    }
                    resp.push(min); 
                }
                Rc::new(resp)
            }};
        }
        macro_rules! minf_str {
            ($items:expr) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    let mut min = &$items[index[i]];
                    i += 1;
                    for _ in 1..*c {
                        let e = &$items[index[i]];
                        i += 1;
                        if e < min {
                            min = e;
                        }
                    }
                    resp.push(min.clone()); 
                }
                Rc::new(resp)
            }};
        }
        match self {
            TileData::I8(items) => Ok(TileData::I8(minf!(items))),
            TileData::I16(items) => Ok(TileData::I16(minf!(items))),
            TileData::I32(items) => Ok(TileData::I32(minf!(items))),
            TileData::I64(items) => Ok(TileData::I64(minf!(items))),
            TileData::Float(items) => Ok(TileData::Float(minf!(items))),
            TileData::Decimal(items,scale) => Ok(TileData::Decimal(minf!(items),*scale)),
            TileData::Double(items) => Ok(TileData::Double(minf!(items))),
            TileData::String(items) => Ok(TileData::String(minf_str!(items))),
            TileData::Date(items) => Ok(TileData::Date(minf_str!(items))),
        }
    }

    fn agg_max(&self, index: &Vec<usize>, counts: &Vec<i32>) -> DvisorResult<TileData> {
        macro_rules! maxf {
            ($items:expr) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    let mut max = $items[index[i]];
                    i += 1;
                    for _ in 1..*c {
                        let e = $items[index[i]];
                        i += 1;
                        if e > max {
                            max = e;
                        }
                    }
                    resp.push(max); 
                }
                Rc::new(resp)
            }};
        }
        macro_rules! maxf_str {
            ($items:expr) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    let mut max = &$items[index[i]];
                    i += 1;
                    for _ in 1..*c {
                        let e = &$items[index[i]];
                        i += 1;
                        if e > max {
                            max = e;
                        }
                    }
                    resp.push(max.clone()); 
                }
                Rc::new(resp)
            }};
        }
        match self {
            TileData::I8(items) => Ok(TileData::I8(maxf!(items))),
            TileData::I16(items) => Ok(TileData::I16(maxf!(items))),
            TileData::I32(items) => Ok(TileData::I32(maxf!(items))),
            TileData::I64(items) => Ok(TileData::I64(maxf!(items))),
            TileData::Float(items) => Ok(TileData::Float(maxf!(items))),
            TileData::Decimal(items,scale) => Ok(TileData::Decimal(maxf!(items),*scale)),
            TileData::Double(items) => Ok(TileData::Double(maxf!(items))),
            TileData::String(items) => Ok(TileData::String(maxf_str!(items))),
            TileData::Date(items) => Ok(TileData::Date(maxf!(items))),
        }
    }

    // Sums are accumulated in 64 bits, integers in i64 and floating-point
    // values in f64, whatever the column's storage type: a column stored in
    // 8 or 16 bits (see Statistics::get_pref_type) would overflow, and an f32
    // accumulator drops digits. The result is always a Double, so every
    // fragment sends a sum as a JavaScript number, whatever its storage type.
    fn agg_sum(&self, index: &Vec<usize>, counts: &Vec<i32>) -> DvisorResult<TileData> {
        macro_rules! sumf {
            ($items:expr,$acc:ty) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    let mut sum = $items[index[i]] as $acc;
                    i += 1;
                    for _ in 1..*c {
                        sum += $items[index[i]] as $acc;
                        i += 1;
                    }
                    resp.push(sum as f64);
                }
                Rc::new(resp)
            }};
        }
        match self {
            TileData::I8(items) => Ok(TileData::Double(sumf!(items,i64))),
            TileData::I16(items) => Ok(TileData::Double(sumf!(items,i64))),
            TileData::I32(items) => Ok(TileData::Double(sumf!(items,i64))),
            TileData::I64(items) => Ok(TileData::Double(sumf!(items,i64))),
            TileData::Float(items) => Ok(TileData::Double(sumf!(items,f64))),
            TileData::Decimal(items,_) => Ok(TileData::Double(sumf!(items,f64))),
            TileData::Double(items) => Ok(TileData::Double(sumf!(items,f64))),
            TileData::String(_) => err!("SUM cannot be computed on Strings"),
            TileData::Date(_) => err!("SUM cannot be computed on Date")
        }
    }

    // Accumulated like agg_sum, then divided by the group's count.
    fn agg_avg(&self, index: &Vec<usize>, counts: &Vec<i32>) -> DvisorResult<TileData> {
        macro_rules! avgf {
            ($items:expr,$acc:ty) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    let mut sum = $items[index[i]] as $acc;
                    i += 1;
                    for _ in 1..*c {
                        sum += $items[index[i]] as $acc;
                        i += 1;
                    }
                    resp.push(sum as f64 / *c as f64);
                }
                Rc::new(resp)
            }};
        }
        match self {
            TileData::I8(items) => Ok(TileData::Double(avgf!(items,i64))),
            TileData::I16(items) => Ok(TileData::Double(avgf!(items,i64))),
            TileData::I32(items) => Ok(TileData::Double(avgf!(items,i64))),
            TileData::I64(items) => Ok(TileData::Double(avgf!(items,i64))),
            TileData::Float(items) => Ok(TileData::Double(avgf!(items,f64))),
            TileData::Decimal(items,_) => Ok(TileData::Double(avgf!(items,f64))),
            TileData::Double(items) => Ok(TileData::Double(avgf!(items,f64))),
            TileData::String(_) => err!("AVG cannot be computed on Strings"),
            TileData::Date(_) => err!("AVG cannot be computed on Date")
        }
    }

    fn agg_first(&self, index: &Vec<usize>, counts: &Vec<i32>) -> DvisorResult<TileData> {
        macro_rules! firstf {
            ($items:expr) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    resp.push($items[index[i]]); 
                    i += *c as usize;
                }
                Rc::new(resp)
            }};
        }
        macro_rules! firstf_str {
            ($items:expr) => {{
                let mut resp = Vec::with_capacity(counts.len());
                let mut i = 0;
                for c in counts {
                    resp.push($items[index[i]].clone()); 
                    i += *c as usize;
                }
                Rc::new(resp)
            }};
        }
        match self {
            TileData::I8(items) => Ok(TileData::I8(firstf!(items))),
            TileData::I16(items) => Ok(TileData::I16(firstf!(items))),
            TileData::I32(items) => Ok(TileData::I32(firstf!(items))),
            TileData::I64(items) => Ok(TileData::I64(firstf!(items))),
            TileData::Float(items) => Ok(TileData::Float(firstf!(items))),
            TileData::Decimal(items,scale) => Ok(TileData::Decimal(firstf!(items),*scale)),
            TileData::Double(items) => Ok(TileData::Double(firstf!(items))),
            TileData::String(items) => Ok(TileData::String(firstf_str!(items))),
            TileData::Date(items) => Ok(TileData::Date(firstf!(items))),
        }
    }

    pub fn aggregate(&self, index: &Vec<usize>, counts: &Rc<Vec<i32>>, function: AggFunction) -> DvisorResult<TileData> {
        match function {
            AggFunction::FIRST => self.agg_first(index, counts),
            AggFunction::SUM => self.agg_sum(index, counts),
            AggFunction::MIN => self.agg_min(index, counts),
            AggFunction::MAX => self.agg_max(index, counts),
            AggFunction::AVG => self.agg_avg(index, counts),
            AggFunction::COUNT => Ok(TileData::I32(counts.clone()))
        }
    }

}

impl IntOperator {

    pub fn new(oper: Operator, value: i32) -> DvisorResult<IntOperator> {
        match oper {
            Operator::EQ => Ok(IntOperator::EQ(value)),
            Operator::NE => Ok(IntOperator::NE(value)),
            Operator::LT => Ok(IntOperator::LT(value)),
            Operator::LE => Ok(IntOperator::LE(value)),
            Operator::GT => Ok(IntOperator::GT(value)),
            Operator::GE => Ok(IntOperator::GE(value)),
        }
    }

    pub fn filter<T>(&self, data: &Vec<T>) -> Vec<usize>
    where 
        T: Copy + CmpI32 
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in 0..data.len() {
                    if data[i].$op ($rhs) {
                        out.push(i);
                    }
                }
                out
            }};
        }
        match self {
            IntOperator::EQ(value) => filter!(eq, *value),
            IntOperator::NE(value) => filter!(ne, *value),
            IntOperator::LT(value) => filter!(lt, *value),
            IntOperator::GT(value) => filter!(gt, *value),
            IntOperator::LE(value) => filter!(le, *value),
            IntOperator::GE(value) => filter!(ge, *value),
        }
    }

    pub fn filter_index<T>(&self, index: &Vec<usize>, data: &Vec<T>) -> Vec<usize>
    where 
        T: Copy + CmpI32
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in index {
                    if data[*i].$op($rhs) {
                        out.push(*i);
                    }
                }
                out
            }};
        }
        match self {
            IntOperator::EQ(value) => filter!(eq, *value),
            IntOperator::NE(value) => filter!(ne, *value),
            IntOperator::LT(value) => filter!(lt, *value),
            IntOperator::GT(value) => filter!(gt, *value),
            IntOperator::LE(value) => filter!(le, *value),
            IntOperator::GE(value) => filter!(ge, *value),
        }
    }

}

impl FloatOperator {

    pub fn new(oper: Operator, value: f32) -> DvisorResult<FloatOperator> {
        match oper {
            Operator::EQ => Ok(FloatOperator::EQ(value)),
            Operator::NE => Ok(FloatOperator::NE(value)),
            Operator::LT => Ok(FloatOperator::LT(value)),
            Operator::LE => Ok(FloatOperator::LE(value)),
            Operator::GT => Ok(FloatOperator::GT(value)),
            Operator::GE => Ok(FloatOperator::GE(value)),
        }
    }

    pub fn filter<T>(&self, data: &Vec<T>) -> Vec<usize>
    where 
        T: Copy + CmpF32 
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in 0..data.len() {
                    if data[i].$op ($rhs) {
                        out.push(i);
                    }
                }
                out
            }};
        }
        match self {
            FloatOperator::EQ(value) => filter!(eq, *value),
            FloatOperator::NE(value) => filter!(ne, *value),
            FloatOperator::LT(value) => filter!(lt, *value),
            FloatOperator::GT(value) => filter!(gt, *value),
            FloatOperator::LE(value) => filter!(le, *value),
            FloatOperator::GE(value) => filter!(ge, *value),
        }
    }

    pub fn filter_index<T>(&self, index: &Vec<usize>, data: &Vec<T>) -> Vec<usize>
    where 
        T: Copy + CmpF32
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in index {
                    if data[*i].$op($rhs) {
                        out.push(*i);
                    }
                }
                out
            }};
        }
        match self {
            FloatOperator::EQ(value) => filter!(eq, *value),
            FloatOperator::NE(value) => filter!(ne, *value),
            FloatOperator::LT(value) => filter!(lt, *value),
            FloatOperator::GT(value) => filter!(gt, *value),
            FloatOperator::LE(value) => filter!(le, *value),
            FloatOperator::GE(value) => filter!(ge, *value),
        }
    }

}

impl StringOperator {

    pub fn new(oper: Operator, value: String) -> DvisorResult<StringOperator> {
        match oper {
            Operator::EQ => Ok(StringOperator::EQ(value)),
            Operator::NE => Ok(StringOperator::NE(value)),
            Operator::LT => Ok(StringOperator::LT(value)),
            Operator::LE => Ok(StringOperator::LE(value)),
            Operator::GT => Ok(StringOperator::GT(value)),
            Operator::GE => Ok(StringOperator::GE(value)),
        }
    }

    pub fn filter_str(&self, data: &Vec<String>) -> Vec<usize>
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in 0..data.len() {
                    if data[i] $op $rhs {
                        out.push(i);
                    }
                }
                out
            }};
        }
        match self {
            StringOperator::EQ(value) => filter!(==, *value),
            StringOperator::NE(value) => filter!(!=, *value),
            StringOperator::LT(value) => filter!(<, *value),
            StringOperator::GT(value) => filter!(>, *value),
            StringOperator::LE(value) => filter!(<=, *value),
            StringOperator::GE(value) => filter!(>=, *value),
        }
    }

    pub fn filter_index_str(&self, index: &Vec<usize>, data: &Vec<String>) -> Vec<usize>
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in index {
                    if data[*i] $op $rhs {
                        out.push(*i);
                    }
                }
                out
            }};
        }
        match self {
            StringOperator::EQ(value) => filter!(==, *value),
            StringOperator::NE(value) => filter!(!=, *value),
            StringOperator::LT(value) => filter!(<, *value),
            StringOperator::GT(value) => filter!(>, *value),
            StringOperator::LE(value) => filter!(<=, *value),
            StringOperator::GE(value) => filter!(>=, *value),
        }
    }

    pub fn filter_date(&self, data: &Rc<Vec<i32>>) -> DvisorResult<Vec<usize>>
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in 0..data.len() {
                    if data[i] $op $rhs {
                        out.push(i);
                    }
                }
                Ok(out)
            }};
        }
        match self {
            StringOperator::EQ(value) => filter!(==, epoch(value)?),
            StringOperator::NE(value) => filter!(!=, epoch(value)?),
            StringOperator::LT(value) => filter!(<, epoch(value)?),
            StringOperator::GT(value) => filter!(>, epoch(value)?),
            StringOperator::LE(value) => filter!(<=, epoch(value)?),
            StringOperator::GE(value) => filter!(>=, epoch(value)?),
        }
    }

    pub fn filter_index_date(&self, index: &Vec<usize>, data: &Rc<Vec<i32>>) -> DvisorResult<Vec<usize>>
    {
        macro_rules! filter {
            ($op:tt, $rhs:expr) => {{
                let mut out = Vec::with_capacity(data.len());
                for i in index {
                    if data[*i] $op $rhs {
                        out.push(*i);
                    }
                }
                Ok(out)
            }};
        }
        match self {
            StringOperator::EQ(value) => filter!(==, epoch(value)?),
            StringOperator::NE(value) => filter!(!=, epoch(value)?),
            StringOperator::LT(value) => filter!(<, epoch(value)?),
            StringOperator::GT(value) => filter!(>, epoch(value)?),
            StringOperator::LE(value) => filter!(<=, epoch(value)?),
            StringOperator::GE(value) => filter!(>=, epoch(value)?),
        }
    }

}


#[cfg(test)]
mod tests {
    use super::*;

    // The values of an aggregate, which must be a Double.
    fn doubles(td: TileData) -> Vec<f64> {
        match td {
            TileData::Double(items) => items.to_vec(),
            other => panic!("expected a Double, got {:?}", other),
        }
    }

    #[test]
    fn sum_of_narrow_integers_does_not_overflow() {
        // Two groups, the rows taken out of order as a sort leaves them.
        let index = vec![3, 0, 1, 2, 4];
        let counts = vec![3, 2];
        let i8s = TileData::I8(Rc::new(vec![100, 100, 120, 100, 127]));
        assert_eq!(doubles(i8s.aggregate(&index, &Rc::new(counts.clone()), AggFunction::SUM).unwrap()), vec![300.0, 247.0]);
        let i16s = TileData::I16(Rc::new(vec![30000; 5]));
        assert_eq!(doubles(i16s.aggregate(&index, &Rc::new(counts.clone()), AggFunction::SUM).unwrap()), vec![90000.0, 60000.0]);
        let i32s = TileData::I32(Rc::new(vec![2_000_000_000; 5]));
        assert_eq!(doubles(i32s.aggregate(&index, &Rc::new(counts), AggFunction::SUM).unwrap()), vec![6e9, 4e9]);
    }

    #[test]
    fn sum_and_avg_of_floats_keep_their_digits() {
        // 2^24 + 1 + 1: an f32 accumulator stays at 2^24.
        let index = vec![0, 1, 2];
        let counts = Rc::new(vec![3]);
        let floats = TileData::Float(Rc::new(vec![16_777_216.0, 1.0, 1.0]));
        assert_eq!(doubles(floats.aggregate(&index, &counts, AggFunction::SUM).unwrap()), vec![16_777_218.0]);
        assert_eq!(doubles(floats.aggregate(&index, &counts, AggFunction::AVG).unwrap()), vec![16_777_218.0 / 3.0]);
        let decimals = TileData::Decimal(Rc::new(vec![16_777_216.0, 1.0, 1.0]), 2);
        assert_eq!(doubles(decimals.aggregate(&index, &counts, AggFunction::SUM).unwrap()), vec![16_777_218.0]);
    }

    #[test]
    fn avg_of_integers_is_exact() {
        let index = vec![0, 1, 2];
        let counts = Rc::new(vec![3]);
        let i32s = TileData::I32(Rc::new(vec![16_777_217, 0, 1]));
        assert_eq!(doubles(i32s.aggregate(&index, &counts, AggFunction::AVG).unwrap()), vec![16_777_218.0 / 3.0]);
    }

    #[test]
    fn sums_and_averages_are_doubles_whatever_the_storage_type() {
        // So every fragment sends them the same way, whichever type each one
        // stores the column in.
        let index = vec![0];
        let counts = Rc::new(vec![1]);
        let columns = [
            TileData::I8(Rc::new(vec![1])),
            TileData::I16(Rc::new(vec![1])),
            TileData::I32(Rc::new(vec![1])),
            TileData::I64(Rc::new(vec![1])),
            TileData::Float(Rc::new(vec![1.0])),
            TileData::Decimal(Rc::new(vec![1.0]), 2),
            TileData::Double(Rc::new(vec![1.0])),
        ];
        for td in &columns {
            for f in [AggFunction::SUM, AggFunction::AVG] {
                assert_eq!(doubles(td.aggregate(&index, &counts, f).unwrap()), vec![1.0], "{:?} of {:?}", f, td);
            }
        }
        let dates = TileData::Date(Rc::new(vec![1]));
        assert!(dates.aggregate(&index, &counts, AggFunction::SUM).is_err());
    }
}
