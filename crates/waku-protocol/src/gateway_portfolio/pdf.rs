//! Read-only derived image chunks. No raw paths, credentials or arbitrary render options.
use super::*;
use ts_rs::TS;
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowPdfDocument {
    pub id: Uuid,
    pub name: String,
    pub format: String,
    pub project_id: Uuid,
    pub task_id: Option<Uuid>,
    pub stage: u16,
    pub team_run_id: Uuid,
    pub assignment_id: Uuid,
    pub bytes: u64,
    pub sha256: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowPdfQuery {
    pub run_id: Uuid,
    pub revision: u64,
    pub expected: WorkflowPdfDocument,
    pub page: u32,
    pub width: u32,
    pub transfer_id: Option<Uuid>,
    pub offset: u32,
    pub limit: u16,
}
impl WorkflowPdfQuery {
    pub fn validate(&self) -> Result<()> {
        let e = &self.expected;
        ensure!(
            !self.run_id.is_nil()
                && !e.id.is_nil()
                && !e.project_id.is_nil()
                && !e.team_run_id.is_nil()
                && !e.assignment_id.is_nil()
                && e.task_id.is_none_or(|id| !id.is_nil())
                && e.format == "pdf"
                && e.bytes > 0
                && e.bytes <= 65_536
                && e.name.chars().count() <= 80
                && e.stage < 24
                && e.sha256.len() == 64
                && e.sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                && self.page < 32
                && [800, 1200, 1600].contains(&self.width)
                && self.limit == 32_768
                && self.offset < 12 * 1024 * 1024
                && self.offset % 32_768 == 0
                && match self.transfer_id {
                    None => self.offset == 0,
                    Some(id) => !id.is_nil() && self.offset > 0,
                },
            "Select exact accepted PDF evidence and a supported page, zoom or image chunk"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> WorkflowPdfQuery {
        WorkflowPdfQuery {
            run_id: Uuid::new_v4(),
            revision: 3,
            expected: WorkflowPdfDocument {
                id: Uuid::new_v4(),
                name: "Report.pdf".into(),
                format: "pdf".into(),
                project_id: Uuid::new_v4(),
                task_id: None,
                stage: 0,
                team_run_id: Uuid::new_v4(),
                assignment_id: Uuid::new_v4(),
                bytes: 200,
                sha256: "a".repeat(64),
                created_at: 100,
            },
            page: 0,
            width: 800,
            transfer_id: None,
            offset: 0,
            limit: 32768,
        }
    }
    #[test]
    fn pdf_query_accepts_only_original_selection_supported_pages_and_exact_chunk_offsets() {
        let q = query();
        q.validate().unwrap();
        for change in 0..10 {
            let mut invalid = q.clone();
            match change {
                0 => invalid.run_id = Uuid::nil(),
                1 => invalid.expected.assignment_id = Uuid::nil(),
                2 => invalid.expected.bytes = 65537,
                3 => invalid.expected.format = "html".into(),
                4 => invalid.page = 32,
                5 => invalid.width = 2000,
                6 => invalid.limit = 32769,
                7 => invalid.offset = 32768,
                8 => invalid.expected.sha256 = "bad".into(),
                _ => invalid.expected.name = "a".repeat(81),
            };
            assert!(invalid.validate().is_err());
        }
        let mut next = q;
        next.offset = 32768;
        next.transfer_id = Some(Uuid::new_v4());
        next.validate().unwrap();
        next.offset += 1;
        assert!(next.validate().is_err());
    }
    #[test]
    fn pdf_query_refuses_effects_paths_private_fields_and_unknown_render_options() {
        let q = query();
        let mut value = serde_json::to_value(&q).unwrap();
        value["path"] = "/private/file".into();
        assert!(serde_json::from_value::<WorkflowPdfQuery>(value).is_err());
        let mut value = serde_json::to_value(&q).unwrap();
        value["expected"]["sourcePath"] = "/private/file".into();
        assert!(serde_json::from_value::<WorkflowPdfQuery>(value).is_err());
        let portfolio = PortfolioQuery::WorkflowPdfPage { query: q };
        portfolio.validate().unwrap();
        assert!(serde_json::to_value(portfolio).unwrap().to_string().len() < 4000);
    }
}
