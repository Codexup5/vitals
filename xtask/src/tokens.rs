//! The design tokens: DTCG 2025.10 data in `crates/vitals-ui/tokens`, built into `styles/_tokens.scss` and judged by
//! the design rules of the user's canon. A port of the kit's `tooling/design`: the same rules, ids and bars.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value};

const ROOT_FONT_SIZE: f64 = 16.0;
const GRID: f64 = 4.0;
const GRID_TOLERANCE: f64 = 0.01;
const TARGET_FINE: f64 = 24.0;
const TARGET_COARSE: f64 = 44.0;
const CONTRAST_TEXT: f64 = 4.5;
const CONTRAST_LARGE: f64 = 3.0;
const CONTRAST_UI: f64 = 3.0;

/// One token after its group's type and role were handed down to it
#[derive(Debug, Clone)]
struct Token {
    path: Vec<String>,
    kind: String,
    value: Value,
    role: Option<String>,
    kit: Map<String, Value>,
}

impl Token {
    fn name(&self) -> String {
        self.path.join("-")
    }

    fn dotted(&self) -> String {
        self.path.join(".")
    }

    fn px(&self) -> Option<f64> {
        let value = self.value.get("value")?.as_f64()?;
        match self.value.get("unit")?.as_str()? {
            "rem" => Some(value * ROOT_FONT_SIZE),
            "px" => Some(value),
            _ => None,
        }
    }

    fn hex(&self) -> Option<&str> {
        self.value.get("hex")?.as_str()
    }
}

#[derive(Debug)]
struct Layer {
    file: String,
    /// The CSS that wraps the layer: none for the base, `:root[data-theme='light']`, `@media (…)` then `:root`
    selector: Vec<String>,
    theme: Option<String>,
    is_coarse: bool,
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

fn collect(
    group: &Map<String, Value>,
    path: &[String],
    kind: Option<&str>,
    kit: &Map<String, Value>,
    out: &mut Vec<Token>,
) {
    let kind = group.get("$type").and_then(Value::as_str).or(kind);
    let mut kit = kit.clone();
    if let Some(Value::Object(own)) = group
        .get("$extensions")
        .and_then(|extensions| extensions.get("kit"))
    {
        kit.extend(own.clone());
    }
    if let Some(value) = group.get("$value") {
        out.push(Token {
            path: path.to_vec(),
            kind: kind.unwrap_or_default().to_owned(),
            value: value.clone(),
            role: kit.get("role").and_then(Value::as_str).map(str::to_owned),
            kit: kit.clone(),
        });
        return;
    }
    for (key, child) in group {
        if key.starts_with('$') {
            continue;
        }
        if let Value::Object(child) = child {
            let mut child_path = path.to_vec();
            child_path.push(key.clone());
            // A role or a list of surfaces belongs to the token that states it, and to its children
            collect(child, &child_path, kind, &kit, out);
        }
    }
}

fn read_layers(dir: &Path) -> Result<Vec<(Layer, Vec<Token>)>, String> {
    let config = read_json(&dir.join("config.json"))?;
    let layers = config
        .get("layers")
        .and_then(Value::as_array)
        .ok_or("config.json has no layers")?;
    layers
        .iter()
        .map(|layer| {
            let file = layer
                .get("file")
                .and_then(Value::as_str)
                .ok_or("a layer has no file")?
                .to_owned();
            let selector = layer
                .get("selector")
                .and_then(Value::as_array)
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let Value::Object(data) = read_json(&dir.join(&file))? else {
                return Err(format!("{file} is not an object"));
            };
            let mut tokens = Vec::new();
            collect(&data, &[], None, &Map::new(), &mut tokens);
            Ok((
                Layer {
                    file,
                    selector,
                    theme: layer
                        .get("theme")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    is_coarse: layer.get("pointer").and_then(Value::as_str) == Some("coarse"),
                },
                tokens,
            ))
        })
        .collect()
}

/// `0.8125` rather than `0.8125000000000001` or `1.0`
fn number(value: f64) -> String {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_owned()
}

/// `#fff` for `#ffffff`, the hex as it is otherwise
fn short_hex(hex: &str) -> String {
    let lower = hex.to_lowercase();
    let bytes = lower.as_bytes();
    if bytes.len() == 7 && bytes[1] == bytes[2] && bytes[3] == bytes[4] && bytes[5] == bytes[6] {
        format!("#{}{}{}", &lower[1..2], &lower[3..4], &lower[5..6])
    } else {
        lower
    }
}

fn css_value(token: &Token) -> Result<String, String> {
    let fail = || {
        format!(
            "{} has a {} value the build cannot read",
            token.dotted(),
            token.kind
        )
    };
    match token.kind.as_str() {
        "color" => token.hex().map(short_hex).ok_or_else(fail),
        "dimension" => {
            let value = token
                .value
                .get("value")
                .and_then(Value::as_f64)
                .ok_or_else(fail)?;
            let unit = token
                .value
                .get("unit")
                .and_then(Value::as_str)
                .ok_or_else(fail)?;
            if token.role.as_deref() == Some("font-size") && unit == "px" {
                Ok(format!("{}rem", number(value / ROOT_FONT_SIZE)))
            } else {
                Ok(format!("{}{unit}", number(value)))
            }
        }
        "fontFamily" => {
            let names = token.value.as_array().ok_or_else(fail)?;
            let names: Vec<String> = names
                .iter()
                .filter_map(Value::as_str)
                .map(|name| {
                    if name.contains(' ') {
                        format!("'{name}'")
                    } else {
                        name.to_owned()
                    }
                })
                .collect();
            Ok(names.join(", "))
        }
        "fontWeight" | "number" => token.value.as_f64().map(number).ok_or_else(fail),
        _ => Err(fail()),
    }
}

/// The text of `_tokens.scss` built from the data.
pub fn build(dir: &Path) -> Result<String, String> {
    let mut out = String::from(
        "// Generated by `cargo xtask tokens` from crates/vitals-ui/tokens: edit the tokens, then run the task.\n",
    );
    for (index, (layer, tokens)) in read_layers(dir)?.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let selectors: Vec<&str> = if layer.selector.is_empty() {
            vec![":root"]
        } else {
            layer.selector.iter().map(String::as_str).collect()
        };
        for (depth, selector) in selectors.iter().enumerate() {
            let _ = writeln!(out, "{}{selector} {{", "  ".repeat(depth));
        }
        let indent = "  ".repeat(selectors.len());
        for token in tokens {
            let _ = writeln!(out, "{indent}--{}: {};", token.name(), css_value(token)?);
        }
        for depth in (0..selectors.len()).rev() {
            let _ = writeln!(out, "{}}}", "  ".repeat(depth));
        }
    }
    Ok(out)
}

