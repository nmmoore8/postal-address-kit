//! Parsing and formatting for US-style postal addresses.
//!
//! The parser accepts the two shapes people actually type addresses in
//! (a street line plus a "City, ST ZIP" line, or the same thing joined
//! onto one comma-separated line) and turns them into a structured
//! [`Address`]. The printer goes the other way, producing the two-line
//! form USPS uses on envelopes.

mod states;

pub use states::is_valid_state_code;

use std::fmt;

/// A structured US postal address.
///
/// `state` is always normalized to its two-letter uppercase form.
/// `postal_code` keeps whatever digit grouping the input used
/// (`"62704"` or `"62704-1234"`) rather than forcing one shape.
/// `unit` holds an apartment, suite, or similar sub-unit designator
/// (`"Apt 4B"`, `"Suite 200"`) separately from `street` so callers can
/// lay the two out however they need. A PO box line ends up in `street`
/// too, normalized to `"PO Box <number>"`; use [`is_po_box`] to tell the
/// two apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub street: String,
    pub unit: Option<String>,
    pub city: String,
    pub state: String,
    pub postal_code: String,
}

/// Reasons an address string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    MissingStreet,
    MissingCityStateZip,
    UnrecognizedState(String),
    InvalidPostalCode(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "address is empty"),
            ParseError::MissingStreet => write!(f, "missing street line"),
            ParseError::MissingCityStateZip => {
                write!(f, "missing city, state, and zip code")
            }
            ParseError::UnrecognizedState(s) => {
                write!(f, "'{s}' is not a recognized state abbreviation")
            }
            ParseError::InvalidPostalCode(s) => write!(f, "'{s}' is not a valid zip code"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses a US postal address from either of two common shapes:
///
/// ```text
/// 123 Main St
/// Springfield, IL 62704
/// ```
///
/// or the same thing on one line:
///
/// ```text
/// 123 Main St, Springfield, IL 62704
/// ```
///
/// Blank lines and surrounding whitespace are ignored. State abbreviations
/// are case-insensitive on input; the returned `Address` always has an
/// uppercase state.
pub fn parse_address(input: &str) -> Result<Address, ParseError> {
    let lines: Vec<&str> = input
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();

    if lines.is_empty() {
        return Err(ParseError::Empty);
    }

    let (raw_street, comma_unit, tail) = if lines.len() == 1 {
        split_single_line(lines[0])?
    } else {
        let street = lines[..lines.len() - 1].join(" ");
        (street, None, lines[lines.len() - 1].to_string())
    };

    if raw_street.trim().is_empty() {
        return Err(ParseError::MissingStreet);
    }

    let trimmed_street = raw_street.trim();
    let (street, embedded_unit) = match normalize_po_box(trimmed_street) {
        Some(po_box) => (po_box, None),
        None => extract_unit(trimmed_street),
    };
    let unit = comma_unit.or(embedded_unit);

    let (city, state, postal_code) = split_city_state_zip(&tail)?;

    Ok(Address {
        street,
        unit,
        city,
        state,
        postal_code,
    })
}

/// Splits a single comma-separated line into
/// `(street, unit if its own comma segment, "city, state zip")`.
fn split_single_line(line: &str) -> Result<(String, Option<String>, String), ParseError> {
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return Err(ParseError::MissingCityStateZip);
    }
    // "123 Main St, Apt 4B, Springfield, IL 62704" - a unit written as its
    // own comma segment rather than folded into the street segment.
    if parts.len() >= 4 && starts_with_unit_designator(parts[1]) {
        let street = parts[0].to_string();
        let unit = parts[1].to_string();
        let tail = parts[2..].join(", ");
        return Ok((street, Some(unit), tail));
    }
    let street = parts[0].to_string();
    let tail = parts[1..].join(", ");
    Ok((street, None, tail))
}

