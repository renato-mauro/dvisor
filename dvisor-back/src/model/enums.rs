use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum SortOrder {
    ASC,
    DESC
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum CalcType {
    INT,
    FLOAT,
    DATE,
    STRING
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::ASC
    }
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum AggFunction {
    FIRST,
    SUM,
    MIN,
    MAX,
    AVG,
    COUNT
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Operator {
    EQ,
    NE,
    LT,
    LE,
    GT,
    GE,
}

impl AggFunction {
    pub fn by_name(name: &str) -> Option<AggFunction> {
        match name.to_ascii_lowercase().as_str() {
            "sum" => Some(AggFunction::SUM),
            "avg" => Some(AggFunction::AVG),
            "min" => Some(AggFunction::MIN),
            "max" => Some(AggFunction::MAX),
            "first" => Some(AggFunction::FIRST),
            "count" => Some(AggFunction::COUNT),
            _ => None,
        }
    }

    pub fn default_field_name(&self, expr: &str) -> String {
        match self {
            AggFunction::FIRST => expr.to_string(),
            AggFunction::SUM => format!("sum_{}",expr),
            AggFunction::MIN => format!("min_{}",expr),
            AggFunction::MAX => format!("max_{}",expr),
            AggFunction::AVG => format!("avg_{}",expr),
            AggFunction::COUNT => "count".to_string(),
        }
    }

    pub fn js_helper_combine(&self, e1: String, e2: String) -> Option<String> {
        match self {
            AggFunction::FIRST => None,
            AggFunction::SUM => Some(format!("{}+={}",e1,e2)),
            AggFunction::MIN => Some(format!("{}={}<{}?{}:{}",e1,e1,e2,e1,e2)),
            AggFunction::MAX => Some(format!("{}={}>{}?{}:{}",e1,e1,e2,e1,e2)),
            AggFunction::AVG => Some(format!("{}=(({}*ca)+({}*cb))/(ca+cb)",e1,e1,e2)),
            AggFunction::COUNT => Some(format!("{}+={}",e1,e2)),
        }
    }
}