/// A context the rules judge: the base with one theme or pointer layer laid over it
struct Context {
    label: String,
    is_theme: bool,
    is_coarse: bool,
    tokens: BTreeMap<String, Token>,
}

fn contexts(dir: &Path) -> Result<Vec<Context>, String> {
    let layers = read_layers(dir)?;
    let Some((base_layer, base)) = layers.first() else {
        return Err("config.json lists no layer".to_owned());
    };
    let base: BTreeMap<String, Token> = base
        .iter()
        .map(|token| (token.dotted(), token.clone()))
        .collect();
    let mut contexts = vec![Context {
        label: base_layer
            .theme
            .clone()
            .unwrap_or_else(|| base_layer.file.clone()),
        is_theme: true,
        is_coarse: false,
        tokens: base.clone(),
    }];
    for (layer, tokens) in &layers[1..] {
        let mut merged = base.clone();
        for token in tokens {
            // A layer changes values; the role and the surfaces stay those of the base
            let entry = merged
                .entry(token.dotted())
                .or_insert_with(|| token.clone());
            entry.value = token.value.clone();
        }
        contexts.push(Context {
            label: layer.theme.clone().unwrap_or_else(|| layer.file.clone()),
            is_theme: layer.theme.is_some(),
            is_coarse: layer.is_coarse,
            tokens: merged,
        });
    }
    Ok(contexts)
}

fn luminance(hex: &str) -> Option<f64> {
    let channel = |range: std::ops::Range<usize>| -> Option<f64> {
        let value = f64::from(u8::from_str_radix(hex.get(range)?, 16).ok()?) / 255.0;
        Some(if value <= 0.040_45 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        })
    };
    Some(0.2126 * channel(1..3)? + 0.7152 * channel(3..5)? + 0.0722 * channel(5..7)?)
}

/// WCAG 2.2 contrast ratio of two `#rrggbb` colours
pub fn contrast(left: &str, right: &str) -> Option<f64> {
    let (left, right) = (luminance(left)?, luminance(right)?);
    let (light, dark) = if left > right {
        (left, right)
    } else {
        (right, left)
    };
    Some((light + 0.05) / (dark + 0.05))
}

fn on_grid(px: f64) -> bool {
    (px / GRID - (px / GRID).round()).abs() * GRID <= GRID_TOLERANCE
}

fn floor2(value: f64) -> f64 {
    (value * 100.0).floor() / 100.0
}

