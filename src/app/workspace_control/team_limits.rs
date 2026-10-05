//! Reusable defaults are edited locally and validated again by the coordinator.
use super::*;
use waku_client::teams::TeamRunLimits;
#[derive(Clone, Copy)]
pub(super) enum LimitField {
    Work,
    Parallel,
    Depth,
    Attempts,
    Calls,
    Tokens,
    Rework,
    Timeout,
}
impl LimitField {
    fn label(self) -> &'static str {
        match self {
            Self::Work => "Maximum assignments",
            Self::Parallel => "Parallel assignments",
            Self::Depth => "Delegation depth",
            Self::Attempts => "Attempts per assignment",
            Self::Calls => "Model calls",
            Self::Tokens => "Token limit (blank for no cap)",
            Self::Rework => "Correction rounds",
            Self::Timeout => "Timeout (minutes)",
        }
    }
    fn value(self, limits: &TeamRunLimits) -> String {
        match self {
            Self::Work => limits.max_work_items.to_string(),
            Self::Parallel => limits.max_parallel_tasks.to_string(),
            Self::Depth => limits.max_depth.to_string(),
            Self::Attempts => limits.max_attempts.to_string(),
            Self::Calls => limits.max_model_calls.to_string(),
            Self::Tokens => limits.max_tokens.map(|n| n.to_string()).unwrap_or_default(),
            Self::Rework => limits.max_rework_rounds.to_string(),
            Self::Timeout => {
                // A legacy non-minute timeout remains exact when the field is untouched.
                if limits.timeout_seconds % 60 == 0 {
                    (limits.timeout_seconds / 60).to_string()
                } else {
                    format!("{}s", limits.timeout_seconds)
                }
            }
        }
    }
}
pub(super) fn inputs(window: &mut Window, cx: &mut App) -> Vec<(LimitField, Entity<TextInput>)> {
    [
        LimitField::Work,
        LimitField::Parallel,
        LimitField::Depth,
        LimitField::Attempts,
        LimitField::Calls,
        LimitField::Tokens,
        LimitField::Rework,
        LimitField::Timeout,
    ]
    .into_iter()
    .map(|field| {
        (
            field,
            cx.new(|cx| TextInput::new(window, cx).placeholder(field.label())),
        )
    })
    .collect()
}
impl Waku {
    pub(super) fn set_team_limit_inputs(&mut self, limits: TeamRunLimits, cx: &mut Context<Self>) {
        self.control_ui.child_review = limits.require_child_review;
        for (field, input) in &self.control_ui.limit_inputs {
            input.update(cx, |input, cx| input.set_content(field.value(&limits), cx));
        }
    }
    pub(super) fn read_team_limit_inputs(&self, cx: &App) -> Result<TeamRunLimits, String> {
        let mut limits = TeamRunLimits {
            require_child_review: self.control_ui.child_review,
            ..Default::default()
        };
        for (field, input) in &self.control_ui.limit_inputs {
            let text = input.read(cx).content().trim();
            if matches!(field, LimitField::Tokens) && text.is_empty() {
                limits.max_tokens = None;
                continue;
            }
            let (text, multiplier) = if matches!(field, LimitField::Timeout) {
                if let Some(seconds) = text.strip_suffix('s') {
                    (seconds, 1)
                } else {
                    (text, 60)
                }
            } else {
                (text, 1)
            };
            let number = text
                .parse::<u64>()
                .ok()
                .and_then(|n| n.checked_mul(multiplier))
                .ok_or_else(|| format!("{}: enter a whole number.", field.label()))?;
            macro_rules! bounded {
                ($range:expr,$hint:literal) => {
                    if !$range.contains(&number) {
                        return Err(format!("{}: {}", field.label(), $hint));
                    }
                };
            }
            match field {
                LimitField::Work => {
                    bounded!(1..=256, "use 1–256");
                    limits.max_work_items = number as u16;
                }
                LimitField::Parallel => {
                    bounded!(1..=16, "use 1–16");
                    limits.max_parallel_tasks = number as u16;
                }
                LimitField::Depth => {
                    bounded!(1..=16, "use 1–16");
                    limits.max_depth = number as u16;
                }
                LimitField::Attempts => {
                    bounded!(1..=8, "use 1–8");
                    limits.max_attempts = number as u16;
                }
                LimitField::Calls => {
                    bounded!(1..=4096, "use 1–4096");
                    limits.max_model_calls = number as u16;
                }
                LimitField::Tokens => {
                    bounded!(
                        1..=1_000_000_000,
                        "use 1–1,000,000,000, or clear for no cap"
                    );
                    limits.max_tokens = Some(number);
                }
                LimitField::Rework => {
                    bounded!(0..=8, "use 0–8");
                    limits.max_rework_rounds = number as u16;
                }
                LimitField::Timeout => {
                    bounded!(60..=604800, "use 1–10,080 minutes");
                    limits.timeout_seconds = number as u32;
                }
            }
        }
        Ok(limits)
    }
    pub(super) fn render_team_limit_inputs(&self, cx: &mut Context<Self>) -> Div {
        let theme = Theme::current(cx);
        let mut form = div().flex().flex_col().gap(px(10.0)).child(
            div()
                .text_size(sp(14.0))
                .font_weight(FontWeight::MEDIUM)
                .child("Run limits"),
        );
        for (index, (field, input)) in self.control_ui.limit_inputs.iter().enumerate() {
            form = form.child(control_field(
                field.label(),
                input.clone(),
                format!("team-limit-{index}"),
            ));
        }
        let enabled = self.control_ui.child_review;
        form.child(self.control_button("team-limit-review",if enabled{"Parent review required"}else{"Parent review optional"},cx,|this,_,cx| {this.control_ui.child_review=!this.control_ui.child_review;cx.notify();}))
            .child(div().text_size(sp(12.5)).text_color(theme.text_secondary).child("Changes apply to new runs. Token limits cover model usage; cloud costs are separate."))
    }
}
