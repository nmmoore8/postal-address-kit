//! US state and territory abbreviations accepted as valid.

const STATE_CODES: &[&str] = &[
    "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "FL", "GA", "HI", "ID", "IL", "IN", "IA",
    "KS", "KY", "LA", "ME", "MD", "MA", "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ",
    "NM", "NY", "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC", "SD", "TN", "TX", "UT", "VT",
    "VA", "WA", "WV", "WI", "WY", "DC", "PR", "GU", "VI", "AS", "MP",
];

/// Checks a two-letter code against the fifty states, DC, and the
/// inhabited territories. The input must already be uppercase; callers
/// normalize case before checking so this stays a plain lookup.
pub fn is_valid_state_code(code: &str) -> bool {
    STATE_CODES.contains(&code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_known_codes() {
        assert!(is_valid_state_code("CA"));
        assert!(is_valid_state_code("DC"));
        assert!(is_valid_state_code("PR"));
    }

    #[test]
    fn rejects_unknown_or_wrong_case_codes() {
        assert!(!is_valid_state_code("ZZ"));
        assert!(!is_valid_state_code("ca"));
        assert!(!is_valid_state_code(""));
    }
}
