//! Preserve CSS layer hierarchy and named-container identity when a reference
//! document becomes part of another document. Query conditions stay native CSS.
use super::*;

fn css_wide(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "initial" | "inherit" | "unset" | "revert" | "revert-layer"
    )
}
fn container_reserved(name: &str) -> bool {
    css_wide(name)
        || matches!(
            name.to_ascii_lowercase().as_str(),
            "none" | "and" | "or" | "not" | "default"
        )
}
fn ident(name: &str, prefix: &str, kind: &str) -> String {
    CssToken::Ident(format!("{prefix}{kind}_{name}").into()).to_css_string()
}
/// Validate the entire name/list before rewriting. In particular, whitespace
/// around a dot and CSS-wide identifiers must not become valid layer names.
fn layers(source: &str, prefix: &str, list: bool, anonymous: bool) -> Result<Option<String>> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let (mut expect_name, mut saw_name, mut whitespace, mut after_dot) =
        (true, false, false, false);
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::WhiteSpace(_) => {
                if after_dot {
                    return Ok(None);
                }
                whitespace = true;
                push(&mut out, &token.to_css_string())?;
            }
            CssToken::Comment(_) => push(&mut out, &token.to_css_string())?,
            CssToken::Ident(name) if expect_name && !css_wide(&name) => {
                push(&mut out, &ident(&name, prefix, "layer"))?;
                expect_name = false;
                saw_name = true;
                whitespace = false;
                after_dot = false;
            }
            CssToken::Delim('.') if !expect_name && !whitespace => {
                push(&mut out, ".")?;
                expect_name = true;
                after_dot = true;
            }
            CssToken::Comma if list && !expect_name => {
                push(&mut out, ",")?;
                expect_name = true;
                whitespace = false;
            }
            _ => return Ok(None),
        }
    }
    Ok(((!expect_name && saw_name) || (!saw_name && anonymous)).then_some(out))
}

pub(super) fn prelude(kind: &str, source: &str, prefix: &str, statement: bool) -> Result<String> {
    if prefix.is_empty() {
        return Ok(source.into());
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    if !matches!(parser.next(), Ok(CssToken::AtKeyword(name)) if name.eq_ignore_ascii_case(kind)) {
        return Ok(source.into());
    }
    let offset = parser.position().byte_index();
    if kind == "layer" {
        return Ok(
            match layers(&source[offset..], prefix, statement, !statement)? {
                Some(value) => format!("{}{value}", &source[..offset]),
                None => source.into(),
            },
        );
    }
    if kind == "container" && !statement {
        let mut out = source[..offset].to_owned();
        let mut first = true;
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            match token {
                CssToken::WhiteSpace(_) | CssToken::Comment(_) => {
                    push(&mut out, &token.to_css_string())?
                }
                CssToken::Comma => {
                    first = true;
                    push(&mut out, ",")?;
                }
                CssToken::Ident(name) if first && !container_reserved(&name) => {
                    push(&mut out, &ident(&name, prefix, "container"))?;
                    first = false;
                }
                CssToken::Function(name) => {
                    let body = nested_text(&mut parser)?;
                    let body = if name.eq_ignore_ascii_case("style") {
                        query(&body, prefix, true, 0)?
                    } else {
                        body
                    };
                    push(
                        &mut out,
                        &format!("{}({body})", CssToken::Ident(name).to_css_string()),
                    )?;
                    first = false;
                }
                CssToken::ParenthesisBlock => {
                    push(
                        &mut out,
                        &format!("({})", query(&nested_text(&mut parser)?, prefix, false, 0)?),
                    )?;
                    first = false;
                }
                CssToken::SquareBracketBlock => {
                    push(&mut out, &format!("[{}]", nested_text(&mut parser)?))?;
                    first = false;
                }
                _ => {
                    push(&mut out, &token.to_css_string())?;
                    first = false;
                }
            }
        }
        return Ok(out);
    }
    if kind == "import" && statement {
        match parser.next().cloned() {
            Ok(CssToken::QuotedString(_) | CssToken::UnquotedUrl(_)) => {}
            Ok(CssToken::Function(name)) if name.eq_ignore_ascii_case("url") => {
                nested_text(&mut parser)?;
            }
            _ => return Ok(source.into()),
        }
        // Only the import's first modifier can be layer(...). Do not interpret
        // functions in supports/media conditions or strings as layer names.
        if !matches!(parser.next(), Ok(CssToken::Function(name)) if name.eq_ignore_ascii_case("layer"))
        {
            return Ok(source.into());
        }
        let opening = parser.position().byte_index();
        let body = nested_text(&mut parser)?;
        let end = parser.position().byte_index();
        if let Some(value) = layers(&body, prefix, false, true)? {
            // Parser positions retain leading comments, function spelling and
            // escaped names without searching for punctuation inside comments.
            return Ok(format!("{}{value}){}", &source[..opening], &source[end..]));
        }
    }
    Ok(source.into())
}

