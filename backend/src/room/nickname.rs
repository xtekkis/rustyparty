const NICKNAME_MAX_LENGTH: usize = 16;

// Characters that never belong in a name: they don't show, or they make text appear in a
// different order than it was typed. Without this, "Alex" plus one of them looks like Alex
fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}' // soft hyphen
            | '\u{034F}' // combining grapheme joiner
            | '\u{061C}' // Arabic letter mark
            | '\u{115F}' | '\u{1160}' | '\u{3164}' | '\u{FFA0}' // Hangul fillers, blank letters
            | '\u{17B4}' | '\u{17B5}' // Khmer invisible vowels
            | '\u{180E}' // Mongolian vowel separator
            | '\u{200B}' // zero-width space
            | '\u{200E}' | '\u{200F}' // left-to-right and right-to-left marks
            | '\u{202A}'..='\u{202E}' // text direction overrides
            | '\u{2060}'..='\u{2064}' // word joiner and invisible operators
            | '\u{2066}'..='\u{206F}' // text direction isolates and other format marks
            | '\u{2800}' // blank Braille pattern
            | '\u{FEFF}' // byte order mark
        )
}

// Joiners and styles inside emoji and some scripts. They're kept, so 👨‍👩‍👧 stays one
// picture, but they don't make a name different or count as something that shows
fn is_glue(c: char) -> bool {
    matches!(
        c,
        '\u{200C}' | '\u{200D}' // zero-width non-joiner and joiner
        | '\u{FE00}'..='\u{FE0F}' // variation selectors, like text or emoji style
        | '\u{E0001}' | '\u{E0020}'..='\u{E007F}' // tags, used in some flags
    )
}

// The nickname as it will be shown: hidden characters taken out, every kind of space made a
// plain one, runs of spaces made one, then trimmed. None unless it's 1 to 16 characters and
// at least one of them shows
pub fn clean_nickname(nickname: &str) -> Option<String> {
    let mut cleaned = String::new();
    for c in nickname.chars() {
        if c.is_whitespace() {
            if !cleaned.is_empty() && !cleaned.ends_with(' ') {
                cleaned.push(' ');
            }
        } else if !is_hidden(c) {
            cleaned.push(c);
        }
    }
    let cleaned = cleaned.trim_end();

    let length = cleaned.chars().count();
    let shows = cleaned.chars().any(|c| !is_glue(c));
    if shows && (1..=NICKNAME_MAX_LENGTH).contains(&length) {
        Some(cleaned.to_string())
    } else {
        None
    }
}

// What nicknames are compared by: two names are the same if this is, which ignores case and glue
pub fn nickname_key(nickname: &str) -> String {
    nickname
        .chars()
        .filter(|&c| !is_glue(c))
        .collect::<String>()
        .to_lowercase()
}
