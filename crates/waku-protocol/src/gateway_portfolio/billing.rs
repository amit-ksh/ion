//! Billing projections contain saved report evidence, never connection or resource identities.
use super::*;
use crate::cloud_billing::{CloudBillingChargePage, CloudBillingPage, CloudBillingReportPage};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillingReportsQuery {
    pub scope_id: Uuid,
    pub after: Option<String>,
    pub limit: u16,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BillingChargesQuery {
    pub scope_id: Uuid,
    pub export_id: String,
    pub after: Option<u64>,
    pub limit: u16,
}
fn identifier(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'),
        "Invalid billing export identity"
    );
    Ok(())
}
impl BillingReportsQuery {
    pub fn validate(&self) -> Result<()> {
        if let Some(after) = &self.after {
            identifier(after)?;
        }
        Ok(())
    }
}
impl BillingChargesQuery {
    pub fn validate(&self) -> Result<()> {
        identifier(&self.export_id)?;
        ensure!(
            self.after.is_none_or(|a| (1..=100_000).contains(&a)),
            "Invalid billing charge cursor"
        );
        Ok(())
    }
}
fn bounded_billing(mut value: Value, rows: &str, cursor: &str) -> Result<Value> {
    while value.to_string().len() > 60_000 {
        let entries = value["page"][rows]
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("Invalid billing page"))?;
        entries.pop();
        let last = value["page"][rows]
            .as_array()
            .and_then(|e| e.last())
            .and_then(|r| r.get(cursor))
            .cloned();
        ensure!(last.is_some(), "Billing row exceeds gateway transfer bound");
        value["page"]["nextCursor"] = last.unwrap();
        value["pageTruncated"] = true.into();
    }
    Ok(value)
}
pub fn accounts_result(page: CloudBillingPage, served_at: u64) -> Result<Value> {
    let scopes: Vec<_> = page.scopes.into_iter().map(|row| json!({"id":row.scope.id,"name":label(&row.scope.name),"currency":row.scope.currency,"revision":row.revision,"reports":row.reports})).collect();
    bounded_billing(
        json!({"kind":"cloudBilling","page":{"scopes":scopes,"nextCursor":page.next_cursor,"total":page.total},"pageTruncated":false,"servedAt":served_at}),
        "scopes",
        "id",
    )
}
pub fn reports_result(page: CloudBillingReportPage, served_at: u64) -> Result<Value> {
    bounded_billing(
        json!({"kind":"cloudBillingReports","page":page,"pageTruncated":false,"servedAt":served_at}),
        "reports",
        "exportId",
    )
}
pub fn charges_result(page: CloudBillingChargePage, served_at: u64) -> Result<Value> {
    let charges: Vec<_> = page.charges.into_iter().map(|row| json!({"ordinal":row.ordinal,"resourceType":row.resource_type.as_deref().map(label),"category":label(&row.category),"class":row.class.as_deref().map(label),"chargePeriodStart":row.charge_period_start,"chargePeriodEnd":row.charge_period_end,"billedCost":row.billed_cost,"effectiveCost":row.effective_cost,"contractedCost":row.contracted_cost})).collect();
    bounded_billing(
        json!({"kind":"cloudBillingCharges","page":{"report":page.report,"charges":charges,"nextCursor":page.next_cursor},"pageTruncated":false,"servedAt":served_at}),
        "charges",
        "ordinal",
    )
}
#[cfg(test)]
mod tests;
