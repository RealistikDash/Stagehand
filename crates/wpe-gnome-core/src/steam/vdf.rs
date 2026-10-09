use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VdfError {
    #[error("unterminated string at offset {0}")]
    UnterminatedString(usize),
    #[error("unexpected `}}` at offset {0}")]
    UnexpectedClose(usize),
    #[error("key `{0}` has no value")]
    MissingValue(String),
    #[error("unbalanced braces at end of input")]
    Unbalanced,
}

/// A node of Valve's `KeyValues` text format as found in `libraryfolders.vdf`. Keys are kept in
/// file order and may repeat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VdfNode {
    Value(String),
    Map(Vec<(String, VdfNode)>),
}

impl VdfNode {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&VdfNode> {
        match self {
            Self::Map(entries) => entries
                .iter()
                .find(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
                .map(|(_, node)| node),
            Self::Value(_) => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Value(text) => Some(text),
            Self::Map(_) => None,
        }
    }

    #[must_use]
    pub fn entries(&self) -> &[(String, VdfNode)] {
        match self {
            Self::Map(entries) => entries,
            Self::Value(_) => &[],
        }
    }
}

type Scope = (Option<String>, Vec<(String, VdfNode)>);

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Text(String),
    Open,
    Close,
}

struct Tokeniser<'a> {
    text: &'a str,
    offset: usize,
}

impl Tokeniser<'_> {
    fn next_token(&mut self) -> Result<Option<Token>, VdfError> {
        loop {
            let rest = &self.text.get(self.offset..).unwrap_or_default();
            let trimmed = rest.trim_start();
            self.offset += rest.len() - trimmed.len();

            if trimmed.is_empty() {
                return Ok(None);
            }

            if trimmed.starts_with("//") {
                let line_end = trimmed.find('\n').unwrap_or(trimmed.len());
                self.offset += line_end;

                continue;
            }

            if trimmed.starts_with('{') {
                self.offset += 1;

                return Ok(Some(Token::Open));
            }

            if trimmed.starts_with('}') {
                self.offset += 1;

                return Ok(Some(Token::Close));
            }

            if trimmed.starts_with('"') {
                return self.quoted(trimmed).map(Some);
            }

            let end = trimmed
                .find(|character: char| {
                    character.is_whitespace() || matches!(character, '{' | '}' | '"')
                })
                .unwrap_or(trimmed.len());
            let word = trimmed.get(..end).unwrap_or_default().to_owned();
            self.offset += end;

            return Ok(Some(Token::Text(word)));
        }
    }

    fn quoted(&mut self, trimmed: &str) -> Result<Token, VdfError> {
        let start = self.offset;
        let mut text = String::new();
        let mut characters = trimmed.char_indices().skip(1);

        while let Some((index, character)) = characters.next() {
            match character {
                '"' => {
                    self.offset += index + 1;

                    return Ok(Token::Text(text));
                }
                '\\' => {
                    let Some((_, escaped)) = characters.next() else {
                        return Err(VdfError::UnterminatedString(start));
                    };

                    text.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                }
                other => text.push(other),
            }
        }

        Err(VdfError::UnterminatedString(start))
    }
}

/// Parses the whole document into one map (top-level files hold a single root key).
pub fn parse_vdf(text: &str) -> Result<VdfNode, VdfError> {
    let mut tokeniser = Tokeniser { text, offset: 0 };
    let mut stack: Vec<Scope> = vec![(None, Vec::new())];
    let mut pending_key: Option<String> = None;

    while let Some(token) = tokeniser.next_token()? {
        match token {
            Token::Text(text) => {
                if let Some(key) = pending_key.take() {
                    if let Some((_, entries)) = stack.last_mut() {
                        entries.push((key, VdfNode::Value(text)));
                    }
                } else {
                    pending_key = Some(text);
                }
            }
            Token::Open => {
                let Some(key) = pending_key.take() else {
                    return Err(VdfError::MissingValue(String::new()));
                };

                stack.push((Some(key), Vec::new()));
            }
            Token::Close => {
                if let Some(key) = pending_key.take() {
                    return Err(VdfError::MissingValue(key));
                }

                let Some((Some(key), entries)) = stack.pop() else {
                    return Err(VdfError::UnexpectedClose(tokeniser.offset));
                };

                if let Some((_, parent)) = stack.last_mut() {
                    parent.push((key, VdfNode::Map(entries)));
                }
            }
        }
    }

    if let Some(key) = pending_key {
        return Err(VdfError::MissingValue(key));
    }

    match stack.pop() {
        Some((None, entries)) if stack.is_empty() => Ok(VdfNode::Map(entries)),
        _ => Err(VdfError::Unbalanced),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_nested_library_folders() {
        let text = r#"
            "libraryfolders"
            {
                "0"
                {
                    "path"  "/home/user/.local/share/Steam"
                    "apps" { "431960" "0" }
                }
                "1" { "path" "/mnt/games/SteamLibrary" }
            }
        "#;

        let root = parse_vdf(text).unwrap();

        let folders = root.get("libraryfolders").unwrap();
        assert_eq!(
            folders.get("0").unwrap().get("path").unwrap().as_str(),
            Some("/home/user/.local/share/Steam")
        );
        assert_eq!(
            folders.get("1").unwrap().get("path").unwrap().as_str(),
            Some("/mnt/games/SteamLibrary")
        );
        assert!(
            folders
                .get("0")
                .unwrap()
                .get("apps")
                .unwrap()
                .get("431960")
                .is_some()
        );
    }

    #[test]
    fn test_rejects_unbalanced_braces() {
        assert_eq!(parse_vdf(r#""a" { "b" "c""#), Err(VdfError::Unbalanced));
    }
}
