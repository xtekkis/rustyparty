use rand::RngExt;

// Room code letters, without I and O
const CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
const CODE_LENGTH: usize = 4;
// Tries before giving up, so the server answers quickly even when almost every code is taken
pub const MAX_CODE_ATTEMPTS: usize = 100;

// Random room code like "KXTP"
pub fn generate_code() -> String {
    let mut rng = rand::rng();
    let mut code = String::new();
    for _ in 0..CODE_LENGTH {
        let index = rng.random_range(0..CODE_LETTERS.len());
        code.push(CODE_LETTERS[index] as char);
    }
    code
}

// A code nobody is using, or None if every try was taken
pub fn free_code(is_taken: impl Fn(&str) -> bool) -> Option<String> {
    free_code_from(generate_code, is_taken)
}

fn free_code_from(
    mut generate: impl FnMut() -> String,
    is_taken: impl Fn(&str) -> bool,
) -> Option<String> {
    (0..MAX_CODE_ATTEMPTS)
        .map(|_| generate())
        .find(|code| !is_taken(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_free_code_is_found() {
        let mut codes = ["KXTP", "KXTP", "ABCD"].into_iter();
        let code = free_code_from(|| codes.next().unwrap().to_string(), |code| code == "KXTP");
        assert_eq!(code.as_deref(), Some("ABCD"));
    }

    #[test]
    fn it_gives_up_when_every_code_is_taken() {
        let mut tries = 0;
        let code = free_code_from(
            || {
                tries += 1;
                "KXTP".to_string()
            },
            |_| true,
        );
        assert_eq!(code, None);
        assert_eq!(tries, MAX_CODE_ATTEMPTS);
    }
    // Codes are random, so check many of them
    #[test]
    fn code_has_four_uppercase_letters() {
        for _ in 0..1000 {
            let code = generate_code();
            assert_eq!(code.len(), 4, "wrong length: {code}");
            assert!(
                code.chars().all(|c| c.is_ascii_uppercase()),
                "not uppercase: {code}"
            );
        }
    }

    #[test]
    fn code_skips_i_and_o() {
        for _ in 0..1000 {
            let code = generate_code();
            assert!(!code.contains('I'), "contains I: {code}");
            assert!(!code.contains('O'), "contains O: {code}");
        }
    }
}