/// Unit/sub-address designators recognized when splitting a street line.
/// Deliberately narrow: broader words like "no" or "unit"-adjacent
/// abbreviations show up as ordinary street words too often to guess at.
const UNIT_DESIGNATORS: &[&str] = &[
    "apt",
    "apartment",
    "suite",
    "ste",
    "unit",
    "rm",
    "room",
    "fl",
    "floor",
    "bldg",
    "building",
];

fn is_unit_designator(token: &str) -> bool {
    UNIT_DESIGNATORS.contains(&token.trim_end_matches('.').to_lowercase().as_str())
}

fn starts_with_unit_designator(segment: &str) -> bool {
    segment
        .split_whitespace()
        .next()
        .is_some_and(is_unit_designator)
}

/// Splits a street line into the street proper and an optional unit, e.g.
/// `"123 Main St Apt 4B"` -> `("123 Main St", Some("Apt 4B"))`. A bare `#4B`
/// with no preceding word also counts as a unit designator.
fn extract_unit(street: &str) -> (String, Option<String>) {
    let tokens: Vec<&str> = street.split_whitespace().collect();
    for (i, tok) in tokens.iter().enumerate() {
        let is_hash_unit = tok.starts_with('#') && tok.len() > 1;
        let is_word_unit = is_unit_designator(tok) && i + 1 < tokens.len();
        if is_hash_unit || is_word_unit {
            let addr_part = tokens[..i].join(" ");
            let unit_part = tokens[i..].join(" ");
            if addr_part.is_empty() {
                continue;
            }
            return (addr_part, Some(unit_part));
        }
    }
    (street.to_string(), None)
}

/// Prefixes people write before a PO box number, checked longest/most
/// specific first so "PO Box" doesn't get short-circuited by the bare
/// "box" fallback. Each token has already had periods stripped and been
/// lowercased by the caller.
const PO_BOX_PREFIXES: &[&[&str]] = &[
    &["po", "box"],
    &["p", "o", "box"],
    &["post", "office", "box"],
    &["pob"],
    &["box"],
];

/// Recognizes a street line that names a PO box rather than a street
/// address, in any of the ways people write it ("PO Box 123", "P.O. Box
/// 123", "P O Box 123", "Post Office Box 123", "POB 123", "Box 123"), and
/// returns it in the canonical "PO Box <number>" form. Returns `None` for
/// anything that isn't a PO box line, including a bare "PO Box" with no
/// number after it.
fn normalize_po_box(street: &str) -> Option<String> {
    let raw_tokens: Vec<&str> = street.split_whitespace().collect();
    let normalized_tokens: Vec<String> = raw_tokens
        .iter()
        .map(|t| t.replace('.', "").to_lowercase())
        .collect();

    for prefix in PO_BOX_PREFIXES {
        if normalized_tokens.len() <= prefix.len() {
            continue;
        }
        let matches = normalized_tokens
            .iter()
            .zip(prefix.iter())
            .all(|(tok, want)| tok.as_str() == *want);
        if matches {
            let box_id = raw_tokens[prefix.len()..].join(" ");
            return Some(format!("PO Box {box_id}"));
        }
    }
    None
}

/// True if `street` is a street line naming a PO box, whether or not it's
/// already in canonical form.
pub fn is_po_box(street: &str) -> bool {
    normalize_po_box(street).is_some()
}

/// Splits `"City, ST ZIP"` into its three parts, validating state and zip.
fn split_city_state_zip(tail: &str) -> Result<(String, String, String), ParseError> {
    let comma = tail.find(',').ok_or(ParseError::MissingCityStateZip)?;
    let city = tail[..comma].trim().to_string();
    let rest = tail[comma + 1..].trim();

    let mut tokens: Vec<&str> = rest.split_whitespace().collect();
    let postal_code = tokens
        .pop()
        .ok_or(ParseError::MissingCityStateZip)?
        .to_string();
    if tokens.is_empty() || city.is_empty() {
        return Err(ParseError::MissingCityStateZip);
    }
    let state = tokens.join(" ").to_uppercase();

    if !states::is_valid_state_code(&state) {
        return Err(ParseError::UnrecognizedState(state));
    }
    if !is_valid_postal_code(&postal_code) {
        return Err(ParseError::InvalidPostalCode(postal_code));
    }

    Ok((city, state, postal_code))
}

