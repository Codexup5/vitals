//! The stylesheet of the window: compiled from SCSS by `grass`, and linted by the rules the kit's stylelint held:
//! colours, spacing, sizes and radii come from the tokens, a line height has no unit.

use std::fs;
use std::path::Path;

/// Properties whose length must be a token; 1–3 px is an optical correction and may be a number
const SPACING_PROPERTIES: &[&str] = &[
    "margin",
    "padding",
    "gap",
    "row-gap",
    "column-gap",
    "font-size",
    "border-radius",
    "inset",
];

/// Compiles `styles/main.scss` into one CSS file.
pub fn compile(main: &Path) -> Result<String, String> {
    grass::from_path(
        main,
        &grass::Options::default().style(grass::OutputStyle::Compressed),
    )
    .map_err(|error| error.to_string())
}

/// A length a token should hold: 4 px and up, or any `em`/`rem`
fn is_raw_size(value: &str) -> bool {
    value
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '.'))
        .any(|word| {
            let number_end = word
                .find(|character: char| !(character.is_ascii_digit() || character == '.'))
                .unwrap_or(word.len());
            let (number, unit) = word.split_at(number_end);
            let Ok(number) = number.parse::<f64>() else {
                return false;
            };
            match unit {
                "px" => number >= 4.0,
                "em" | "rem" => true,
                _ => false,
            }
        })
}

fn has_raw_colour(value: &str) -> bool {
    let lower = value.to_lowercase();
    let has_hex = lower.match_indices('#').any(|(index, _)| {
        let digits = lower[index + 1..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .count();
        // `#{$kind}` is SCSS interpolation, not a colour
        matches!(digits, 3 | 4 | 6 | 8) && !lower[index + 1..].starts_with('{')
    });
    let has_function = ["rgb(", "rgba(", "hsl(", "hsla("]
        .iter()
        .any(|function| lower.contains(function));
    has_hex || has_function
}

/// The findings of one SCSS file, as `file:line: message`.
pub fn lint(path: &Path, allows_important: bool) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let mut findings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.split("//").next().unwrap_or_default().trim();
        let Some((property, value)) = line.split_once(':') else {
            continue;
        };
        let property = property.trim();
        if property.starts_with('$')
            || property.starts_with('&')
            || property.starts_with('@')
            || property.contains(' ')
        {
            continue;
        }
        let at = format!("{name}:{}", index + 1);
        if has_raw_colour(value) {
            findings.push(format!("{at}: a colour is a token only, `var(--color-…)`"));
        }
        let is_spacing = SPACING_PROPERTIES
            .iter()
            .any(|spacing| property == *spacing || property.starts_with(&format!("{spacing}-")));
        if is_spacing && is_raw_size(value) {
            findings.push(format!(
                "{at}: spacing, font size and radius are tokens only; 1–3 px is an optical correction"
            ));
        }
        if property == "line-height" && is_raw_size(&value.replace("var(", "")) {
            findings.push(format!(
                "{at}: a line height is unitless, a number or a token (design/lh-unitless)"
            ));
        }
        if value.contains("!important") && !allows_important {
            findings.push(format!(
                "{at}: `!important` lives only in the reduced-motion block of _base.scss"
            ));
        }
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raw_size_is_four_pixels_and_up_or_any_em() {
        assert!(is_raw_size(" 12px;"));
        assert!(is_raw_size(" 0 1.5rem;"));
        assert!(!is_raw_size(" var(--space-4);"));
        assert!(!is_raw_size(" 2px solid var(--color-focus);"));
    }

    #[test]
    fn a_colour_is_a_hex_or_a_colour_function_not_an_interpolation() {
        assert!(has_raw_colour(" #fff;"));
        assert!(has_raw_colour(" rgba(0, 0, 0, 0.5);"));
        assert!(!has_raw_colour(" var(--color-chart-#{$token});"));
        assert!(!has_raw_colour(" var(--color-text);"));
    }
}
