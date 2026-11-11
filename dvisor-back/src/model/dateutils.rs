use crate::{error::DvisorResult, throw};

pub fn epoch(ymd: &str) -> DvisorResult<i32> {
    if ymd.len() != 10 || &ymd[4..5] != "-" || &ymd[7..8] != "-" {
        throw!("invalid format, expected YYYY-MM-DD");
    }
    let y: i32 = ymd[0..4].parse().map_err(|_| "invalid year")?;
    let m: u32 = ymd[5..7].parse().map_err(|_| "invalid year")?;
    let d: u32 = ymd[8..10].parse().map_err(|_| "invalid year")?;

    if !(1..=12).contains(&m) { 
        throw!("month out of range"); 
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let mdays = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if leap { 29 } else { 28 },
        _ => throw!("month out of range")
    };
    if d == 0 || d > mdays { throw!("day out of range"); }
    let y_adj = y - if m <= 2 { 1 } else { 0 };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let m_adj = m as i32 + if m > 2 { -3 } else { 9 };
    let doy = (153 * m_adj + 2) / 5 + d as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468)
}

