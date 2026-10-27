//! winget prints tables, not JSON: a title line, a line of dashes, rows padded to the columns of the titles. The
//! titles follow the language of Windows, so columns are found by where the titles start, never by their words.

/// A line of dashes this long or longer separates the titles from the rows
const MIN_RULE: usize = 10;

/// What the console shows of a line: winget redraws its spinner with `\r`, the last drawing is what stays.
fn shown(line: &str) -> &str {
    line.rsplit('\r')
        .find(|part| !part.trim().is_empty())
        .unwrap_or("")
}

/// The width a console gives a character: East Asian wide letters take two cells, as winget pads them.
fn cells(letter: char) -> usize {
    let code = u32::from(letter);
    let is_wide = matches!(code,
        0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x1_F300..=0x1_F64F | 0x2_0000..=0x3_FFFD);
    if is_wide { 2 } else { 1 }
}

/// The cells where the titles start: a letter after a space, or the first letter.
fn column_starts(titles: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut cell = 0;
    let mut after_space = true;
    for letter in titles.chars() {
        let is_space = letter == ' ';
        if !is_space && after_space {
            starts.push(cell);
        }
        after_space = is_space;
        cell += cells(letter);
    }
    starts
}

/// A row cut at the column starts, each field trimmed.
fn fields(row: &str, starts: &[usize]) -> Vec<String> {
    let mut fields = vec![String::new(); starts.len()];
    let mut cell = 0;
    for letter in row.chars() {
        let column = starts.iter().rposition(|start| *start <= cell).unwrap_or(0);
        if let Some(field) = fields.get_mut(column) {
            field.push(letter);
        }
        cell += cells(letter);
    }
    fields.iter().map(|field| field.trim().to_owned()).collect()
}

/// The rows of the first table of the output, up to its first empty line.
pub fn parse(output: &str) -> Vec<Vec<String>> {
    let lines: Vec<&str> = output.split('\n').map(shown).collect();
    let Some(rule) = lines.iter().position(|line| {
        let line = line.trim_end();
        line.len() >= MIN_RULE && line.chars().all(|letter| letter == '-')
    }) else {
        return Vec::new();
    };
    let Some(titles) = rule.checked_sub(1).and_then(|index| lines.get(index)) else {
        return Vec::new();
    };
    let starts = column_starts(titles);
    lines
        .iter()
        .skip(rule + 1)
        .take_while(|line| !line.trim().is_empty())
        .map(|line| fields(line, &starts))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_the_titles_in_any_language() {
        let output = "Имя      ИД          Версия Доступно Источник\n\
                      ---------------------------------------------\n\
                      Git      Git.Git     2.51.2 2.56.0.3 winget\n\
                      \n\
                      Name Id\n----------\nOther table\n";
        assert_eq!(
            parse(output),
            [["Git", "Git.Git", "2.51.2", "2.56.0.3", "winget"]]
        );
    }

    #[test]
    fn a_wide_letter_takes_two_cells() {
        let output = "Name   Id    Version\n--------------------\n微信    Wx.In 4.0\n";
        assert_eq!(parse(output), [["微信", "Wx.In", "4.0"]]);
    }

    #[test]
    fn the_spinner_before_the_titles_is_not_a_line() {
        assert_eq!(shown("\r   - \r   \\ \rName Id"), "Name Id");
        assert_eq!(shown("Git Git.Git\r"), "Git Git.Git");
    }

    #[test]
    fn no_rule_is_no_table() {
        assert_eq!(
            parse("No installed package found matching input criteria."),
            Vec::<Vec<String>>::new()
        );
    }
}
