//! Signed exact report amounts. 18 fractional places; excessive precision rejects, never rounds.
use anyhow::{Result, ensure};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Decimal(i128);
impl Decimal {
    pub fn parse(value: &str) -> Result<Self> {
        ensure!(
            !value.is_empty() && value.trim() == value && value.len() <= 80,
            "Invalid exact billing amount"
        );
        let (negative, value) = value
            .strip_prefix('-')
            .map_or((false, value), |v| (true, v));
        let mut exponent = value.split(['e', 'E']);
        let base = exponent.next().unwrap();
        let power = exponent
            .next()
            .map(str::parse::<i32>)
            .transpose()
            .map_err(|_| anyhow::anyhow!("Invalid billing exponent"))?
            .unwrap_or(0);
        ensure!(
            exponent.next().is_none() && (-36..=36).contains(&power),
            "Billing amount exponent exceeds bound"
        );
        let mut parts = base.split('.');
        let whole = parts.next().unwrap();
        let fraction = parts.next().unwrap_or("");
        ensure!(
            parts.next().is_none()
                && !whole.is_empty()
                && whole.bytes().all(|b| b.is_ascii_digit())
                && fraction.bytes().all(|b| b.is_ascii_digit()),
            "Invalid exact billing amount"
        );
        let digits = format!("{whole}{fraction}");
        let mut number = digits
            .parse::<i128>()
            .map_err(|_| anyhow::anyhow!("Billing amount exceeds bound"))?;
        let scale = 18 + power - i32::try_from(fraction.len())?;
        if scale >= 0 {
            number = number
                .checked_mul(
                    10i128
                        .checked_pow(scale as u32)
                        .ok_or_else(|| anyhow::anyhow!("Billing amount exceeds bound"))?,
                )
                .ok_or_else(|| anyhow::anyhow!("Billing amount exceeds bound"))?;
        } else {
            let divisor = 10i128
                .checked_pow((-scale) as u32)
                .ok_or_else(|| anyhow::anyhow!("Billing precision exceeds bound"))?;
            ensure!(
                number % divisor == 0,
                "Billing amount precision exceeds 18 places"
            );
            number /= divisor;
        }
        if negative {
            number = number
                .checked_neg()
                .ok_or_else(|| anyhow::anyhow!("Billing amount exceeds bound"))?;
        }
        Ok(Self(number))
    }
    pub fn add(self, other: Self) -> Result<Self> {
        Ok(Self(self.0.checked_add(other.0).ok_or_else(|| {
            anyhow::anyhow!("Billing total exceeds bound")
        })?))
    }
    pub fn text(self) -> String {
        let n = self.0.unsigned_abs();
        let whole = n / 1_000_000_000_000_000_000;
        let fraction = n % 1_000_000_000_000_000_000;
        let sign = if self.0 < 0 { "-" } else { "" };
        if fraction == 0 {
            format!("{sign}{whole}")
        } else {
            format!(
                "{sign}{whole}.{}",
                format!("{fraction:018}").trim_end_matches('0')
            )
        }
    }
}