fn check_context(context: &Context, findings: &mut Vec<String>) {
    let label = &context.label;
    let has_role = |token: &&Token, role: &str| token.role.as_deref() == Some(role);
    for token in context.tokens.values() {
        let name = token.dotted();
        let px = token.px();
        if ["space", "size", "target"]
            .iter()
            .any(|role| has_role(&token, role))
            && let Some(px) = px
            && !on_grid(px)
            && !(1.0..=2.0).contains(&px)
        {
            findings.push(format!(
                "violation design/grid — {name}: {px} px is not a multiple of {GRID} px"
            ));
        }
        if has_role(&token, "line-height") {
            if token.kind != "number" {
                findings.push(format!(
                    "violation design/lh-unitless — {name}: {} instead of a number",
                    token.kind
                ));
            } else if let (Some(height), Some(size)) = (
                token.value.as_f64(),
                token
                    .kit
                    .get("fontSize")
                    .and_then(Value::as_str)
                    .and_then(|alias| context.tokens.get(alias.trim_matches(['{', '}'])))
                    .and_then(Token::px),
            ) && !on_grid(size * height)
                && token.kit.get("oneLine").and_then(Value::as_bool) != Some(true)
            {
                findings.push(format!(
                    "violation design/lh-grid — {name}: {size} px × {height} = {} px, not a multiple of {GRID} px",
                    size * height
                ));
            }
        }
        if has_role(&token, "target")
            && let Some(px) = px
        {
            if context.is_theme && px < TARGET_FINE {
                findings.push(format!(
                    "violation design/target-fine — {name} ({label}): {px} px < {TARGET_FINE} px"
                ));
            }
            if context.is_coarse && px < TARGET_COARSE {
                findings.push(format!(
                    "violation design/target-coarse — {name} ({label}): {px} px < {TARGET_COARSE} px"
                ));
            }
        }
    }
    if !context.is_theme {
        return;
    }
    let surfaces_all: Vec<&Token> = context
        .tokens
        .values()
        .filter(|token| has_role(token, "surface"))
        .collect();
    for token in context.tokens.values() {
        let (bar, id) = match token.role.as_deref() {
            Some("text") if token.kit.get("large").and_then(Value::as_bool) == Some(true) => {
                (CONTRAST_LARGE, "design/contrast-text")
            }
            Some("text") => (CONTRAST_TEXT, "design/contrast-text"),
            Some("ui") => (CONTRAST_UI, "design/contrast-ui"),
            _ => continue,
        };
        if token.kit.get("incidental").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let surfaces: Vec<&Token> = match token.kit.get("surfaces").and_then(Value::as_array) {
            Some(aliases) => aliases
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|alias| context.tokens.get(alias.trim_matches(['{', '}'])))
                .collect(),
            None => surfaces_all.clone(),
        };
        for surface in surfaces {
            let (Some(color), Some(background)) = (token.hex(), surface.hex()) else {
                continue;
            };
            if let Some(ratio) = contrast(color, background)
                && ratio < bar
            {
                findings.push(format!(
                    "violation {id} — {} on {} ({label}): {:.2} < {bar}",
                    token.dotted(),
                    surface.dotted(),
                    floor2(ratio)
                ));
            }
        }
    }
}

/// Every violation of the design rules in every theme and pointer context.
pub fn check(dir: &Path) -> Result<Vec<String>, String> {
    let mut findings = Vec::new();
    for context in contexts(dir)? {
        check_context(&context, &mut findings);
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_matches_wcag() {
        let ratio = contrast("#000000", "#ffffff").unwrap();
        assert!((ratio - 21.0).abs() < 1e-9);
        let same = contrast("#1e66f5", "#1e66f5").unwrap();
        assert!((same - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_rounded_number_drops_its_zeros() {
        assert_eq!(number(0.8125), "0.8125");
        assert_eq!(number(2.0), "2");
        assert_eq!(number(1.4286), "1.4286");
    }

    #[test]
    fn a_hex_shortens_only_when_it_can() {
        assert_eq!(short_hex("#FFFFFF"), "#fff");
        assert_eq!(short_hex("#1e1e2e"), "#1e1e2e");
    }

    #[test]
    fn the_grid_tolerates_a_rounding_hair() {
        assert!(on_grid(24.0 * 1.3333));
        assert!(!on_grid(14.0 * 1.5));
    }

    #[test]
    fn the_tokens_of_the_window_pass_every_rule() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/vitals-ui/tokens");
        assert_eq!(check(&dir).unwrap(), Vec::<String>::new());
    }
}
