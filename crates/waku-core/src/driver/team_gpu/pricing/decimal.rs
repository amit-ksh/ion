//! Exact nonnegative decimal amounts. Never pass provider prices through f64.
use anyhow::{Context, Result, ensure};
const ATTOS_PER_MICRO: u128 = 1_000_000_000_000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Amount(u128);
impl Amount {
    pub(super) const ZERO: Self = Self(0);
    pub(super) fn parse(value: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() && value.len() <= 96 && value.is_ascii(),
            "Price decimal is invalid"
        );
        let value = value.strip_prefix('+').unwrap_or(value);
        ensure!(
            !value.starts_with('-'),
            "Negative price cannot authorize GPU spending"
        );
        let mut parts = value.split(['e', 'E']);
        let mantissa = parts.next().context("Price decimal missing")?;
        let exponent = match parts.next() {
            Some(value) => {
                let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
                ensure!(
                    !digits.is_empty()
                        && digits.len() <= 3
                        && digits.bytes().all(|b| b.is_ascii_digit()),
                    "Price exponent is invalid"
                );
                value.parse::<i32>().context("Price exponent overflow")?
            }
            None => 0,
        };
        ensure!(
            parts.next().is_none() && (-38..=38).contains(&exponent),
            "Price exponent exceeds its bound"
        );
        let mut parts = mantissa.split('.');
        let integer = parts.next().context("Price integer missing")?;
        let fraction = parts.next().unwrap_or("");
        ensure!(
            parts.next().is_none()
                && !mantissa.is_empty()
                && (1..=38).contains(&(integer.len() + fraction.len()))
                && integer
                    .bytes()
                    .chain(fraction.bytes())
                    .all(|b| b.is_ascii_digit()),
            "Price decimal is invalid"
        );
        let coefficient = integer
            .bytes()
            .chain(fraction.bytes())
            .try_fold(0u128, |v, b| {
                v.checked_mul(10)
                    .and_then(|n| n.checked_add(u128::from(b - b'0')))
                    .context("Price coefficient overflow")
            })?;
        if coefficient == 0 {
            return Ok(Self::ZERO);
        }
        let shift = 18 + exponent - i32::try_from(fraction.len())?;
        let attos = if shift >= 0 {
            coefficient
                .checked_mul(
                    10u128
                        .checked_pow(shift as u32)
                        .context("Price precision overflow")?,
                )
                .context("Price amount overflow")?
        } else {
            let divisor = 10u128
                .checked_pow((-shift) as u32)
                .context("Price precision exceeds its bound")?;
            ensure!(
                coefficient % divisor == 0,
                "Price precision is not exactly representable"
            );
            coefficient / divisor
        };
        Ok(Self(attos))
    }
    pub(super) fn add(self, other: Self) -> Result<Self> {
        Ok(Self(
            self.0
                .checked_add(other.0)
                .context("Price total overflow")?,
        ))
    }
    pub(super) fn micros(self) -> Result<u64> {
        self.ratio_micros(1, 1)
    }
    pub(super) fn ratio_micros(self, numerator: u64, denominator: u64) -> Result<u64> {
        ensure!(denominator > 0, "Price unit denominator missing");
        let scaled = self
            .0
            .checked_mul(u128::from(numerator))
            .context("Price duration overflow")?;
        let divisor = ATTOS_PER_MICRO
            .checked_mul(u128::from(denominator))
            .context("Price unit overflow")?;
        let rounded = scaled / divisor + u128::from(scaled % divisor != 0);
        rounded
            .try_into()
            .context("Price micro-unit total overflow")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn team_gpu_price_decimal_exact_rounding_and_duration_never_underreserve() {
        for (value, micros) in [
            ("0", 0),
            ("0.00000001", 1),
            ("1.234567", 1_234_567),
            ("1.234567000000000001", 1_234_568),
            ("+1.2e-3", 1200),
            (".5", 500_000),
            ("1.", 1_000_000),
            ("12e2", 1_200_000_000),
            ("1.0000000000000000000", 1_000_000),
        ] {
            assert_eq!(
                Amount::parse(value).unwrap().micros().unwrap(),
                micros,
                "{value}"
            );
        }
        let rate = Amount::parse("0.123456789").unwrap();
        assert_eq!(rate.ratio_micros(1, 3600).unwrap(), 35);
        assert_eq!(rate.ratio_micros(3601, 3600).unwrap(), 123_492);
        assert_eq!(
            Amount::parse("0.000000000000000001")
                .unwrap()
                .ratio_micros(1, 3600)
                .unwrap(),
            1
        );
        assert_eq!(
            Amount::parse("1e-7")
                .unwrap()
                .add(Amount::parse("2e-7").unwrap())
                .unwrap()
                .micros()
                .unwrap(),
            1
        );
    }
    #[test]
    fn team_gpu_price_decimal_rejects_invalid_negative_precision_and_overflow() {
        for value in [
            "", " ", " 1", "1 ", "NaN", "Infinity", "-0", "-1", "+", ".", "1.2.3", "1e", "1e+",
            "1e1e2", "1e39", "1e-39", "1e-19", "1e38", "1_000", "1/2", "１", "++1",
        ] {
            assert!(Amount::parse(value).is_err(), "{value}");
        }
        let amount = Amount::parse("1e19").unwrap();
        assert!(amount.micros().is_err());
        assert!(amount.ratio_micros(u64::MAX, 1).is_err());
        assert!(Amount::ZERO.ratio_micros(1, 0).is_err());
        assert!(Amount(u128::MAX).add(Amount(1)).is_err());
    }
}
