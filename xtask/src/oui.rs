//! The makers of network cards by the first three bytes of a MAC: the IEEE MA-L registry
//! (`https://standards-oui.ieee.org/oui/oui.csv`), compacted to `PREFIX<tab>Maker` lines, sorted, for the network
//! screen. The legal form at the end of a name says nothing on a screen and is dropped.

/// Legal forms at the end of a maker's name, longest first
const LEGAL_FORMS: &[&str] = &[
    "co., ltd.",
    "co.,ltd.",
    "co., ltd",
    "co.,ltd",
    "co. ltd.",
    "co ltd",
    "incorporation",
    "incorporated",
    "corporation",
    "limited",
    "gmbh & co. kg",
    "gmbh",
    "corp.",
    "corp",
    "inc.",
    "inc",
    "llc",
    "ltd.",
    "ltd",
    "s.a.",
    "s.p.a.",
    "b.v.",
    "a.g.",
    "ag",
    "oy",
    "ab",
    "sa",
    "co.",
];

/// The fields of one CSV line: commas split, a quoted field keeps its commas, `""` is a quote.
pub fn fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut is_quoted = false;
    let mut letters = line.chars().peekable();
    while let Some(letter) = letters.next() {
        match (letter, is_quoted) {
            ('"', true) if letters.peek() == Some(&'"') => {
                field.push('"');
                letters.next();
            }
            ('"', _) => is_quoted = !is_quoted,
            (',', false) => fields.push(std::mem::take(&mut field)),
            _ => field.push(letter),
        }
    }
    fields.push(field);
    fields
}

/// `Samsung Electronics Co.,Ltd` → `Samsung Electronics`; a name that is only a legal form stays.
pub fn short_name(name: &str) -> String {
    let mut name: String = name.split_whitespace().collect::<Vec<_>>().join(" ");
    loop {
        let lower = name.to_lowercase();
        let Some(form) = LEGAL_FORMS.iter().find(|form| {
            lower.len() > form.len()
                && lower.ends_with(*form)
                && lower[..lower.len() - form.len()].ends_with([' ', ','])
        }) else {
            break;
        };
        name.truncate(name.len() - form.len());
        name = name.trim_end_matches([' ', ',']).to_owned();
    }
    name
}

/// The registry's CSV as sorted `PREFIX<tab>Maker` lines.
pub fn compact(csv: &str) -> String {
    let mut rows: Vec<(String, String)> = csv
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields = fields(line);
            let (registry, prefix, name) = (fields.first()?, fields.get(1)?, fields.get(2)?);
            let prefix = prefix.trim().to_uppercase();
            let is_prefix =
                prefix.len() == 6 && prefix.chars().all(|letter| letter.is_ascii_hexdigit());
            let name = short_name(name);
            (registry == "MA-L" && is_prefix && !name.is_empty() && !name.contains('\t'))
                .then_some((prefix, name))
        })
        .collect();
    rows.sort();
    rows.dedup_by(|left, right| left.0 == right.0);
    rows.into_iter()
        .fold(String::new(), |mut text, (prefix, name)| {
            text.push_str(&prefix);
            text.push('\t');
            text.push_str(&name);
            text.push('\n');
            text
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quoted_field_keeps_its_commas() {
        assert_eq!(
            fields(r#"MA-L,04CB01,"Samsung Electronics Co.,Ltd","#),
            ["MA-L", "04CB01", "Samsung Electronics Co.,Ltd", ""]
        );
        assert_eq!(fields(r#"a,"say ""hi""",b"#), ["a", r#"say "hi""#, "b"]);
    }

    #[test]
    fn legal_forms_are_dropped() {
        assert_eq!(
            short_name("Samsung Electronics Co.,Ltd"),
            "Samsung Electronics"
        );
        assert_eq!(short_name("ASRock Incorporation"), "ASRock");
        assert_eq!(short_name("zte corporation"), "zte");
        assert_eq!(short_name("Apple, Inc."), "Apple");
        assert_eq!(short_name("Inc"), "Inc");
        assert_eq!(
            short_name("Sagemcom Broadband SAS"),
            "Sagemcom Broadband SAS"
        );
    }

    #[test]
    fn the_registry_compacts_sorted_and_whole() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
                   MA-L,DC5193,zte corporation,\"Shenzhen\"\n\
                   MA-M,ABCDEF1,Too Long,x\n\
                   MA-L,04cb01,\"Samsung Electronics Co.,Ltd\",x\n";
        assert_eq!(compact(csv), "04CB01\tSamsung Electronics\nDC5193\tzte\n");
    }
}
