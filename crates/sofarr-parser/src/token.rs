//! Splits a release title into tokens.

/// File extensions that are cut off before tokenizing. `.ts` is deliberately missing:
/// `TS` is far more often a source tag than a container.
const EXTENSIONS: &[&str] = &[".mkv", ".mp4", ".avi", ".m4v", ".wmv", ".mov", ".nzb"];

/// What separated a token from the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sep {
    /// First token of the title.
    Start,
    /// `.`, `_`, whitespace, `,` or a mix of them.
    Plain,
    /// A run of separators that contains a `-`.
    Dash,
    /// A run of separators that contains an opening or closing bracket.
    Bracket,
}

#[derive(Debug, Clone)]
pub struct Token<'a> {
    /// The token exactly as it appears in the input.
    pub text: &'a str,
    /// Lower-cased text without apostrophes, used for keyword lookups.
    pub key: String,
    /// Byte range in the (extension-stripped) input.
    pub start: usize,
    pub end: usize,
    pub sep: Sep,
    /// Inside `()`, `[]` or `{}`.
    pub bracketed: bool,
}

/// Removes a trailing container/NZB extension.
pub fn strip_extension(input: &str) -> &str {
    let trimmed = input.trim();
    let lower = trimmed.to_ascii_lowercase();
    for ext in EXTENSIONS {
        if lower.len() > ext.len() && lower.ends_with(ext) {
            // `to_ascii_lowercase` keeps byte lengths, and the extension is ASCII,
            // so the cut is on a char boundary.
            return &trimmed[..trimmed.len() - ext.len()];
        }
    }
    trimmed
}

fn is_separator(c: char) -> bool {
    matches!(c, '.' | '_' | '-' | ',') || c.is_whitespace()
}

fn is_bracket(c: char) -> bool {
    matches!(c, '(' | ')' | '[' | ']' | '{' | '}')
}

pub fn tokenize(input: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut depth: u32 = 0;
    let mut sep = Sep::Start;
    let mut token_start: Option<usize> = None;

    let mut push = |start: usize, end: usize, sep: Sep, depth: u32| {
        let text = &input[start..end];
        let key = text
            .chars()
            .filter(|c| !matches!(c, '\'' | '’' | '`'))
            .flat_map(char::to_lowercase)
            .collect();
        tokens.push(Token {
            text,
            key,
            start,
            end,
            sep,
            bracketed: depth > 0,
        });
    };

    for (i, c) in input.char_indices() {
        if is_separator(c) || is_bracket(c) {
            if let Some(start) = token_start.take() {
                push(start, i, sep, depth);
                sep = Sep::Plain;
            }
            // Strongest separator in the run wins: bracket > dash > plain.
            if is_bracket(c) {
                sep = Sep::Bracket;
                match c {
                    '(' | '[' | '{' => depth += 1,
                    _ => depth = depth.saturating_sub(1),
                }
            } else if c == '-' && sep != Sep::Bracket && sep != Sep::Start {
                sep = Sep::Dash;
            }
        } else if token_start.is_none() {
            token_start = Some(i);
        }
    }
    if let Some(start) = token_start {
        push(start, input.len(), sep, depth);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(input: &str) -> Vec<&str> {
        tokenize(input).iter().map(|t| t.text).collect()
    }

    #[test]
    fn splits_on_all_separators() {
        assert_eq!(
            texts("Alpha.Beta_Gamma Delta-Epsilon"),
            ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]
        );
    }

    #[test]
    fn records_separator_kind() {
        let tokens = tokenize("One.Two-Three (Four)");
        let seps: Vec<Sep> = tokens.iter().map(|t| t.sep).collect();
        assert_eq!(seps, [Sep::Start, Sep::Plain, Sep::Dash, Sep::Bracket]);
        assert!(tokens[3].bracketed);
        assert!(!tokens[2].bracketed);
    }

    #[test]
    fn spaced_dash_counts_as_dash() {
        let tokens = tokenize("Left - Right");
        assert_eq!(tokens[1].sep, Sep::Dash);
    }

    #[test]
    fn key_is_lowercase_without_apostrophes() {
        let tokens = tokenize("Collector's.EDITION");
        assert_eq!(tokens[0].key, "collectors");
        assert_eq!(tokens[1].key, "edition");
    }

    #[test]
    fn strips_known_extensions_only() {
        assert_eq!(strip_extension("Some.Film.2001.mkv"), "Some.Film.2001");
        assert_eq!(strip_extension("Some.Film.2001.NZB"), "Some.Film.2001");
        assert_eq!(strip_extension("Some.Film.2001.TS"), "Some.Film.2001.TS");
        assert_eq!(strip_extension(".mkv"), ".mkv");
    }

    #[test]
    fn handles_unbalanced_brackets_and_unicode() {
        assert_eq!(texts("Über ]Größe[ (x"), ["Über", "Größe", "x"]);
        assert!(tokenize("").is_empty());
        assert!(tokenize(" ..--__ ").is_empty());
    }
}
