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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub street: String,
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

    let (street, tail) = if lines.len() == 1 {
        split_single_line(lines[0])?
    } else {
        let street = lines[..lines.len() - 1].join(" ");
        (street, lines[lines.len() - 1].to_string())
    };

    if street.trim().is_empty() {
        return Err(ParseError::MissingStreet);
    }

    let (city, state, postal_code) = split_city_state_zip(&tail)?;

    Ok(Address {
        street: street.trim().to_string(),
        city,
        state,
        postal_code,
    })
}

/// Splits a single comma-separated line into `(street, "city, state zip")`.
fn split_single_line(line: &str) -> Result<(String, String), ParseError> {
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return Err(ParseError::MissingCityStateZip);
    }
    let street = parts[0].to_string();
    let tail = parts[1..].join(", ");
    Ok((street, tail))
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
        address.street, address.city, address.state, address.postal_code
    )
}

/// Renders an address as a single comma-separated line, useful for
/// spreadsheet cells or log lines where newlines are awkward.
pub fn format_address_single_line(address: &Address) -> String {
    format!(
        "{}, {}, {} {}",
        address.street, address.city, address.state, address.postal_code
    )
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
}