/// Rewrite only style features whose property consumes a container name.
/// Custom-property values are opaque: a token spelled `card` there is data,
/// rather than a container reference. Nested boolean conditions retain syntax.
fn query(source: &str, prefix: &str, style: bool, depth: usize) -> Result<String> {
    ensure!(depth <= 32, "CSS container query nesting exceeds its bound");
    if style {
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        if let Ok(CssToken::Ident(property)) = parser.next().cloned() {
            if matches!(parser.next(), Ok(CssToken::Colon)) {
                if property.eq_ignore_ascii_case("container")
                    || property.eq_ignore_ascii_case("container-name")
                {
                    let offset = parser.position().byte_index();
                    let (body, priority) = named_styles::priority(&source[offset..])?;
                    return Ok(format!(
                        "{}{}{}",
                        &source[..offset],
                        value(body, prefix, property.eq_ignore_ascii_case("container"))?,
                        priority
                    ));
                }
                return Ok(source.into());
            }
        }
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::ParenthesisBlock => push(
                &mut out,
                &format!(
                    "({})",
                    query(&nested_text(&mut parser)?, prefix, style, depth + 1)?
                ),
            )?,
            CssToken::Function(name) => {
                let body = nested_text(&mut parser)?;
                let body = if !style && name.eq_ignore_ascii_case("style") {
                    query(&body, prefix, true, depth + 1)?
                } else {
                    body
                };
                push(
                    &mut out,
                    &format!("{}({body})", CssToken::Ident(name).to_css_string()),
                )?;
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

pub(super) fn value(source: &str, prefix: &str, shorthand: bool) -> Result<String> {
    if prefix.is_empty() {
        return Ok(source.into());
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut saw_name = false;
    let mut split = source.len();
    loop {
        let start = parser.position().byte_index();
        match parser.next().cloned() {
            Ok(CssToken::Ident(name)) if !container_reserved(&name) => saw_name = true,
            Ok(CssToken::Ident(_)) => return Ok(source.into()),
            Ok(CssToken::Delim('/')) if shorthand => {
                split = start;
                break;
            }
            Err(_) => break,
            _ => return Ok(source.into()), // deferred var()/ident() values stay native
        }
    }
    if !saw_name {
        return Ok(source.into());
    }
    let mut input = ParserInput::new(&source[..split]);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        if let CssToken::Ident(name) = token {
            push(&mut out, &ident(&name, prefix, "container"))?;
        } else {
            push(&mut out, &token.to_css_string())?;
        }
    }
    push(&mut out, &source[split..])?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names() -> Names {
        let mut names = Names::native(HashSet::new(), "owner".into(), true);
        names.prefix = "private_".into();
        names
    }
    #[test]
    fn context_css_layer_paths_order_imports_and_anonymous_layers_keep_hierarchy() {
        let source = r#"@layer reset,framework.theme,default;@layer framework{@layer theme{path{fill:green}}}@layer {path{opacity:.5}}@import "data:text/css;base64,eA==" layer(framework.theme) supports(display:grid) screen;@import "data:text/css;base64,eA==" layer screen;"#;
        let css = namespace_styles(source, &names(), 0).unwrap();
        assert!(css.contains("@layer private_layer_reset,private_layer_framework.private_layer_theme,private_layer_default;"));
        assert!(css.contains("@layer private_layer_framework{@layer private_layer_theme{"));
        assert!(css.contains("@layer {path:where("));
        assert!(css.contains(
            "layer(private_layer_framework.private_layer_theme) supports(display:grid) screen"
        ));
        assert!(css.contains("layer screen;"));
        assert_eq!(
            prelude("layer", r"@layer f\6fo.bar", "p_", false).unwrap(),
            "@layer p_layer_foo.p_layer_bar"
        );
    }
    #[test]
    fn context_css_comment_punctuation_and_layer_keyword_roles_are_retained() {
        assert_eq!(
            prelude(
                "import",
                r#"@import "x" /*(*/lAyEr(foo/**/.bar) print"#,
                "p_",
                true
            )
            .unwrap(),
            r#"@import "x" /*(*/lAyEr(p_layer_foo/**/.p_layer_bar) print"#
        );
        assert_eq!(
            prelude("layer", "@layer none.Default", "p_", false).unwrap(),
            "@layer p_layer_none.p_layer_Default"
        );
    }
    #[test]
    fn context_css_invalid_layer_rules_keep_native_invalidity() {
        for source in [
            "@layer inherit",
            "@layer a .b",
            "@layer a. b",
            "@layer a..b",
            "@layer a b",
            "@layer a,",
            "@layer a.revert-layer",
            "@layer 'quoted'",
        ] {
            assert_eq!(prelude("layer", source, "p_", true).unwrap(), source);
        }
        assert_eq!(
            prelude("layer", "@layer a,b", "p_", false).unwrap(),
            "@layer a,b"
        );
        assert_eq!(
            prelude("import", r#"@import "x" supports(layer(foo))"#, "p_", true).unwrap(),
            r#"@import "x" supports(layer(foo))"#
        );
        assert_eq!(
            prelude("import", r#"@import "x" layer(a b)"#, "p_", true).unwrap(),
            r#"@import "x" layer(a b)"#
        );
    }
    #[test]
    fn context_css_named_container_queries_only_change_name_positions() {
        let source = "@container card (width > 20px), other style(--tone:card), not (width < 1px), style(--tone:card){path{container:card small / inline-size;container-name:card;content:'card'}}";
        let css = namespace_styles(source, &names(), 0).unwrap();
        assert!(css.contains("@container private_container_card (width > 20px), private_container_other style(--private_property_tone:card), not (width < 1px), style(--private_property_tone:card)"));
        assert!(
            css.contains("container:private_container_card private_container_small / inline-size")
        );
        assert!(css.contains("container-name:private_container_card"));
        assert!(css.contains("content:\"card\""));
        assert_eq!(
            value(r"c\61rd Card", "p_", false).unwrap(),
            "p_container_card p_container_Card"
        );
        for source in [
            "none",
            "inherit",
            "none other",
            "and",
            "not",
            "var(--container)",
            "ident('--card')",
            "'card'",
        ] {
            assert_eq!(value(source, "p_", false).unwrap(), source);
        }
        assert_eq!(
            prelude("container", "@container not (width < 1px)", "p_", false).unwrap(),
            "@container not (width < 1px)"
        );
    }
    #[test]
    fn context_css_style_queries_follow_container_names_without_changing_custom_values() {
        let source = "@container card ((style(container-name:card)) and (width>2px)), style((container:card/size) and (--tone:card)), style(--tokens:style(container-name:card)){path{fill:red}}";
        let css = namespace_styles(source, &names(), 0).unwrap();
        assert!(css.contains("@container private_container_card ((style(container-name:private_container_card)) and (width>2px)), style((container:private_container_card/size) and (--private_property_tone:card)), style(--private_property_tokens:style(container-name:card))"));
        assert!(
            prelude(
                "container",
                &format!(
                    "@container {}style(container-name:card){}",
                    "(".repeat(34),
                    ")".repeat(34)
                ),
                "p_",
                false
            )
            .is_err()
        );
    }
    #[test]
    fn context_css_source_html_names_remain_native_and_depth_bound_holds() {
        let native = Names::native(HashSet::new(), "owner".into(), true);
        let source =
            "@layer base,top;@layer top{@container card (width>1px){.owner{container:card/size}}}";
        let css = namespace_styles(source, &native, 0).unwrap();
        assert!(css.contains("@layer base,top;@layer top{@container card (width>1px)"));
        assert!(css.contains("container:card/size"));
        assert!(!css.contains("_layer_"));
        assert!(namespace_styles(&"@layer a{".repeat(34), &names(), 0).is_err());
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    fn fixture() -> String {
        let mut bundle=Bundle{resources:[
            ("a.svg","image/svg+xml",r##"<svg xmlns="http://www.w3.org/2000/svg" style="container:card/inline-size"><style>@import 'shared.css' layer(framework.theme) supports(display:grid);@layer reset,framework.theme;@layer framework{@layer theme{.shape{container-name:card;fill:red}}}@container card (width > 1px){.shape{stroke:green}}@container card style(container-name:card){.shape{opacity:.8}}</style><symbol id="badge"><path class="shape"/></symbol></svg>"##),
            ("b.svg","image/svg+xml",r##"<svg xmlns="http://www.w3.org/2000/svg" style="container-name:card"><style>@import url('shared.css') layer(framework.theme);@layer framework.theme{.shape{fill:blue}}@container card (width > 2px){.shape{stroke:purple}}</style><symbol id="badge"><path class="shape" style="container:card small / inline-size"/></symbol></svg>"##),
            ("shared.css","text/css",r##"@layer framework.theme{.shape{opacity:.7;container:card/inline-size}}@container card style(--tone:green){.shape{fill:green}}@container card style(container-name:card){.shape{stroke:green}}"##),
        ].into_iter().map(|(path,mime,css)|(path.into(),(mime,css.as_bytes().to_vec()))).collect(),encoded:HashMap::new(),visiting:HashSet::new(),used:HashSet::new()};
        let html=bundle.html(r##"<html><head><style>@layer reset,framework.theme;@layer framework.theme{.owner{container:card/inline-size}}@container card (width > 1px){.owner{color:green}}</style></head><body><div class="owner">Owner</div><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
        assert_eq!(bundle.used.len(), 3);
        html
    }
    #[test]
    fn context_css_publication_keeps_two_document_layers_and_containers_independent() {
        let html = fixture();
        let document = roxmltree::Document::parse(&html).unwrap();
        let styles = document
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .filter_map(|n| n.text())
            .collect::<Vec<_>>();
        assert!(
            styles
                .iter()
                .any(|css| css.starts_with("@layer reset,framework.theme;"))
        );
        for file in ["a.svg", "b.svg"] {
            let prefix = format!("ion_svg_{}_", hash(file.as_bytes()));
            let css = styles
                .iter()
                .find(|css| {
                    css.contains(&format!(
                        "layer({prefix}layer_framework.{prefix}layer_theme)"
                    ))
                })
                .unwrap();
            assert!(css.contains(&format!("@container {prefix}container_card (width >")));
            let import = css
                .split("data:text/css;base64,")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let import = String::from_utf8(STANDARD.decode(import).unwrap()).unwrap();
            assert!(import.contains(&format!(
                "@layer {prefix}layer_framework.{prefix}layer_theme"
            )));
            assert!(import.contains(&format!("container:{prefix}container_card/inline-size")));
            assert!(import.contains(&format!(
                "@container {prefix}container_card style(--{prefix}property_tone:green)"
            )));
            assert!(import.contains(&format!(
                "@container {prefix}container_card style(container-name:{prefix}container_card)"
            )));
            assert!(
                document
                    .descendants()
                    .filter_map(|n| n.attribute("style"))
                    .any(
                        |style| style.contains(&format!("container:{prefix}container_card"))
                            || style.contains(&format!("container-name:{prefix}container_card"))
                    )
            );
        }
    }
    #[test]
    #[ignore = "Exports the actual production context CSS document to ION_CONTEXT_CSS_FIXTURE"]
    fn native_context_css_phone_fixture() {
        std::fs::write(
            std::env::var_os("ION_CONTEXT_CSS_FIXTURE").expect("Set ION_CONTEXT_CSS_FIXTURE"),
            fixture(),
        )
        .unwrap();
    }
}
