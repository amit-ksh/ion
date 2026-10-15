//! File-local CSS names for inlined SVG documents. Values are parsed by their
//! property grammar: a keyword or string is not blindly renamed across CSS.
use super::*;

#[derive(Clone, Default)]
pub(super) struct Globals {
    counters: HashSet<String>,
}
fn wide(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
    )
}
fn identifier(value: &str, prefix: &str, kind: &str) -> String {
    CssToken::Ident(format!("{prefix}{kind}_{value}").into()).to_css_string()
}
/// Separate the top-level priority from the value before interpreting names.
/// An animation/counter may itself be named `important`.
pub(super) fn priority(source: &str) -> Result<(&str, &str)> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    loop {
        let start = parser.position();
        let Ok(token) = parser.next().cloned() else {
            break;
        };
        if matches!(token, CssToken::Delim('!')) {
            if matches!(parser.next(),Ok(CssToken::Ident(name)) if name.eq_ignore_ascii_case("important"))
                && parser.expect_exhausted().is_ok()
            {
                let offset = start.byte_index();
                return Ok(source.split_at(offset));
            }
        } else if matches!(
            token,
            CssToken::Function(_)
                | CssToken::CurlyBracketBlock
                | CssToken::SquareBracketBlock
                | CssToken::ParenthesisBlock
        ) {
            nested_text(&mut parser)?;
        }
    }
    Ok((source, ""))
}
// W3C CSS Counter Styles 3 sections 6/7. These predefined names canonicalize
// ASCII case; author-defined names retain exact case.
fn predefined(value: &str) -> bool {
    const NAMES: &str = "decimal decimal-leading-zero arabic-indic armenian upper-armenian lower-armenian bengali cambodian khmer cjk-decimal devanagari georgian gujarati gurmukhi hebrew kannada lao malayalam mongolian myanmar oriya persian lower-roman upper-roman tamil telugu thai tibetan lower-alpha lower-latin upper-alpha upper-latin lower-greek hiragana hiragana-iroha katakana katakana-iroha disc circle square disclosure-open disclosure-closed cjk-earthly-branch cjk-heavenly-stem japanese-informal japanese-formal korean-hangul-formal korean-hanja-informal korean-hanja-formal simp-chinese-informal simp-chinese-formal trad-chinese-informal trad-chinese-formal cjk-ideographic ethiopic-numeric";
    NAMES.split_ascii_whitespace().any(|name| name == value)
}
fn counter_key(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if predefined(&lower) {
        lower
    } else {
        value.into()
    }
}
fn rule_name(kind: &str, source: &str) -> Option<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let name = match parser.next().ok()?.clone() {
        CssToken::Ident(value) if !wide(&value) && !value.eq_ignore_ascii_case("none") => {
            value.to_string()
        }
        CssToken::QuotedString(value) if matches!(kind, "keyframes" | "-webkit-keyframes") => {
            value.to_string()
        }
        _ => return None,
    };
    parser.expect_exhausted().ok()?;
    if kind == "counter-style"
        && matches!(
            name.to_ascii_lowercase().as_str(),
            "decimal" | "disc" | "square" | "circle" | "disclosure-open" | "disclosure-closed"
        )
    {
        return None;
    }
    Some(name)
}
pub(super) fn property(source: &str) -> Option<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let name = parser.expect_ident_cloned().ok()?.to_ascii_lowercase();
    parser.expect_exhausted().ok()?;
    Some(name)
}
impl Globals {
    pub(super) fn is_empty(&self) -> bool {
        self.counters.is_empty()
    }
    pub(super) fn collect_rule(&mut self, kind: &str, source: &str) -> Result<()> {
        if kind == "counter-style" {
            if let Some(name) = rule_name(kind, source) {
                self.counters.insert(counter_key(&name));
                ensure!(
                    self.counters.len() <= 4096,
                    "SVG counter-style definitions exceed 4096 names"
                );
            }
        }
        Ok(())
    }
    pub(super) fn prelude(&self, kind: &str, source: &str, prefix: &str) -> Result<String> {
        if prefix.is_empty() || !matches!(kind, "keyframes" | "-webkit-keyframes" | "counter-style")
        {
            return Ok(source.into());
        }
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        if !matches!(parser.next(),Ok(CssToken::AtKeyword(name)) if name.eq_ignore_ascii_case(kind))
        {
            return Ok(source.into());
        }
        let start = parser.position();
        while parser.next_including_whitespace_and_comments().is_ok() {}
        let Some(name) = rule_name(kind, parser.slice_from(start)) else {
            return Ok(source.into());
        };
        let (tag, name) = if kind == "counter-style" {
            ("counter_style", counter_key(&name))
        } else {
            ("animation", name)
        };
        Ok(format!("@{kind} {}", identifier(&name, prefix, tag)))
    }
    /// A conditional override of a predefined style must still resolve to the
    /// browser's baseline style while its defining @media/@supports is false.
    pub(super) fn baseline(&self, prefix: &str) -> Result<String> {
        if prefix.is_empty() {
            return Ok(String::new());
        }
        let mut styles = self
            .counters
            .iter()
            .filter(|name| predefined(name))
            .collect::<Vec<_>>();
        styles.sort();
        let mut out = String::new();
        for name in styles {
            push(
                &mut out,
                &format!(
                    "@counter-style {}{{system:extends {name}}}",
                    identifier(name, prefix, "counter_style")
                ),
            )?;
        }
        Ok(out)
    }
    pub(super) fn value(
        &self,
        property: &str,
        source: &str,
        prefix: &str,
        descriptor: bool,
    ) -> Result<String> {
        if prefix.is_empty() {
            return Ok(source.into());
        }
        match property {
            "animation-name" | "-webkit-animation-name" => self.animation(source, prefix, false),
            "animation" | "-webkit-animation" => self.animation(source, prefix, true),
            "list-style-type" => self.counter_values(source, prefix, CounterMode::Type, 0),
            "list-style" => self.counter_values(source, prefix, CounterMode::List, 0),
            "fallback" if descriptor => self.counter_values(source, prefix, CounterMode::Type, 0),
            "speak-as" if descriptor => self.counter_values(source, prefix, CounterMode::Speak, 0),
            "system" if descriptor => self.counter_values(source, prefix, CounterMode::System, 0),
            // counter()/counters() may occur in content, a var fallback, or a
            // custom property's token sequence. Ordinary strings stay literal.
            _ => self.counter_values(source, prefix, CounterMode::Functions, 0),
        }
    }
    fn animation(&self, source: &str, prefix: &str, shorthand: bool) -> Result<String> {
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
                        consumed.keyword(&name)
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
                    consumed.iteration = true;
                    push(&mut out, &token.to_css_string())?;
                }
                CssToken::Function(name) => {
                    if matches!(
                        name.to_ascii_lowercase().as_str(),
                        "linear" | "steps" | "cubic-bezier"
                    ) {
                        consumed.timing = true;
                    }
                    push(
                        &mut out,
                        &format!(
                            "{}({})",
                            CssToken::Ident(name).to_css_string(),
                            nested_text(&mut parser)?
                        ),
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
    fn counter_values(
        &self,
        source: &str,
        prefix: &str,
        mode: CounterMode,
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
                    let keyword = name.to_ascii_lowercase();
                    let replace = match mode {
                        CounterMode::Functions => false,
                        CounterMode::Type => !wide(&name) && keyword != "none",
                        CounterMode::List => {
                            !wide(&name)
                                && !matches!(keyword.as_str(), "none" | "inside" | "outside")
                        }
                        CounterMode::Speak => !matches!(
                            keyword.as_str(),
                            "auto" | "bullets" | "numbers" | "words" | "spell-out"
                        ),
                        CounterMode::System => {
                            if extends {
                                extends = false;
                                true
                            } else {
                                extends = keyword == "extends";
                                false
                            }
                        }
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
                        "counter" => self.counter_function(&body, prefix, 1, depth + 1)?,
                        "counters" => self.counter_function(&body, prefix, 2, depth + 1)?,
                        _ => {
                            self.counter_values(&body, prefix, CounterMode::Functions, depth + 1)?
                        }
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
                depth,
            )?,
        )?;
        Ok(out)
    }
}
#[derive(Clone, Copy)]
enum CounterMode {
    Functions,
    Type,
    List,
    Speak,
    System,
}
#[derive(Default)]
struct Animation {
    timing: bool,
    iteration: bool,
    direction: bool,
    fill: bool,
    play: bool,
}
impl Animation {
    fn keyword(&mut self, word: &str) -> bool {
        if wide(word) || word.eq_ignore_ascii_case("none") {
            return true;
        }
        let slot = match word.to_ascii_lowercase().as_str() {
            "ease" | "linear" | "ease-in" | "ease-out" | "ease-in-out" | "step-start"
            | "step-end" => Some(&mut self.timing),
            "infinite" => Some(&mut self.iteration),
            "normal" | "reverse" | "alternate" | "alternate-reverse" => Some(&mut self.direction),
            "forwards" | "backwards" | "both" => Some(&mut self.fill),
            "running" | "paused" => Some(&mut self.play),
            _ => None,
        };
        match slot {
            Some(slot) if !*slot => {
                *slot = true;
                true
            }
            _ => false,
        }
    }
}
pub(super) fn keyframes(source: &str, names: &Names, depth: usize) -> Result<String> {
    ensure!(depth <= 32, "CSS keyframe nesting exceeds its bound");
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        if matches!(token, CssToken::CurlyBracketBlock) {
            push(
                &mut out,
                &format!("{{{}}}", declarations(&nested_text(&mut parser)?, names)?),
            )?;
        } else {
            push(&mut out, &token.to_css_string())?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names() -> Names {
        let mut names = Names::native(HashSet::new(), "owner".into(), true);
        names.prefix = "private_".into();
        for key in ["marks", "fallback-style", "inside", "words"] {
            names.globals.collect_rule("counter-style", key).unwrap();
        }
        names
    }
    #[test]
    fn named_css_animation_keywords_quoted_names_and_lists_keep_their_roles() {
        let n = names();
        assert_eq!(declarations("animation:2s linear pulse,linear 1s linear,2s both both;animation-name:Pulse,pulse,PULSE,none,\"none\",\"quoted name\";color:pulse;content:'pulse'",&n).unwrap(),"animation:2s linear private_animation_pulse,linear 1s private_animation_linear,2s both private_animation_both;animation-name:private_animation_Pulse,private_animation_pulse,private_animation_PULSE,none,private_animation_none,private_animation_quoted\\ name;color:pulse;content:\"pulse\"");
        let style=namespace_styles("@keyframes pulse{from{opacity:0}50%,to{opacity:1}}@-webkit-keyframes \"quoted name\"{0%{opacity:0}100%{opacity:1}}.shape{animation:1s steps(2,end) pulse}",&n,0).unwrap();
        assert!(
            style.contains("@keyframes private_animation_pulse{from{opacity:0}50%,to{opacity:1}}")
        );
        assert!(style.contains(
            "@-webkit-keyframes private_animation_quoted\\ name{0%{opacity:0}100%{opacity:1}}"
        ));
        assert!(!style.contains("from:where"));
        assert!(!style.contains("50%:where"));
        assert!(style.contains("1s steps(2,end) private_animation_pulse"));
        assert_eq!(
            declarations(
                "animation-name:pulse ! IMPORTANT;animation:linear 1s linear!important",
                &n
            )
            .unwrap(),
            "animation-name:private_animation_pulse ! IMPORTANT;animation:linear 1s private_animation_linear!important"
        );

        assert_eq!(
            declarations(r"animat\69on-name:p\75lse!important", &n).unwrap(),
            "animation-name:private_animation_pulse!important"
        );
    }
    #[test]
    fn named_css_invalid_rules_and_reserved_counters_keep_native_meaning() {
        let mut n = names();
        for rule in [
            "decimal",
            "DISC",
            "circle",
            "square",
            "disclosure-open",
            "disclosure-closed",
            "inherit",
            "marks extra",
            "'marks'",
        ] {
            n.globals.collect_rule("counter-style", rule).unwrap();
            let source = format!("@counter-style {rule}");
            assert_eq!(
                n.globals
                    .prelude("counter-style", &source, &n.prefix)
                    .unwrap(),
                source
            );
        }
        n.globals
            .collect_rule("counter-style", "LOWER-ROMAN")
            .unwrap();
        assert_eq!(
            n.globals
                .prelude("counter-style", "@counter-style LOWER-ROMAN", &n.prefix)
                .unwrap(),
            "@counter-style private_counter_style_lower-roman"
        );
        assert_eq!(
            n.globals.baseline(&n.prefix).unwrap(),
            "@counter-style private_counter_style_lower-roman{system:extends lower-roman}"
        );
        assert_eq!(declarations("list-style-type:Lower-Roman;list-style-type:decimal;list-style-type:DISC;list-style-type:inside",&n).unwrap(),"list-style-type:private_counter_style_lower-roman;list-style-type:decimal;list-style-type:DISC;list-style-type:private_counter_style_inside");
        assert_eq!(
            n.globals
                .prelude("keyframes", "@keyframes inherit", &n.prefix)
                .unwrap(),
            "@keyframes inherit"
        );
        assert_eq!(
            n.globals
                .prelude("keyframes", "@keyframes one two", &n.prefix)
                .unwrap(),
            "@keyframes one two"
        );
    }
    #[test]
    fn named_css_counter_descriptors_and_function_arguments_keep_literal_strings() {
        let n = names();
        let css=namespace_styles("@counter-style marks{system:extends fallback-style;fallback:fallback-style;speak-as:words;symbols:'marks' 'words'}@counter-style fallback-style{system:cyclic;symbols:'x';speak-as:marks}.shape{list-style:inside marks;list-style-type:marks;content:counter(step,marks) counters(section,'.',fallback-style) 'marks';--caption:counter(step,marks)}",&n,0).unwrap();
        assert!(css.contains("@counter-style private_counter_style_marks{system:extends private_counter_style_fallback-style;fallback:private_counter_style_fallback-style;speak-as:words;symbols:\"marks\" \"words\"}"));
        assert!(css.contains("speak-as:private_counter_style_marks"));
        assert!(css.contains("list-style:inside private_counter_style_marks"));
        assert!(css.contains("counter(step,private_counter_style_marks) counters(section,\".\",private_counter_style_fallback-style) \"marks\""));
        assert!(css.contains("--caption:counter(step,private_counter_style_marks)"));
        assert_eq!(declarations("list-style:inside inside;list-style-type:'marks';list-style-type:decimal;list-style-type:none;content:counters(chapter,'marks')",&n).unwrap(),"list-style:inside inside;list-style-type:\"marks\";list-style-type:decimal;list-style-type:none;content:counters(chapter,\"marks\")");
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    fn bundle() -> Bundle {
        Bundle {resources:[
            ("a.svg", "image/svg+xml", r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import 'shared.css';.shape{animation:1s linear pulse;list-style-type:marks;content:counter(step,marks)}</style><symbol id="badge"><path class="shape" style="animation-name:pulse;list-style:inside marks"/></symbol></svg>"##),
            ("b.svg", "image/svg+xml", r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import url('shared.css');@keyframes pulse{0%{opacity:.4}to{opacity:.8}}.shape{animation:pulse 2s ease-in-out;content:counters(section,'.',marks)}</style><symbol id="badge"><path class="shape"/></symbol></svg>"##),
            ("shared.css", "text/css", r##"@counter-style marks{system:cyclic;symbols:'A' 'B';fallback:decimal}@media(min-width:1px){@keyframes pulse{from{opacity:0}50%,to{opacity:1}}.shape{animation-name:pulse}}"##),
        ].into_iter().map(|(path,mime,text)|(path.into(),(mime,text.as_bytes().to_vec()))).collect(),encoded:HashMap::new(),visiting:HashSet::new(),used:HashSet::new()}
    }
    fn fixture() -> String {
        let mut b = bundle();
        let html=b.html(r##"<html><head><style>@keyframes pulse{from{opacity:1}to{opacity:.2}}@counter-style marks{system:cyclic;symbols:'X'}.owner{animation:3s pulse;list-style-type:marks}</style></head><body><div class="owner">Owner</div><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
        assert_eq!(b.used.len(), 3);
        html
    }
    #[test]
    fn named_css_publication_keeps_same_named_sprite_animations_and_counter_styles_independent() {
        let html = fixture();
        let parsed = roxmltree::Document::parse(&html).unwrap();
        let a = format!("ion_svg_{}_", hash(b"a.svg"));
        let b = format!("ion_svg_{}_", hash(b"b.svg"));
        let styles = parsed
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .filter_map(|n| n.text())
            .collect::<Vec<_>>();
        assert!(
            styles
                .iter()
                .any(|css| css.contains("@keyframes pulse{from{opacity:1}to{opacity:0.2}}"))
        );
        assert!(
            styles
                .iter()
                .any(|css| css.contains(&format!("animation:1s linear {a}animation_pulse")))
        );
        assert!(
            styles
                .iter()
                .any(|css| css.contains(&format!("animation:{b}animation_pulse 2s ease-in-out")))
        );
        assert!(styles.iter().any(|css| css.contains(&format!(
            "@keyframes {b}animation_pulse{{0%{{opacity:0.4}}to{{opacity:0.8}}}}"
        ))));
        let inline = parsed
            .descendants()
            .find_map(|n| n.attribute("style"))
            .unwrap();
        assert!(inline.contains(&format!("animation-name:{a}animation_pulse")));
        assert!(inline.contains(&format!("list-style:inside {a}counter_style_marks")));
        let mut imports = Vec::new();
        for style in styles {
            if let Some(data) = style.split("data:text/css;base64,").nth(1) {
                let data = data.split('"').next().unwrap();
                imports.push(String::from_utf8(STANDARD.decode(data).unwrap()).unwrap());
            }
        }
        assert_eq!(imports.len(), 2);
        for prefix in [&a, &b] {
            let css = imports
                .iter()
                .find(|css| css.contains(&format!("@keyframes {prefix}animation_pulse")))
                .unwrap();
            assert!(css.contains(&format!("@counter-style {prefix}counter_style_marks")));
            assert!(css.contains("fallback:decimal"));
            assert!(css.contains(&format!("animation-name:{prefix}animation_pulse")));
            assert!(!css.contains("from:where"));
        }
    }
    #[test]
    fn named_css_root_styles_remain_unrenamed_without_imports_and_unknown_animation_does_not_bind_owner()
     {
        let native = Names::native(HashSet::new(), "owner".into(), true);
        assert_eq!(
            namespace_styles(
                "@keyframes pulse{from{opacity:0}to{opacity:1}}.owner{animation:pulse 1s}",
                &native,
                0
            )
            .unwrap()
            .matches("animation:pulse 1s")
            .count(),
            1
        );
        let mut imported = Names::native(HashSet::new(), "owner".into(), true);
        imported.prefix = "file_".into();
        assert_eq!(
            declarations("animation-name:missing;content:'missing'", &imported).unwrap(),
            "animation-name:file_animation_missing;content:\"missing\""
        );
        assert!(namespace_styles(&"@media(x){".repeat(34), &imported, 0).is_err());
    }
    #[test]
    #[ignore = "Exports the actual native named-CSS document to ION_NAMED_CSS_FIXTURE"]
    fn native_named_css_phone_fixture() {
        std::fs::write(
            std::env::var_os("ION_NAMED_CSS_FIXTURE").expect("Set ION_NAMED_CSS_FIXTURE"),
            fixture(),
        )
        .unwrap();
    }
}
