use crate::model::pitch::Pitch;
use crate::model::scale::Scale;
use std::str::FromStr;

/// Shift the `key:` directive's pitch by `delta` semitones. If there is no
/// `key:` line, insert `key: <C shifted by delta>` at the top.
pub fn shift_key(text: &str, delta: i8) -> String {
    let shift = |p: Pitch| -> Pitch {
        let n = (p as i8 + delta).rem_euclid(12) as u8;
        Pitch::try_from(n).unwrap()
    };
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if let Some(line) = lines.iter_mut().find(|l| l.trim_start().starts_with("key:")) {
        if let Some((_, rest)) = line.split_once("key:") {
            if let Ok(p) = Pitch::from_str(rest.trim()) {
                *line = format!("key: {}", shift(p));
            }
        }
        lines.join("\n")
    } else {
        let mut out = vec![format!("key: {}", shift(Pitch::C))];
        out.extend(lines);
        out.join("\n")
    }
}

/// Set (or insert) the `scale:` directive. If absent, insert right after the
/// `key:` line (or at the top if there is no key line).
pub fn set_scale(text: &str, scale: Scale) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if let Some(line) = lines.iter_mut().find(|l| l.trim_start().starts_with("scale:")) {
        *line = format!("scale: {}", scale);
        lines.join("\n")
    } else {
        let insert_at = lines
            .iter()
            .position(|l| l.trim_start().starts_with("key:"))
            .map(|i| i + 1)
            .unwrap_or(0);
        lines.insert(insert_at, format!("scale: {}", scale));
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_key_existing() {
        assert_eq!(shift_key("key: C\nI |", 1), "key: C#\nI |");
        assert_eq!(shift_key("key: C\nI |", -1), "key: B\nI |");
        assert_eq!(shift_key("key: B\nI |", 1), "key: C\nI |");
    }

    #[test]
    fn test_shift_key_absent_inserts() {
        assert_eq!(shift_key("I | IV |", 2), "key: D\nI | IV |");
    }

    #[test]
    fn test_set_scale_existing() {
        assert_eq!(set_scale("key: C\nscale: major\nI |", Scale::Dorian), "key: C\nscale: dorian\nI |");
    }

    #[test]
    fn test_set_scale_absent_after_key() {
        assert_eq!(set_scale("key: C\nI |", Scale::Lydian), "key: C\nscale: lydian\nI |");
    }
}
