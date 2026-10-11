// SPDX-License-Identifier: Apache-2.0
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Big {
    mantissa: f64,
    exponent: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BigError {
    #[error("`{0}` is not a number written as <mantissa>[e<exponent>]")]
    Malformed(String),
    #[error("`{0}` is not finite")]
    NotFinite(String),
    #[error("the exponent of `{0}` does not fit in 32 bits")]
    ExponentOverflow(String),
}

impl Big {
    pub const ZERO: Big = Big {
        mantissa: 0.0,
        exponent: 0,
    };

    pub fn new(mantissa: f64, exponent: i32) -> Result<Big, BigError> {
        let shown = format!("{mantissa}e{exponent}");
        if !mantissa.is_finite() {
            return Err(BigError::NotFinite(shown));
        }
        if mantissa == 0.0 {
            return Ok(Big::ZERO);
        }

        let scientific = format!("{mantissa:e}");
        let (digits, shift) = scientific
            .split_once('e')
            .ok_or_else(|| BigError::Malformed(shown.clone()))?;
        let shift: i32 = shift
            .parse()
            .map_err(|_| BigError::Malformed(shown.clone()))?;
        let mantissa: f64 = digits
            .parse()
            .map_err(|_| BigError::Malformed(shown.clone()))?;
        let exponent = exponent
            .checked_add(shift)
            .ok_or(BigError::ExponentOverflow(shown))?;

        Ok(Big { mantissa, exponent })
    }

    pub fn from_f64(value: f64) -> Option<Big> {
        Big::new(value, 0).ok()
    }

    pub fn mantissa(self) -> f64 {
        self.mantissa
    }

    pub fn exponent(self) -> i32 {
        self.exponent
    }

    pub fn to_f64(self) -> f64 {
        self.to_string()
            .parse()
            .expect("a normalised Big always prints as a float")
    }

    fn sign(self) -> i8 {
        if self.mantissa > 0.0 {
            1
        } else if self.mantissa < 0.0 {
            -1
        } else {
            0
        }
    }

    pub fn compare(self, other: Big) -> Ordering {
        match self.sign().cmp(&other.sign()) {
            Ordering::Equal => {}
            different => return different,
        }
        if self.sign() == 0 {
            return Ordering::Equal;
        }
        let magnitude = self
            .exponent
            .cmp(&other.exponent)
            .then(self.mantissa.abs().total_cmp(&other.mantissa.abs()));
        if self.sign() < 0 {
            magnitude.reverse()
        } else {
            magnitude
        }
    }
}

impl fmt::Display for Big {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}e{}", self.mantissa, self.exponent)
    }
}

impl FromStr for Big {
    type Err = BigError;

    fn from_str(text: &str) -> Result<Big, BigError> {
        let trimmed = text.trim();
        let (mantissa, exponent) = match trimmed.rfind(['e', 'E']) {
            Some(at) => (&trimmed[..at], Some(&trimmed[at + 1..])),
            None => (trimmed, None),
        };
        if mantissa.contains(['e', 'E']) {
            return Err(BigError::Malformed(text.to_owned()));
        }
        let mantissa: f64 = mantissa
            .parse()
            .map_err(|_| BigError::Malformed(text.to_owned()))?;
        let exponent: i32 = match exponent {
            Some(raw) => raw.parse().map_err(|error: std::num::ParseIntError| {
                use std::num::IntErrorKind::{NegOverflow, PosOverflow};
                match error.kind() {
                    PosOverflow | NegOverflow => BigError::ExponentOverflow(text.to_owned()),
                    _ => BigError::Malformed(text.to_owned()),
                }
            })?,
            None => 0,
        };
        if !mantissa.is_finite() {
            return Err(BigError::NotFinite(text.to_owned()));
        }
        Big::new(mantissa, exponent).map_err(|error| match error {
            BigError::ExponentOverflow(_) => BigError::ExponentOverflow(text.to_owned()),
            other => other,
        })
    }
}

impl TryFrom<String> for Big {
    type Error = BigError;

    fn try_from(text: String) -> Result<Big, BigError> {
        text.parse()
    }
}

