# addrkit

A small Rust library for parsing and printing US postal addresses.

Address strings show up everywhere in slightly different shapes: a two-line
form on an envelope, a comma-joined single line in a CSV export, mixed case
state codes, zip vs. zip+4. This library turns any of those into one
`Address` struct so the rest of your code can stop guessing at string
formats, and turns an `Address` back into the canonical two-line form when
you need to print or export one.

Everything here is a pure function: same input always gives the same
output, nothing touches the filesystem or the network, and there's no
hidden state to set up before you call something. That makes the whole
library trivial to unit test, which is the point.

## What it checks

- the street line is present and non-empty
- the state is a real two-letter USPS abbreviation (50 states, DC, and the
  inhabited territories), case-insensitive on input
- the postal code is a valid 5-digit zip or 9-digit zip+4

Anything that fails one of these comes back as a `ParseError` describing
which part was wrong, not a generic "invalid address."

## Usage

```rust
use addrkit::{parse_address, format_address, Address, ParseError};

fn main() {
    // Two-line form, the way it'd appear on an envelope.
    let addr = parse_address("123 Main St\nSpringfield, IL 62704").unwrap();
    assert_eq!(addr.city, "Springfield");
    assert_eq!(addr.state, "IL");

    // Single-line form works the same way.
    let same = parse_address("123 Main St, Springfield, IL 62704").unwrap();
    assert_eq!(addr, same);

    // Bad state abbreviations and malformed zips are rejected with a
    // specific reason instead of a generic error.
    match parse_address("123 Main St\nSpringfield, ZZ 62704") {
        Err(ParseError::UnrecognizedState(s)) => assert_eq!(s, "ZZ"),
        _ => unreachable!(),
    }

    // Printing goes the other direction, always producing the canonical
    // two-line form regardless of how the address was originally written.
    let built = Address {
        street: "1600 Pennsylvania Ave NW".to_string(),
        city: "Washington".to_string(),
        state: "DC".to_string(),
        postal_code: "20500".to_string(),
    };
    println!("{}", format_address(&built));
    // 1600 Pennsylvania Ave NW
    // Washington, DC 20500
}
```

## Status

Early skeleton. Handles standard single-address US mail formatting; it does
not yet handle apartment/suite lines as a separate field, PO boxes, or
non-US addresses. See the code for the current field-by-field validation
rules.

## Building

Standard library only, no dependencies:

```
cargo build
cargo test
```

## License

MIT, see [LICENSE](LICENSE).
