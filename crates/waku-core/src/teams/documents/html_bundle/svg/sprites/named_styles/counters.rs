//! Counter-style grammar, preserving native substitution and descriptor boundaries.
use super::*;
impl Globals {
    fn counter_keyword(mode: CounterMode, name: &str) -> bool {
        wide(name)
            || match mode {
                CounterMode::Type => name.eq_ignore_ascii_case("none"),
                CounterMode::List => ["none", "inside", "outside"]
                    .iter()
                    .any(|word| name.eq_ignore_ascii_case(word)),
                CounterMode::Speak => ["auto", "bullets", "numbers", "words", "spell-out"]
                    .iter()
                    .any(|word| name.eq_ignore_ascii_case(word)),
                CounterMode::Functions | CounterMode::System => true,
            }
    }
    fn computed_counter(&self, body: &str, prefix: &str, mode: CounterMode) -> Result<String> {
        if let Some(name) = computed_names::literal(body)? {
            let key = counter_key(&name);
            return Ok(
                if !Self::counter_keyword(mode, &name) && self.counters.contains(&key) {
                    computed_names::quoted(&format!("{prefix}counter_style_{key}"))
                } else {
                    body.into()
                },
            );
        }
        let mut input = ParserInput::new(body);
        let mut parser = Parser::new(&mut input);
        let first = match parser.next().cloned() {
            Ok(CssToken::Ident(name) | CssToken::QuotedString(name)) if !name.is_empty() => {
                name.to_string()
            }
            _ => return Ok(body.into()),
        };
        // Only author-name prefixes that can reach a collected definition are
        // transformed. Ordered lookup avoids a scan of up to 4096 definitions
        // for every computed value in a large imported document.
        if !self
            .counters
            .range(first.clone()..)
            .next()
            .is_some_and(|name| name.starts_with(&first))
        {
            return Ok(body.into());
        }
        // An arbitrary suffix must never turn a UA predefined style/keyword
        // into an unknown private name. Built-in canonicalization is only
        // applied when the complete literal result is actually known.
        let mut reserved = PREDEFINED.to_vec();
        reserved.push("none");
        if matches!(mode, CounterMode::List) {
            reserved.extend(["inside", "outside"]);
        }
        Ok(
            computed_names::affix(body, prefix, "counter_style", false, &reserved)?
                .unwrap_or_else(|| body.into()),
        )
    }
    pub(super) fn counter_values(
        &self,
        source: &str,
        prefix: &str,
        mode: CounterMode,
        computed: bool,
        depth: usize,
    ) -> Result<String> {
        ensure!(depth <= 32, "CSS named-value nesting exceeds its bound");
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        let mut out = String::new();
        let mut extends = false;
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            match token {
                CssToken::Ident(name) => {
                    let replace = match mode {
                        CounterMode::System => {
                            if extends {
                                extends = false;
                                true
                            } else {
                                extends = name.eq_ignore_ascii_case("extends");
                                false
                            }
                        }
                        _ => !Self::counter_keyword(mode, &name),
                    };
                    let key = counter_key(&name);
                    if replace && self.counters.contains(&key) {
                        push(&mut out, &identifier(&key, prefix, "counter_style"))?;
                    } else {
                        push(&mut out, &CssToken::Ident(name).to_css_string())?;
                    }
                }
                CssToken::Function(name) => {
                    let body = nested_text(&mut parser)?;
                    let rewritten = match name.to_ascii_lowercase().as_str() {
                        "counter" => {
                            self.counter_function(&body, prefix, 1, computed, depth + 1)?
                        }
                        "counters" => {
                            self.counter_function(&body, prefix, 2, computed, depth + 1)?
                        }
                        "ident"
                            if computed
                                && matches!(mode, CounterMode::Type | CounterMode::List) =>
                        {
                            self.computed_counter(&body, prefix, mode)?
                        }
                        "var"
                            if computed
                                && matches!(mode, CounterMode::Type | CounterMode::List) =>
                        {
                            if let Some(offset) = computed_names::fallback_offset(&body) {
                                format!(
                                    "{}{}",
                                    &body[..offset],
                                    self.counter_values(
                                        &body[offset..],
                                        prefix,
                                        mode,
                                        true,
                                        depth + 1
                                    )?
                                )
                            } else {
                                body
                            }
                        }
                        _ => self.counter_values(
                            &body,
                            prefix,
                            CounterMode::Functions,
                            computed,
                            depth + 1,
                        )?,
                    };
                    push(
                        &mut out,
                        &format!("{}({rewritten})", CssToken::Ident(name).to_css_string()),
                    )?;
                }
                CssToken::ParenthesisBlock => push(
                    &mut out,
                    &format!(
                        "({})",
                        self.counter_values(
                            &nested_text(&mut parser)?,
                            prefix,
                            CounterMode::Functions,
                            computed,
                            depth + 1
                        )?
                    ),
                )?,
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
    fn counter_function(
        &self,
        source: &str,
        prefix: &str,
        style_index: usize,
        computed: bool,
        depth: usize,
    ) -> Result<String> {
        ensure!(
            depth <= 32,
            "CSS counter function nesting exceeds its bound"
        );
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        let mut out = String::new();
        let mut part = String::new();
        let mut index = 0;
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            if matches!(token, CssToken::Comma) {
                push(
                    &mut out,
                    &self.counter_values(
                        &part,
                        prefix,
                        if index == style_index {
                            CounterMode::Type
                        } else {
                            CounterMode::Functions
                        },
                        computed,
                        depth,
                    )?,
                )?;
                push(&mut out, ",")?;
                part.clear();
                index += 1;
                continue;
            }
            match token {
                CssToken::Function(name) => push(
                    &mut part,
                    &format!(
                        "{}({})",
                        CssToken::Ident(name).to_css_string(),
                        nested_text(&mut parser)?
                    ),
                )?,
                CssToken::ParenthesisBlock => {
                    push(&mut part, &format!("({})", nested_text(&mut parser)?))?
                }
                CssToken::SquareBracketBlock => {
                    push(&mut part, &format!("[{}]", nested_text(&mut parser)?))?
                }
                CssToken::CurlyBracketBlock => {
                    push(&mut part, &format!("{{{}}}", nested_text(&mut parser)?))?
                }
                _ => push(&mut part, &token.to_css_string())?,
            }
        }
        push(
            &mut out,
            &self.counter_values(
                &part,
                prefix,
                if index == style_index {
                    CounterMode::Type
                } else {
                    CounterMode::Functions
                },
                computed,
                depth,
            )?,
        )?;
        Ok(out)
    }
}
#[cfg(test)]
#[path = "computed_tests.rs"]
mod tests;
