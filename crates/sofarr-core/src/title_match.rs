//! Does a release belong to a film? Compares the parsed title candidates with the
//! film's titles.
//!
//! Derived from the release oracle: a release is matched when a candidate equals one
//! of the film's titles, extends it by further words (a localised subtitle appended,
//! `The Film Untertitel`), or differs from it by a small typo (one edit per ten
//! letters). Titles are compared case-insensitively, punctuation ignored.

/// Whether any of `candidates` (parsed from the release) names the film with `titles`.
pub fn matches(candidates: &[String], titles: &[String]) -> bool {
    let titles: Vec<Vec<String>> = titles
        .iter()
        .map(|t| words(t))
        .filter(|w| !w.is_empty())
        .collect();
    candidates.iter().any(|candidate| {
        let candidate = words(candidate);
        titles.iter().any(|title| same_film(&candidate, title))
    })
}

fn same_film(candidate: &[String], title: &[String]) -> bool {
    if candidate.starts_with(title) {
        return true;
    }
    let (a, b) = (candidate.concat(), title.concat());
    let allowed = b.chars().count() / 10;
    allowed > 0 && edit_distance(&a, &b) <= allowed
}

/// Lower-case words of letters and digits.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Levenshtein distance over chars.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = diagonal + usize::from(ca != *cb);
            diagonal = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(diagonal + 1);
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(candidate: &str, title: &str) -> bool {
        matches(&[candidate.to_owned()], &[title.to_owned()])
    }

    #[test]
    fn exact_ignoring_case_and_punctuation() {
        assert!(check("Some Film Part Two", "Some Film: Part Two"));
        assert!(check("some-film part two", "Some Film Part Two"));
        assert!(!check("Other Film", "Some Film"));
    }

    #[test]
    fn appended_words_still_match() {
        assert!(check("The Long Night Die lange Nacht", "The Long Night"));
        assert!(!check("Long Night", "The Long Night"));
    }

    #[test]
    fn small_typos_scale_with_length() {
        assert!(check("The Long Nght", "The Long Night"));
        assert!(check("The Lon Night", "The Long Night"));
        assert!(!check("The Song Fight", "The Long Night"));
        // Short titles must match exactly.
        assert!(!check("Us", "Up"));
    }

    #[test]
    fn any_candidate_and_any_title() {
        let candidates = ["Ein Film".to_owned(), "A Film".to_owned()];
        let titles = ["Another".to_owned(), "A Film".to_owned()];
        assert!(matches(&candidates, &titles));
        assert!(!matches(&[], &titles));
        assert!(!matches(&candidates, &[]));
    }

    #[test]
    fn distance() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("same", "same"), 0);
    }
}
