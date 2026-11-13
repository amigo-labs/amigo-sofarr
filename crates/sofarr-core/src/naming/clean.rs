//! `{… CleanTitle}`: a title as it can stand in a path on any file system, in the words
//! Radarr's rename shows (owner's comparison, `docs/parity.md` "Stand Dateinamen").

/// Letters with diacritics and their plain spelling; `ß` and the ligatures need two letters.
const PLAIN: &[(&str, &str)] = &[
    ("àáâãäåāăą", "a"),
    ("ÀÁÂÃÄÅĀĂĄ", "A"),
    ("çćĉċč", "c"),
    ("ÇĆĈĊČ", "C"),
    ("ďđ", "d"),
    ("ĎĐ", "D"),
    ("èéêëēĕėęě", "e"),
    ("ÈÉÊËĒĔĖĘĚ", "E"),
    ("ĝğġģ", "g"),
    ("ĜĞĠĢ", "G"),
    ("ĥħ", "h"),
    ("ĤĦ", "H"),
    ("ìíîïĩīĭįı", "i"),
    ("ÌÍÎÏĨĪĬĮİ", "I"),
    ("ĵ", "j"),
    ("Ĵ", "J"),
    ("ķ", "k"),
    ("Ķ", "K"),
    ("ĺļľŀł", "l"),
    ("ĹĻĽĿŁ", "L"),
    ("ñńņň", "n"),
    ("ÑŃŅŇ", "N"),
    ("òóôõöøōŏő", "o"),
    ("ÒÓÔÕÖØŌŎŐ", "O"),
    ("ŕŗř", "r"),
    ("ŔŖŘ", "R"),
    ("śŝşš", "s"),
    ("ŚŜŞŠ", "S"),
    ("ţťŧ", "t"),
    ("ŢŤŦ", "T"),
    ("ùúûüũūŭůűų", "u"),
    ("ÙÚÛÜŨŪŬŮŰŲ", "U"),
    ("ŵ", "w"),
    ("Ŵ", "W"),
    ("ýÿŷ", "y"),
    ("ÝŸŶ", "Y"),
    ("źżž", "z"),
    ("ŹŻŽ", "Z"),
    ("ß", "ss"),
    ("æ", "ae"),
    ("Æ", "AE"),
    ("œ", "oe"),
    ("Œ", "OE"),
];

/// Title for `{… CleanTitle}`: letters (diacritics removed, `ß` as `ss`), digits, `-`, `.`
/// and `–` remain; `&` becomes `and`; other marks are dropped, `:` and slashes become a space.
pub(super) fn clean_title(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if let Some(plain) = plain(c) {
            out.push_str(plain);
        } else if c.is_alphanumeric() || matches!(c, '-' | '.' | '–') {
            out.push(c);
        } else if c == '&' {
            out.push_str(" and ");
        } else if c.is_whitespace() || matches!(c, ':' | '/' | '\\') {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn plain(c: char) -> Option<&'static str> {
    PLAIN
        .iter()
        .find(|(letters, _)| letters.contains(c))
        .map(|(_, plain)| *plain)
}

#[cfg(test)]
mod tests {
    use super::clean_title;

    #[test]
    fn diacritics_are_removed_and_sharp_s_becomes_ss() {
        assert_eq!(clean_title("Fack ju Göhte"), "Fack ju Gohte");
        assert_eq!(clean_title("Der große Gatsby"), "Der grosse Gatsby");
        assert_eq!(
            clean_title("Der Herr der Ringe: Die Rückkehr des Königs"),
            "Der Herr der Ringe Die Ruckkehr des Konigs"
        );
        assert_eq!(clean_title("Ça déménage à Noël"), "Ca demenage a Noel");
        assert_eq!(clean_title("ÄRA"), "ARA");
    }

    #[test]
    fn hyphens_dots_and_dashes_remain_other_marks_go() {
        assert_eq!(clean_title("Ant-Man"), "Ant-Man");
        assert_eq!(
            clean_title("Dead Snow: Red vs. Dead"),
            "Dead Snow Red vs. Dead"
        );
        assert_eq!(
            clean_title("Der Super Mario Bros. Film"),
            "Der Super Mario Bros. Film"
        );
        assert_eq!(
            clean_title("Die Tribute von Panem – Catching Fire"),
            "Die Tribute von Panem – Catching Fire"
        );
        assert_eq!(clean_title("Ender's Game"), "Enders Game");
        assert_eq!(clean_title("Fast & Furious"), "Fast and Furious");
        assert_eq!(clean_title("Wer? Wie! Was/Wo"), "Wer Wie Was Wo");
    }
}
