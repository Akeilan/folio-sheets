//! Date serial numbers in the 1900 system (with its phantom 29 February 1900) and the
//! 1904 system.

/// Days since 1970-01-01 for a proleptic Gregorian date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

const D1900: i64 = -25568; // days_from_civil(1900,1,1) - 1  => serial 1 = 1900-01-01
const D1904: i64 = -24107; // days_from_civil(1904,1,1)       => serial 0 = 1904-01-01

/// (year, month, day) of the integer part of a serial. Serial 0 in the 1900 system is
/// "1900-01-00" and 60 is the phantom 1900-02-29, as the original shows them.
pub fn serial_to_ymd(serial: f64, date1904: bool) -> Option<(i64, u32, u32)> {
    if !serial.is_finite() || serial < 0.0 || serial >= 2958466.0 {
        return None;
    }
    let n = serial.floor() as i64;
    if date1904 {
        return Some(civil_from_days(n + D1904));
    }
    if n == 0 {
        return Some((1900, 1, 0));
    }
    if n == 60 {
        return Some((1900, 2, 29));
    }
    let n = if n > 60 { n - 1 } else { n };
    Some(civil_from_days(n + D1900))
}

/// Serial for a date; months and days may overflow (DATE(2026,14,0) works like the original).
pub fn ymd_to_serial(y: i64, m: i64, d: i64, date1904: bool) -> Option<f64> {
    let mut y = y;
    if (0..1900).contains(&y) && !date1904 {
        y += 1900;
    }
    if date1904 && (0..1904).contains(&y) {
        y += 1900;
    }
    let total_m = y * 12 + (m - 1);
    let yy = total_m.div_euclid(12);
    let mm = total_m.rem_euclid(12) + 1;
    let base = days_from_civil(yy, mm as u32, 1);
    let days = base + d - 1;
    if date1904 {
        let s = days - D1904;
        return if (0..=2957003).contains(&s) { Some(s as f64) } else { None };
    }
    let mut s = days - D1900;
    if s >= 60 {
        s += 1; // after the phantom 29 Feb 1900
    }
    if (0..=2958465).contains(&s) {
        Some(s as f64)
    } else {
        None
    }
}

/// 1 = Sunday ... 7 = Saturday, as WEEKDAY(serial) returns. Serial 1 counts as a Sunday.
pub fn weekday(serial: f64, date1904: bool) -> u32 {
    let n = serial.floor() as i64;
    if date1904 {
        // 1904-01-01 was a Friday
        return ((n + 5).rem_euclid(7) + 1) as u32;
    }
    // the phantom 29 Feb 1900 makes serial 1 a Sunday and keeps every serial after 60 right
    ((n - 1).rem_euclid(7) + 1) as u32
}

pub fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

/// Hours, minutes, seconds of the time part, rounded to whole seconds.
pub fn hms(serial: f64) -> (u32, u32, u32) {
    let frac = serial - serial.floor();
    let secs = (frac * 86400.0 + 0.5e-6).round() as i64;
    let secs = secs.min(86399).max(0) as u32;
    (secs / 3600, (secs / 60) % 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serials() {
        assert_eq!(ymd_to_serial(2026, 1, 2, false), Some(46024.0));
        assert_eq!(serial_to_ymd(46024.0, false), Some((2026, 1, 2)));
        assert_eq!(serial_to_ymd(45000.0, false), Some((2023, 3, 15)));
        assert_eq!(serial_to_ymd(1.0, false), Some((1900, 1, 1)));
        assert_eq!(serial_to_ymd(61.0, false), Some((1900, 3, 1)));
        assert_eq!(ymd_to_serial(1900, 3, 1, false), Some(61.0));
        assert_eq!(weekday(1.0, false), 1);
        assert_eq!(weekday(46024.0, false), 6); // 2 Jan 2026 is a Friday
        assert_eq!(ymd_to_serial(1904, 1, 1, true), Some(0.0));
    }
}
