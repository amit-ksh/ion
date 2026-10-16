//! Property-role evidence for animation names; substitution stays native.
use super::*;

impl Globals {
    pub(super) fn animation(
        &self,
        source: &str,
        prefix: &str,
        shorthand: bool,
        depth: usize,
    ) -> Result<String> {
        ensure!(depth <= 32, "Computed animation nesting exceeds its bound");
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        let mut out = String::new();
        let mut consumed = Animation::default();
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            match token {
                CssToken::Comma => {
                    consumed = Animation::default();
                    push(&mut out, ",")?;
                }
                CssToken::Ident(name) => {
                    let reserved = if shorthand {
                        consumed.keyword(&name) != Role::Name
                    } else {
                        wide(&name) || name.eq_ignore_ascii_case("none")
                    };
                    if !reserved {
                        push(&mut out, &identifier(&name, prefix, "animation"))?;
                    } else {
                        push(&mut out, &CssToken::Ident(name).to_css_string())?;
                    }
                }
                CssToken::QuotedString(name) => {
                    push(&mut out, &identifier(&name, prefix, "animation"))?
                }
                CssToken::Number { .. } => {
                    consumed.iteration = Slot::Used;
                    push(&mut out, &token.to_css_string())?;
                }
                CssToken::Function(name) => {
                    let body = nested_text(&mut parser)?;
                    let body = if shorthand && name.eq_ignore_ascii_case("ident") {
                        consumed.computed(&body, prefix)?
                    } else if !shorthand && name.eq_ignore_ascii_case("ident") {
                        if let Some(value) = computed_names::literal(&body)? {
                            if computed_names::reserved(&value, &["none"]) {
                                body
                            } else {
                                computed_names::quoted(&format!("{prefix}animation_{value}"))
                            }
                        } else {
                            computed_names::affix(&body, prefix, "animation", false, &["none"])?
                                .unwrap_or(body)
                        }
                    } else if !shorthand && name.eq_ignore_ascii_case("var") {
                        if let Some(offset) = computed_names::fallback_offset(&body) {
                            format!(
                                "{}{}",
                                &body[..offset],
                                self.animation(&body[offset..], prefix, false, depth + 1)?
                            )
                        } else {
                            body
                        }
                    } else {
                        body
                    };
                    if matches!(
                        name.to_ascii_lowercase().as_str(),
                        "linear" | "steps" | "cubic-bezier"
                    ) {
                        consumed.timing = Slot::Used;
                    } else if shorthand && !name.eq_ignore_ascii_case("ident") {
                        // Arbitrary substitutions may insert commas and reset
                        // any preceding slot. Never guess a later keyword role.
                        consumed.uncertain_substitution();
                    }
                    push(
                        &mut out,
                        &format!("{}({})", CssToken::Ident(name).to_css_string(), body),
                    )?;
                }
                CssToken::ParenthesisBlock => {
                    push(&mut out, &format!("({})", nested_text(&mut parser)?))?
                }
                CssToken::SquareBracketBlock => {
                    push(&mut out, &format!("[{}]", nested_text(&mut parser)?))?
                }
                CssToken::CurlyBracketBlock => {
                    push(&mut out, &format!("{{{}}}", nested_text(&mut parser)?))?
                }
                _ => push(&mut out, &token.to_css_string())?,
            }
        }
        Ok(out)
    }
}
#[derive(Clone, Copy, Default)]
enum Slot {
    #[default]
    Free,
    Used,
    Unknown,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Keyword,
    Name,
    Unknown,
}
#[derive(Default)]
struct Animation {
    timing: Slot,
    iteration: Slot,
    direction: Slot,
    fill: Slot,
    play: Slot,
}
impl Animation {
    fn uncertain_substitution(&mut self) {
        self.timing = Slot::Unknown;
        self.iteration = Slot::Unknown;
        self.direction = Slot::Unknown;
        self.fill = Slot::Unknown;
        self.play = Slot::Unknown;
    }
    fn uncertain_identifier(&mut self) {
        // One ident() cannot insert a comma, a number, or a function, nor
        // undo an already filled keyword slot. Its result may fill a free one.
        for slot in [
            &mut self.timing,
            &mut self.iteration,
            &mut self.direction,
            &mut self.fill,
            &mut self.play,
        ] {
            if matches!(slot, Slot::Free) {
                *slot = Slot::Unknown;
            }
        }
    }
    fn keyword(&mut self, word: &str) -> Role {
        if wide(word) {
            return Role::Keyword;
        }
        if word.eq_ignore_ascii_case("none") {
            // The other-property keyword wins before animation-name: e.g.
            // "1s none both" names the animation "both".
            self.fill = Slot::Used;
            return Role::Keyword;
        }
        let slot = match word.to_ascii_lowercase().as_str() {
            "ease" | "linear" | "ease-in" | "ease-out" | "ease-in-out" | "step-start"
            | "step-end" => &mut self.timing,
            "infinite" => &mut self.iteration,
            "normal" | "reverse" | "alternate" | "alternate-reverse" => &mut self.direction,
            "forwards" | "backwards" | "both" => &mut self.fill,
            "running" | "paused" => &mut self.play,
            _ => return Role::Name,
        };
        let role = match slot {
            Slot::Free => Role::Keyword,
            Slot::Used => Role::Name,
            Slot::Unknown => Role::Unknown,
        };
        *slot = Slot::Used;
        role
    }
    fn computed(&mut self, body: &str, prefix: &str) -> Result<String> {
        if let Some(value) = computed_names::literal(body)? {
            // New duration/timeline grammar needs a separate browser contract;
            // do not turn those computed roles into an ordinary keyframe name.
            if value.eq_ignore_ascii_case("auto") || value.starts_with("--") {
                self.uncertain_identifier();
                return Ok(body.into());
            }
            return Ok(if self.keyword(&value) == Role::Name {
                computed_names::quoted(&format!("{prefix}animation_{value}"))
            } else {
                body.into()
            });
        }
        let keywords = [
            "none",
            "auto",
            "ease",
            "linear",
            "ease-in",
            "ease-out",
            "ease-in-out",
            "step-start",
            "step-end",
            "infinite",
            "normal",
            "reverse",
            "alternate",
            "alternate-reverse",
            "forwards",
            "backwards",
            "both",
            "running",
            "paused",
        ];
        let mut input = ParserInput::new(body);
        let mut parser = Parser::new(&mut input);
        let first = match parser.next().cloned() {
            Ok(CssToken::Ident(value) | CssToken::QuotedString(value)) => Some(value.to_string()),
            _ => None,
        };
        // A fixed dash prefix must also rule out a dashed timeline identity.
        if first
            .as_ref()
            .is_some_and(|value| !value.starts_with("--") && !"--".starts_with(value))
        {
            if let Some(value) = computed_names::affix(body, prefix, "animation", false, &keywords)?
            {
                return Ok(value);
            }
        }
        self.uncertain_identifier();
        Ok(body.into())
    }
}
#[cfg(test)]
#[path = "animation_tests.rs"]
mod tests;
