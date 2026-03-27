#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    pub key: String,
    pub value: String,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceError {
    pub message: String,
    pub line: u32,
    pub column: u32,
}

pub fn parse_mf2_source(input: &str) -> Result<Vec<SourceEntry>, SourceError> {
    let mut entries = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();
    let mut current_line = 0u32;

    for (idx, raw_line) in input.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if current_key.is_none() {
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }
            let mut parts = line.splitn(2, '=');
            let key_part = parts.next().unwrap_or("").trim();
            let value_part = parts.next().ok_or_else(|| SourceError {
                message: "expected '=' in entry".to_string(),
                line: line_no,
                column: 1,
            })?;
            if key_part.is_empty() {
                return Err(SourceError {
                    message: "missing key".to_string(),
                    line: line_no,
                    column: 1,
                });
            }
            if !is_valid_key(key_part) {
                return Err(SourceError {
                    message: "invalid key".to_string(),
                    line: line_no,
                    column: 1,
                });
            }
            current_key = Some(key_part.to_string());
            current_value.clear();
            current_value.push_str(value_part.trim_start());
            current_line = line_no;
        } else if trimmed.is_empty() {
            flush_entry(
                &mut entries,
                &mut current_key,
                &mut current_value,
                current_line,
            );
        } else {
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(line);
        }
    }

    if current_key.is_some() {
        flush_entry(
            &mut entries,
            &mut current_key,
            &mut current_value,
            current_line,
        );
    }

    Ok(entries)
}

fn flush_entry(
    entries: &mut Vec<SourceEntry>,
    key: &mut Option<String>,
    value: &mut String,
    line: u32,
) {
    if let Some(key_value) = key.take() {
        entries.push(SourceEntry {
            key: key_value,
            value: value.trim_end().to_string(),
            line,
        });
    }
    value.clear();
}

fn is_valid_key(key: &str) -> bool {
    key.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || byte == b'.'
            || byte == b'_'
            || byte == b'-'
    })
}

#[cfg(test)]
mod tests {
    use super::parse_mf2_source;

    #[test]
    fn parses_single_line_entry() {
        let input = "home.title = Hello { $name }";
        let entries = parse_mf2_source(input).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "home.title");
        assert_eq!(entries[0].value, "Hello { $name }");
    }

    #[test]
    fn parses_multiline_entry() {
        let input = "home.body = line1\nline2\n\nfooter.text = end";
        let entries = parse_mf2_source(input).expect("parse");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].value, "line1\nline2");
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let input = "# comment\n\nhome.title = Hi\n// other\n";
        let entries = parse_mf2_source(input).expect("parse");
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn rejects_invalid_key() {
        let input = "Home.Title = Hi";
        let err = parse_mf2_source(input).expect_err("error");
        assert_eq!(err.message, "invalid key");
    }
}
