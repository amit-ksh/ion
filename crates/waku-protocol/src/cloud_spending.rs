//! Owner-approved shared GPU execution budgets. Holds are not provider invoices.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudBudget {
    pub id: Uuid,
    pub name: String,
    pub cloud_project: String,
    pub targets: Vec<String>,
    pub currency: String,
    pub limit_micros: u64,
    pub paused: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBudgetStatus {
    pub budget: CloudBudget,
    pub held_micros: u64,
    pub available_micros: u64,
    pub requests: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CloudBudgetPage {
    pub budgets: Vec<CloudBudgetStatus>,
    pub next_cursor: Option<Uuid>,
    pub total: u64,
}
pub fn format_micros(micros: u64) -> String {
    format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000)
}
pub fn parse_micros(text: &str) -> Result<u64, &'static str> {
    let text = text.trim();
    if text.is_empty() || text.len() > 20 {
        return Err("Enter an amount with at most six decimal places");
    }
    let mut pieces = text.split('.');
    let whole = pieces.next().unwrap();
    let fraction = pieces.next().unwrap_or("");
    if pieces.next().is_some()
        || whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err("Enter an amount with at most six decimal places");
    }
    let whole = whole
        .parse::<u64>()
        .map_err(|_| "Budget amount is too large")?;
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction
            .parse::<u64>()
            .map_err(|_| "Invalid budget amount")?
            * 10u64.pow(6 - fraction.len() as u32)
    };
    let micros = whole
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(fraction))
        .ok_or("Budget amount is too large")?;
    if !(1..=1_000_000_000_000).contains(&micros) {
        return Err("Budget must be positive and no more than 1,000,000 currency units");
    }
    Ok(micros)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monetary_input_uses_exact_integer_micro_units_without_rounding() {
        for (text, value) in [
            ("0.000001", 1),
            ("1.234567", 1_234_567),
            ("1000000", 1_000_000_000_000),
            (" 01.20 ", 1_200_000),
        ] {
            assert_eq!(parse_micros(text).unwrap(), value);
        }
        for text in [
            "0",
            "-1",
            "NaN",
            "1e6",
            ".5",
            "1.2.3",
            "1.0000001",
            "1000000.000001",
            "18446744073709551615",
        ] {
            assert!(parse_micros(text).is_err(), "{text}");
        }
    }
}
