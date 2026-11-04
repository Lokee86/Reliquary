pub(crate) const WORD_NUMBER_RE: &str = "(?:one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|thirty|forty|fifty|sixty|seventy|eighty|ninety)(?:[-\\s]+(?:one|two|three|four|five|six|seven|eight|nine))?";

pub(crate) fn parse_number(value: &str) -> Option<i64> {
    if let Ok(number) = value.parse::<i64>() {
        return Some(number);
    }

    let normalized = value.to_ascii_lowercase().replace('-', " ");
    let mut parts = normalized.split_whitespace();
    let first = parts.next()?;
    let second = parts.next();
    if parts.next().is_some() {
        return None;
    }

    let first_value = word_value(first)?;
    match second {
        None => Some(first_value),
        Some(second) if first_value >= 20 && first_value % 10 == 0 => {
            let second_value = word_value(second)?;
            (second_value < 10).then_some(first_value + second_value)
        }
        Some(_) => None,
    }
}

fn word_value(value: &str) -> Option<i64> {
    match value {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        "eleven" => Some(11),
        "twelve" => Some(12),
        "thirteen" => Some(13),
        "fourteen" => Some(14),
        "fifteen" => Some(15),
        "sixteen" => Some(16),
        "seventeen" => Some(17),
        "eighteen" => Some(18),
        "nineteen" => Some(19),
        "twenty" => Some(20),
        "thirty" => Some(30),
        "forty" => Some(40),
        "fifty" => Some(50),
        "sixty" => Some(60),
        "seventy" => Some(70),
        "eighty" => Some(80),
        "ninety" => Some(90),
        _ => None,
    }
}