/// True for a five digit zip (`"12345"`) or zip+4 (`"12345-6789"`).
pub fn is_valid_postal_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    match bytes.len() {
        5 => bytes.iter().all(u8::is_ascii_digit),
        10 => {
            bytes[..5].iter().all(u8::is_ascii_digit)
                && bytes[5] == b'-'
                && bytes[6..].iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

/// Renders an address as two lines, the form USPS prints on envelopes:
///
/// ```text
/// 123 Main St
/// Springfield, IL 62704
/// ```
pub fn format_address(address: &Address) -> String {
    format!(
        "{}\n{}, {} {}",
        street_line(address),
        address.city,
        address.state,
        address.postal_code
    )
}

/// Renders an address as a single comma-separated line, useful for
/// spreadsheet cells or log lines where newlines are awkward.
pub fn format_address_single_line(address: &Address) -> String {
    format!(
        "{}, {}, {} {}",
        street_line(address),
        address.city,
        address.state,
        address.postal_code
    )
}

/// The street segment as printed: the street address followed by the
/// unit designator, when there is one, on the same line.
fn street_line(address: &Address) -> String {
    match &address.unit {
        Some(unit) => format!("{} {}", address.street, unit),
        None => address.street.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_line_address() {
        let addr = parse_address("123 Main St\nSpringfield, IL 62704").unwrap();
        assert_eq!(addr.street, "123 Main St");
        assert_eq!(addr.city, "Springfield");
        assert_eq!(addr.state, "IL");
        assert_eq!(addr.postal_code, "62704");
    }

    #[test]
    fn parses_single_line_address() {
        let addr = parse_address("123 Main St, Springfield, IL 62704").unwrap();
        assert_eq!(addr.street, "123 Main St");
        assert_eq!(addr.city, "Springfield");
        assert_eq!(addr.state, "IL");
    }

    #[test]
    fn accepts_zip_plus_four() {
        let addr = parse_address("1 Infinite Loop\nCupertino, CA 95014-2083").unwrap();
        assert_eq!(addr.postal_code, "95014-2083");
    }

    #[test]
    fn lowercase_state_is_normalized_to_uppercase() {
        let addr = parse_address("1 Infinite Loop\nCupertino, ca 95014").unwrap();
        assert_eq!(addr.state, "CA");
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(parse_address(""), Err(ParseError::Empty));
        assert_eq!(parse_address("   \n  "), Err(ParseError::Empty));
    }

    #[test]
    fn rejects_unknown_state() {
        let err = parse_address("123 Main St\nSpringfield, ZZ 62704").unwrap_err();
        assert_eq!(err, ParseError::UnrecognizedState("ZZ".to_string()));
    }

    #[test]
    fn rejects_bad_zip() {
        let err = parse_address("123 Main St\nSpringfield, IL 6270").unwrap_err();
        assert_eq!(err, ParseError::InvalidPostalCode("6270".to_string()));
    }

    #[test]
    fn rejects_line_missing_comma_between_city_and_state() {
        let err = parse_address("123 Main St\nSpringfield IL 62704").unwrap_err();
        assert_eq!(err, ParseError::MissingCityStateZip);
    }

    #[test]
    fn format_produces_two_line_shape() {
        let addr = parse_address("123 Main St\nSpringfield, IL 62704").unwrap();
        assert_eq!(format_address(&addr), "123 Main St\nSpringfield, IL 62704");
    }

    #[test]
    fn format_single_line_round_trips_through_parse() {
        let addr = parse_address("123 Main St\nSpringfield, IL 62704").unwrap();
        let line = format_address_single_line(&addr);
        assert_eq!(line, "123 Main St, Springfield, IL 62704");
        assert_eq!(parse_address(&line).unwrap(), addr);
    }

    #[test]
    fn extracts_unit_folded_into_two_line_street() {
        let addr = parse_address("123 Main St Apt 4B\nSpringfield, IL 62704").unwrap();
        assert_eq!(addr.street, "123 Main St");
        assert_eq!(addr.unit.as_deref(), Some("Apt 4B"));
    }

    #[test]
    fn extracts_unit_from_own_line() {
        let addr = parse_address("123 Main St\nApt 4B\nSpringfield, IL 62704").unwrap();
        assert_eq!(addr.street, "123 Main St");
        assert_eq!(addr.unit.as_deref(), Some("Apt 4B"));
    }

    #[test]
    fn extracts_unit_folded_into_single_line_street() {
        let addr = parse_address("123 Main St Suite 200, Springfield, IL 62704").unwrap();
        assert_eq!(addr.street, "123 Main St");
        assert_eq!(addr.unit.as_deref(), Some("Suite 200"));
    }

    #[test]
    fn extracts_unit_as_own_comma_segment() {
        let addr = parse_address("123 Main St, Apt 4B, Springfield, IL 62704").unwrap();
        assert_eq!(addr.street, "123 Main St");
        assert_eq!(addr.unit.as_deref(), Some("Apt 4B"));
        assert_eq!(addr.city, "Springfield");
    }

    #[test]
    fn extracts_hash_unit_with_no_designator_word() {
        let addr = parse_address("123 Main St #4B\nSpringfield, IL 62704").unwrap();
        assert_eq!(addr.street, "123 Main St");
        assert_eq!(addr.unit.as_deref(), Some("#4B"));
    }

    #[test]
    fn addresses_without_a_unit_leave_it_none() {
        let addr = parse_address("123 Main St\nSpringfield, IL 62704").unwrap();
        assert_eq!(addr.unit, None);
    }

    #[test]
    fn format_places_unit_on_street_line() {
        let addr = parse_address("123 Main St Apt 4B\nSpringfield, IL 62704").unwrap();
        assert_eq!(
            format_address(&addr),
            "123 Main St Apt 4B\nSpringfield, IL 62704"
        );
    }

    #[test]
    fn format_with_unit_round_trips_through_parse() {
        let addr = parse_address("123 Main St Apt 4B\nSpringfield, IL 62704").unwrap();
        let line = format_address_single_line(&addr);
        assert_eq!(line, "123 Main St Apt 4B, Springfield, IL 62704");
        assert_eq!(parse_address(&line).unwrap(), addr);
    }

    #[test]
    fn recognizes_po_box_street_line() {
        let addr = parse_address("PO Box 123\nSpringfield, IL 62704").unwrap();
        assert_eq!(addr.street, "PO Box 123");
        assert_eq!(addr.unit, None);
        assert!(is_po_box(&addr.street));
    }

    #[test]
    fn normalizes_po_box_spelling_variants() {
        let variants = [
            "P.O. Box 123",
            "P O Box 123",
            "Post Office Box 123",
            "POB 123",
            "Box 123",
        ];
        for variant in variants {
            let addr = parse_address(&format!("{variant}\nSpringfield, IL 62704")).unwrap();
            assert_eq!(addr.street, "PO Box 123", "input was {variant}");
        }
    }

    #[test]
    fn recognizes_po_box_on_single_comma_line() {
        let addr = parse_address("PO Box 123, Springfield, IL 62704").unwrap();
        assert_eq!(addr.street, "PO Box 123");
    }

    #[test]
    fn po_box_round_trips_through_format() {
        let addr = parse_address("PO Box 123\nSpringfield, IL 62704").unwrap();
        assert_eq!(format_address(&addr), "PO Box 123\nSpringfield, IL 62704");
    }

    #[test]
    fn bare_po_box_with_no_number_is_not_a_po_box() {
        assert!(!is_po_box("PO Box"));
        assert!(!is_po_box("123 Main St"));
    }
}
