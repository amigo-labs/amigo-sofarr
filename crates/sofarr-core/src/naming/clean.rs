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

/// Marks a clean title keeps; all of them are valid in file names on Windows too.
const KEPT: [char; 8] = ['-', '.', '–', '!', '…', '#', '·', '‘'];

/// Title for `{… CleanTitle}`: letters (diacritics removed, `ß` as `ss`), digits and the
/// [`KEPT`] marks remain, a run of dots as one dot; a `-` standing alone between spaces is
/// dropped; `&` becomes `and`; other marks are dropped, `:` and slashes become a space.
pub(super) fn clean_title(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if let Some(plain) = plain(c) {
            out.push_str(plain);
        } else if c == '.' && out.ends_with('.') {
            continue;
        } else if c.is_alphanumeric() || KEPT.contains(&c) {
            out.push(c);
        } else if c == '&' {
            out.push_str(" and ");
        } else if c.is_whitespace() || matches!(c, ':' | '/' | '\\') {
            out.push(' ');
        }
    }
    out.split_whitespace()
        .filter(|word| *word != "-")
        .collect::<Vec<_>>()
        .join(" ")
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
        // Apostrophes go (Radarr: "Oceans Eleven", "Trau keinem"); only `‘` stays ("Mary Poppins‘").
        assert_eq!(clean_title("Ocean's Eleven"), "Oceans Eleven");
        assert_eq!(
            clean_title("Haywire – Trau’ keinem"),
            "Haywire – Trau keinem"
        );
        assert_eq!(clean_title("Fast & Furious"), "Fast and Furious");
        assert_eq!(clean_title("Wer? Wie! Was/Wo"), "Wer Wie! Was Wo");
    }

    #[test]
    fn a_hyphen_standing_alone_is_dropped_one_inside_a_word_stays() {
        assert_eq!(clean_title("96 Hours - Taken 2"), "96 Hours Taken 2");
        assert_eq!(
            clean_title("Dead Snow - Red vs. Dead"),
            "Dead Snow Red vs. Dead"
        );
        assert_eq!(clean_title("- Anfang -"), "Anfang");
        assert_eq!(clean_title("Super-Pets - Der Film"), "Super-Pets Der Film");
    }

    #[test]
    fn exclamation_marks_and_typographic_marks_stay_dot_runs_become_one_dot() {
        assert_eq!(
            clean_title("Shazam! Fury of the Gods"),
            "Shazam! Fury of the Gods"
        );
        assert_eq!(
            clean_title("Männerherzen... und die ganz ganz große Liebe"),
            "Mannerherzen. und die ganz ganz grosse Liebe"
        );
        assert_eq!(
            clean_title("Jungfrau (40), männlich, sucht ..."),
            "Jungfrau 40 mannlich sucht ."
        );
        assert_eq!(
            clean_title("Once Upon a Time in… Hollywood"),
            "Once Upon a Time in… Hollywood"
        );
        assert_eq!(clean_title("Lucky # Slevin"), "Lucky # Slevin");
        assert_eq!(clean_title("WALL·E"), "WALL·E");
        assert_eq!(
            clean_title("Mary Poppins‘ Rückkehr"),
            "Mary Poppins‘ Ruckkehr"
        );
    }
}
