use serde_json::Value;

pub(crate) fn timestamp_value(value: &Value) -> Result<Option<u64>, String> {
    match value {
        Value::Null => Ok(None),
        Value::Number(number) => number
            .as_u64()
            .or_else(|| {
                number
                    .as_i64()
                    .and_then(|value| (value >= 0).then_some(value as u64))
            })
            .ok_or_else(|| "timestamp must be a non-negative integer".to_string())
            .map(Some),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                return Ok(None);
            }
            if let Ok(timestamp_ms) = text.parse::<u64>() {
                return Ok(Some(timestamp_ms));
            }
            parse_utc_timestamp_ms(text)
                .map(Some)
                .ok_or_else(|| "timestamp must be epoch milliseconds or UTC ISO-8601".to_string())
        }
        _ => Err("timestamp must be epoch milliseconds or UTC ISO-8601".to_string()),
    }
}

fn parse_utc_timestamp_ms(text: &str) -> Option<u64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second_part = time_parts.next()?;
    if time_parts.next().is_some() {
        return None;
    }
    let (second, millis) = parse_second_millis(second_part)?;
    let max_day = days_in_month(year, month)?;
    if day == 0 || day > max_day || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64;
    (seconds >= 0).then_some(seconds as u64 * 1_000 + millis as u64)
}

fn parse_second_millis(value: &str) -> Option<(u32, u32)> {
    let (second, fraction) = value.split_once('.').unwrap_or((value, ""));
    let second = second.parse::<u32>().ok()?;
    let millis = fraction
        .chars()
        .take(3)
        .try_fold(String::new(), |mut acc, character| {
            character.is_ascii_digit().then(|| {
                acc.push(character);
                acc
            })
        })?;
    let millis = if fraction.is_empty() {
        0
    } else {
        format!("{millis:0<3}").parse::<u32>().ok()?
    };
    Some((second, millis))
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - (month <= 2) as i32;
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    (era * 146_097 + day_of_era - 719_468) as i64
}
