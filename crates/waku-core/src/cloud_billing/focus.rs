//! Bounded archive/CSV interpretation. No extraction or subprocess; preserve all charges.
use super::*;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use decimal::Decimal;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    io::Read,
    path::{Component, PathBuf},
};
const MAX_EXPANDED: usize = 32 * 1024 * 1024;
pub(super) struct Parsed {
    pub charges: Vec<CloudBillingCharge>,
    pub billed: String,
    pub effective: String,
    pub contracted: String,
    pub missing_resources: u64,
    pub csv_hash: String,
}
pub(super) fn period_bounds(period: &str) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    ensure!(
        period.len() == 7 && period.as_bytes()[4] == b'-',
        "Use a single billing month in YYYY-MM form"
    );
    let start = NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d")
        .context("Invalid billing month")?;
    ensure!(
        start.format("%Y-%m").to_string() == period,
        "Invalid billing month"
    );
    let (year, month) = if start.month() == 12 {
        (start.year() + 1, 1)
    } else {
        (start.year(), start.month() + 1)
    };
    let end = NaiveDate::from_ymd_opt(year, month, 1).context("Billing month exceeds bound")?;
    Ok((
        Utc.from_utc_datetime(&start.and_hms_opt(0, 0, 0).unwrap()),
        Utc.from_utc_datetime(&end.and_hms_opt(0, 0, 0).unwrap()),
    ))
}
fn instant(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .map_err(|_| anyhow::anyhow!("Invalid billing period timestamp"))?
        .with_timezone(&Utc))
}
fn optional(value: &str) -> Option<String> {
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        None
    } else {
        Some(value.to_owned())
    }
}
pub(super) fn parse(scope: &CloudBillingScope, period: &str, archive: &[u8]) -> Result<Parsed> {
    let mut expanded = Vec::new();
    flate2::read::MultiGzDecoder::new(archive)
        .take((MAX_EXPANDED + 1) as u64)
        .read_to_end(&mut expanded)
        .map_err(|_| anyhow::anyhow!("Invalid billing gzip archive"))?;
    ensure!(
        expanded.len() <= MAX_EXPANDED,
        "Billing archive expansion exceeds 32 MiB"
    );
    let mut tar = tar::Archive::new(std::io::Cursor::new(expanded));
    let (start, end) = period_bounds(period)?;
    let mut charges = vec![];
    let mut billed = Decimal::default();
    let mut effective = Decimal::default();
    let mut contracted = Decimal::default();
    let mut hash = Sha256::new();
    let mut paths = HashSet::new();
    let mut files = 0;
    for (entry_index, entry) in tar
        .entries()
        .map_err(|_| anyhow::anyhow!("Invalid billing tar archive"))?
        .enumerate()
    {
        ensure!(entry_index < 128, "Billing archive has too many entries");
        let entry = entry.map_err(|_| anyhow::anyhow!("Invalid billing archive entry"))?;
        let path = entry
            .path()
            .map_err(|_| anyhow::anyhow!("Invalid billing archive path"))?
            .into_owned();
        ensure!(
            path.components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
                && path.as_os_str().len() <= 512,
            "Billing archive contains an unsafe path"
        );
        let path: PathBuf = path
            .components()
            .filter_map(|c| {
                if let Component::Normal(part) = c {
                    Some(part)
                } else {
                    None
                }
            })
            .collect();
        if entry.header().entry_type().is_dir() {
            continue;
        }
        ensure!(
            entry.header().entry_type().is_file() && paths.insert(path.clone()),
            "Billing archive has a link, duplicate or unsupported entry"
        );
        let is_csv = path.extension().is_some_and(|e| e == "csv");
        ensure!(
            is_csv,
            "Billing archive contains an unsupported non-CSV file"
        );
        files += 1;
        let mut data = Vec::new();
        entry
            .take((MAX_EXPANDED + 1) as u64)
            .read_to_end(&mut data)
            .map_err(|_| anyhow::anyhow!("Could not read billing CSV"))?;
        ensure!(data.len() <= MAX_EXPANDED, "Billing CSV exceeds bound");
        let path = path.to_str().context("Billing archive path is not UTF-8")?;
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update((data.len() as u64).to_le_bytes());
        hash.update(&data);
        let mut reader = csv::ReaderBuilder::new()
            .flexible(false)
            .from_reader(data.as_slice());
        let header = reader
            .headers()
            .map_err(|_| anyhow::anyhow!("Invalid billing CSV header"))?
            .clone();
        ensure!(
            header.len() <= 128 && header.iter().collect::<HashSet<_>>().len() == header.len(),
            "Billing CSV header is duplicated or too large"
        );
        let columns = [
            "BillingAccountId",
            "BillingCurrency",
            "BillingPeriodStart",
            "BillingPeriodEnd",
            "ChargePeriodStart",
            "ChargePeriodEnd",
            "BilledCost",
            "EffectiveCost",
            "ContractedCost",
            "ChargeCategory",
            "ChargeClass",
            "x_ContainerId",
            "ResourceId",
            "ResourceType",
        ];
        let indices = columns
            .iter()
            .map(|name| {
                header
                    .iter()
                    .position(|h| h == *name)
                    .ok_or_else(|| anyhow::anyhow!("Billing CSV is missing a required FOCUS field"))
            })
            .collect::<Result<Vec<_>>>()?;
        for record in reader.records() {
            ensure!(
                charges.len() < 100_000,
                "Billing report exceeds 100,000 rows"
            );
            let record = record.map_err(|_| anyhow::anyhow!("Invalid billing CSV row"))?;
            ensure!(
                record.iter().all(|v| v.len() <= 16_384
                    && !v
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))),
                "Billing field exceeds its bound"
            );
            let cell = |i: usize| record.get(indices[i]).unwrap_or_default();
            ensure!(
                cell(0) == scope.account_id && cell(1) == scope.currency,
                "Billing account or currency differs from the selected scope"
            );
            ensure!(
                instant(cell(2))? == start && instant(cell(3))? == end,
                "Billing CSV contains another or incomplete billing period"
            );
            let charge_start = instant(cell(4))?;
            let charge_end = instant(cell(5))?;
            ensure!(
                charge_end > charge_start,
                "Billing charge period is invalid"
            );
            let b = Decimal::parse(cell(6))?;
            let e = Decimal::parse(cell(7))?;
            let c = Decimal::parse(cell(8))?;
            billed = billed.add(b)?;
            effective = effective.add(e)?;
            contracted = contracted.add(c)?;
            ensure!(
                !cell(9).is_empty()
                    && cell(9).len() <= 80
                    && !cell(11).is_empty()
                    && cell(11).len() <= 160,
                "Billing category or hierarchy identity is missing"
            );
            ensure!(
                cell(10).len() <= 80 && cell(12).len() <= 160 && cell(13).len() <= 160,
                "Billing attribution exceeds its bound"
            );
            charges.push(CloudBillingCharge {
                ordinal: charges.len() as u64 + 1,
                container_id: cell(11).into(),
                resource_id: optional(cell(12)),
                resource_type: optional(cell(13)),
                category: cell(9).into(),
                class: optional(cell(10)),
                charge_period_start: charge_start.to_rfc3339(),
                charge_period_end: charge_end.to_rfc3339(),
                billed_cost: b.text(),
                effective_cost: e.text(),
                contracted_cost: c.text(),
            });
        }
    }
    // Reject concatenated/hidden tar payloads after the end marker; no files are extracted.
    let cursor = tar.into_inner();
    let at = cursor.position() as usize;
    ensure!(
        cursor.get_ref()[at..].iter().all(|b| *b == 0),
        "Billing archive has trailing nonzero data"
    );
    ensure!(
        files > 0 && !charges.is_empty(),
        "Billing export has no account charges to verify"
    );
    let missing_resources = charges.iter().filter(|c| c.resource_id.is_none()).count() as u64;
    Ok(Parsed {
        charges,
        billed: billed.text(),
        effective: effective.text(),
        contracted: contracted.text(),
        missing_resources,
        csv_hash: format!("{:x}", hash.finalize()),
    })
}