impl From<Big> for String {
    fn from(big: Big) -> String {
        big.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::{Equal, Greater, Less};

    fn big(text: &str) -> Big {
        text.parse().unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn the_notations_a_mod_can_produce_all_parse() {
        assert_eq!(big("1.5e+400"), Big::new(1.5, 400).unwrap());
        assert_eq!(big("-2E-3"), Big::new(-2.0, -3).unwrap());
        assert_eq!(big("42"), Big::new(4.2, 1).unwrap());
        assert_eq!(big(" 7e2 "), big("7e2"));
    }

    #[test]
    fn the_same_quantity_written_differently_is_one_value() {
        assert_eq!(big("15e399"), big("1.5e400"));
        assert_eq!(big("0.03e5"), big("3e3"));
        assert_eq!(big("0e50"), big("-0"));
        assert_eq!(
            big("0e50").exponent(),
            0,
            "zero has one spelling, or two zeros would differ"
        );
    }

    #[test]
    fn malformed_and_non_finite_text_is_refused() {
        for text in [
            "",
            "abc",
            "e5",
            "1e",
            "1e5e3",
            "1.5.2e3",
            "NaN",
            "inf",
            "-infinity",
        ] {
            assert!(text.parse::<Big>().is_err(), "`{text}` must not parse");
        }
        assert!(matches!("NaN".parse::<Big>(), Err(BigError::NotFinite(_))));
    }

    #[test]
    fn an_exponent_that_overflows_is_refused_not_wrapped() {
        assert!(matches!(
            "1e99999999999".parse::<Big>(),
            Err(BigError::ExponentOverflow(_))
        ));
        assert!(matches!(
            "15e2147483647".parse::<Big>(),
            Err(BigError::ExponentOverflow(_))
        ));
        assert!("1e2147483647".parse::<Big>().is_ok());
    }

    #[test]
    fn printing_round_trips_through_parsing() {
        for text in [
            "1.5e400",
            "-9.999e-320",
            "1e0",
            "0e0",
            "7.123456789012345e18",
        ] {
            let value = big(text);
            assert_eq!(big(&value.to_string()), value, "{text}");
        }
    }

    #[test]
    fn ordering_works_across_signs_exponents_and_mantissas() {
        assert_eq!(big("2e400").compare(big("1e400")), Greater);
        assert_eq!(big("9e399").compare(big("1e400")), Less);
        assert_eq!(big("-2e400").compare(big("-1e400")), Less);
        assert_eq!(big("-1e400").compare(big("1e-400")), Less);
        assert_eq!(big("1e-400").compare(Big::ZERO), Greater);
        assert_eq!(big("-1e-400").compare(Big::ZERO), Less);
        assert_eq!(big("1.5e3").compare(big("1500")), Equal);
    }

    #[test]
    fn converting_to_f64_saturates_outside_its_range() {
        assert_eq!(big("1.5e3").to_f64(), 1500.0);
        assert_eq!(big("1e400").to_f64(), f64::INFINITY);
        assert_eq!(big("1e-400").to_f64(), 0.0);
    }

    #[test]
    fn what_the_csharp_and_javascript_bridges_write_is_accepted() {
        let from_csharp = ["1.5e400", "1e0", "-2.5e-3", "3e3", "1.2e2"];
        let from_javascript = ["1.5e+400", "2.5e1000", "42", "0.5", "-7E-2"];

        for text in from_csharp.into_iter().chain(from_javascript) {
            assert!(text.parse::<Big>().is_ok(), "`{text}` must parse");
        }
        assert_eq!(big("1.2e2"), big("120"));
    }

    #[test]
    fn json_carries_it_as_a_string_and_refuses_a_bad_one() {
        let json = serde_json::to_string(&big("1.5e400")).unwrap();
        assert_eq!(json, "\"1.5e400\"");
        assert_eq!(serde_json::from_str::<Big>(&json).unwrap(), big("1.5e400"));
        assert!(serde_json::from_str::<Big>("\"nan\"").is_err());
        assert!(serde_json::from_str::<Big>("1.5").is_err());
    }
}
