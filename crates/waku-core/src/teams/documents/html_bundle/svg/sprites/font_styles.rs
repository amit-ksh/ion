//! Private palette and feature-value identities for imported SVG styles.
//! Font matching, glyph selection and palette interpolation stay browser-owned.
use super::*;

fn dashed(value: &str) -> bool {
    value.starts_with("--") && value.len() > 2
}
fn key(value: &str, prefix: &str, palette: bool) -> String {
    let name = if palette {
        format!("--{prefix}font_palette_{}", &value[2..])
    } else {
        format!("{prefix}font_feature_{value}")
    };
    CssToken::Ident(name.into()).to_css_string()
}
fn single(source: &str, prefix: &str, palette: bool) -> Option<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut found = None;
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::WhiteSpace(_) | CssToken::Comment(_) => {}
            CssToken::Ident(name) if found.is_none() && (!palette || dashed(&name)) => {
                found = Some((parser.position().byte_index(), name.to_string()));
            }
            _ => return None,
        }
    }
    let (end, name) = found?;
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    loop {
        let start = parser.position().byte_index();
        match parser.next_including_whitespace_and_comments().ok()? {
            CssToken::Ident(_) => {
                return Some(format!(
                    "{}{}{}",
                    &source[..start],
                    key(&name, prefix, palette),
                    &source[end..]
                ));
            }
            _ => {}
        }
    }
}
fn feature(kind: &str) -> bool {
    matches!(
        kind,
        "stylistic"
            | "historical-forms"
            | "styleset"
            | "character-variant"
            | "swash"
            | "ornaments"
            | "annotation"
    )
}
pub(super) fn prelude(kind: &str, source: &str, names: &Names) -> Result<String> {
    if names.prefix.is_empty() || !matches!(kind, "font-palette-values" | "font-feature-values") {
        return Ok(source.into());
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    if !matches!(parser.next(),Ok(CssToken::AtKeyword(name)) if name.eq_ignore_ascii_case(kind)) {
        return Ok(source.into());
    }
    let end = parser.position().byte_index();
    let tail = if kind == "font-palette-values" {
        single(&source[end..], &names.prefix, true).unwrap_or_else(|| source[end..].into())
    } else {
        names.family(&source[end..])?
    };
    Ok(format!("{}{tail}", &source[..end]))
}
#[derive(Clone, Copy)]
enum Mode {
    Palette,
    Alternates,
    FeatureArgs,
    Opaque,
}
fn values(source: &str, prefix: &str, mode: Mode, depth: usize) -> Result<String> {
    ensure!(depth <= 32, "Font CSS value nesting exceeds its bound");
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            break;
        };
        match token {
            CssToken::Ident(name)
                if matches!(mode, Mode::FeatureArgs)
                    || (matches!(mode, Mode::Palette) && dashed(&name)) =>
            {
                push(&mut out, &key(&name, prefix, matches!(mode, Mode::Palette)))?
            }
            CssToken::Function(name) => {
                let opening = parser.position().byte_index();
                let body = nested_text(&mut parser)?;
                let rendered = if name.eq_ignore_ascii_case("var") {
                    let mut input = ParserInput::new(&body);
                    let mut inner = Parser::new(&mut input);
                    if matches!(inner.next(),Ok(CssToken::Ident(name)) if dashed(name))
                        && matches!(inner.next(), Ok(CssToken::Comma))
                    {
                        let offset = inner.position().byte_index();
                        format!(
                            "{}{}",
                            &body[..offset],
                            values(&body[offset..], prefix, mode, depth + 1)?
                        )
                    } else {
                        body.clone()
                    }
                } else if matches!(mode, Mode::Palette) && name.eq_ignore_ascii_case("palette-mix")
                {
                    let mut input = ParserInput::new(&body);
                    let mut inner = Parser::new(&mut input);
                    if matches!(inner.next(),Ok(CssToken::Ident(name)) if name.eq_ignore_ascii_case("in"))
                    {
                        while inner
                            .next()
                            .is_ok_and(|token| !matches!(token, CssToken::Comma))
                        {
                        }
                        let offset = inner.position().byte_index();
                        format!(
                            "{}{}",
                            &body[..offset],
                            values(&body[offset..], prefix, Mode::Palette, depth + 1)?
                        )
                    } else {
                        values(&body, prefix, Mode::Palette, depth + 1)?
                    }
                } else if matches!(mode, Mode::Alternates)
                    && feature(&name.to_ascii_lowercase())
                    && !name.eq_ignore_ascii_case("historical-forms")
                {
                    values(&body, prefix, Mode::FeatureArgs, depth + 1)?
                } else {
                    values(&body, prefix, Mode::Opaque, depth + 1)?
                };
                let end = parser.position().byte_index();
                if body == rendered {
                    push(&mut out, &source[start..end])?;
                } else {
                    let close = if source.as_bytes().get(end.wrapping_sub(1)) == Some(&b')') {
                        ")"
                    } else {
                        ""
                    };
                    push(
                        &mut out,
                        &format!("{}{rendered}{close}", &source[start..opening]),
                    )?;
                }
            }
            CssToken::ParenthesisBlock
            | CssToken::SquareBracketBlock
            | CssToken::CurlyBracketBlock => {
                let _ = nested_text(&mut parser)?;
                push(&mut out, &source[start..parser.position().byte_index()])?;
            }
            _ => push(&mut out, &source[start..parser.position().byte_index()])?,
        }
    }
    Ok(out)
}
pub(super) fn value(property: &str, source: &str, prefix: &str) -> Result<String> {
    if prefix.is_empty() {
        return Ok(source.into());
    }
    let mode = match property {
        "font-palette" => Mode::Palette,
        "font-variant" | "font-variant-alternates" => Mode::Alternates,
        _ => return Ok(source.into()),
    };
    values(source, prefix, mode, 0)
}
fn feature_declarations(source: &str, prefix: &str) -> Result<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let mut start = 0;
    let mut colon = false;
    loop {
        let before = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            break;
        };
        if matches!(token, CssToken::Colon) && !colon {
            let property = &source[start..before];
            push(
                &mut out,
                &single(property, prefix, false).unwrap_or_else(|| property.into()),
            )?;
            push(&mut out, ":")?;
            start = parser.position().byte_index();
            colon = true;
        } else if matches!(token, CssToken::Semicolon) {
            push(&mut out, &source[start..parser.position().byte_index()])?;
            start = parser.position().byte_index();
            colon = false;
        } else if matches!(
            token,
            CssToken::Function(_)
                | CssToken::CurlyBracketBlock
                | CssToken::SquareBracketBlock
                | CssToken::ParenthesisBlock
        ) {
            let _ = nested_text(&mut parser)?;
        }
    }
    push(&mut out, &source[start..])?;
    Ok(out)
}
pub(super) fn feature_body(source: &str, names: &Names) -> Result<String> {
    if names.prefix.is_empty() {
        return Ok(source.into());
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let mut start = 0;
    loop {
        let before = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            break;
        };
        if matches!(token, CssToken::CurlyBracketBlock) {
            let header = &source[start..before];
            let body = nested_text(&mut parser)?;
            let mut input = ParserInput::new(header);
            let mut inner = Parser::new(&mut input);
            let recognized = matches!(inner.next(),Ok(CssToken::AtKeyword(kind)) if feature(&kind.to_ascii_lowercase()))
                && inner.expect_exhausted().is_ok();
            push(&mut out, header)?;
            push(&mut out, "{")?;
            push(
                &mut out,
                &if recognized {
                    feature_declarations(&body, &names.prefix)?
                } else {
                    body
                },
            )?;
            push(&mut out, "}")?;
            start = parser.position().byte_index();
        } else if matches!(token, CssToken::Semicolon) {
            push(&mut out, &source[start..parser.position().byte_index()])?;
            start = parser.position().byte_index();
        } else if matches!(
            token,
            CssToken::Function(_) | CssToken::SquareBracketBlock | CssToken::ParenthesisBlock
        ) {
            let _ = nested_text(&mut parser)?;
        }
    }
    push(&mut out, &source[start..])?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names() -> Names {
        let mut names = Names::native(HashSet::new(), "owner".into(), true);
        names.prefix = "file_".into();
        names.fonts = HashSet::from(["report".into(), "serif".into()]);
        names
    }
    #[test]
    fn font_css_palette_rule_names_keep_descriptors_and_literal_references_private() {
        let css=namespace_styles(r#"@font-palette-values --Cool {font-family:Report;base-palette:1;override-colors:0 red,1 var(--tone)}.x{font-palette:--Cool!important;--data:--Cool}"#,&names(),0).unwrap();
        assert!(css.contains("@font-palette-values --file_font_palette_Cool"));
        assert!(css.contains(r#"font-family:"file_font_report";base-palette:1;override-colors:0 red,1 var(--file_property_tone)"#));
        assert!(css.contains("font-palette:--file_font_palette_Cool!important"));
        assert!(css.contains("--file_property_data:--Cool"));
        assert_eq!(
            value(
                "font-palette",
                r#"palette-mix(in --profile, --Cool 25%,palette-mix(--Cool,dark))"#,
                "file_"
            )
            .unwrap(),
            "palette-mix(in --profile, --file_font_palette_Cool 25%,palette-mix(--file_font_palette_Cool,dark))"
        );
    }
    #[test]
    fn font_css_feature_families_and_keys_follow_declared_fonts_and_installed_fonts() {
        let css=namespace_styles(r#"@font-feature-values Report,Installed Face{@styleset{Loop:1;loop:2;--tag:3}@swash{Loop:4}font-display:swap;@unknown{Loop:7}}.x{font-variant:small-caps styleset(Loop,loop,--tag) swash(Loop);font-feature-settings:"swsh" 1}"#,&names(),0).unwrap();
        assert!(css.contains(r#"@font-feature-values"file_font_report",Installed Face{"#));
        assert!(css.contains(
            "@styleset{file_font_feature_Loop:1;file_font_feature_loop:2;file_font_feature_--tag:3}"
        ));
        assert!(css.contains("@swash{file_font_feature_Loop:4}font-display:swap;@unknown{Loop:7}"));
        assert!(css.contains("styleset(file_font_feature_Loop,file_font_feature_loop,file_font_feature_--tag) swash(file_font_feature_Loop)"));
        assert!(css.contains(r#"font-feature-settings:"swsh" 1"#));
    }
    #[test]
    fn font_css_fallbacks_queries_escaped_keys_and_opaque_strings_keep_roles() {
        assert_eq!(
            value(
                "font-variant-alternates",
                r#"styleset(\4c oop,"Loop") var(--mode,swash(Loop)) unknown(styleset(Loop))"#,
                "file_"
            )
            .unwrap(),
            r#"styleset(file_font_feature_Loop,"Loop") var(--mode,swash(file_font_feature_Loop)) unknown(styleset(Loop))"#
        );
        let css = namespace_styles(
            "@supports(font-palette:--Cool){.x{font-palette:var(--palette,--Cool)}}",
            &names(),
            0,
        )
        .unwrap();
        assert!(css.contains("@supports(font-palette:--file_font_palette_Cool)"));
        assert!(css.contains("font-palette:var(--file_property_palette,--file_font_palette_Cool)"));
        assert_eq!(
            value("content", r#""--Cool""#, "file_").unwrap(),
            r#""--Cool""#
        );
    }
    #[test]
    fn font_css_generic_families_quoted_names_and_invalid_rules_preserve_validity() {
        assert_eq!(
            names()
                .family("serif, 'serif', Report, sans-serif")
                .unwrap(),
            r#"serif,"file_font_serif","file_font_report", sans-serif"#
        );
        for value in [
            r#""Report" Other"#,
            r#"Report "Other""#,
            "inherit",
            "Report inherit",
        ] {
            assert_eq!(names().family(value).unwrap(), value);
        }
        for header in [
            "@font-palette-values Cool",
            "@font-palette-values --",
            "@font-palette-values --Cool extra",
            "@font-palette-values '--Cool'",
        ] {
            assert_eq!(
                prelude("font-palette-values", header, &names()).unwrap(),
                header
            );
        }
        let invalid = namespace_styles(
            "@font-feature-values serif{@styleset{Loop:-1}}",
            &names(),
            0,
        )
        .unwrap();
        assert!(invalid.starts_with("@font-feature-values serif{"));
        assert!(invalid.contains("file_font_feature_Loop:-1"));
    }
    #[test]
    fn font_css_source_html_remains_native_and_nesting_output_bounds_hold() {
        assert_eq!(
            names()
                .presentation("data-label", "styleset(Loop)")
                .unwrap(),
            "styleset(Loop)"
        );
        assert_eq!(
            names()
                .presentation("font-variant-alternates", "styleset(Loop)")
                .unwrap(),
            "styleset(Loop)"
        );
        let native = Names::native(HashSet::new(), "owner".into(), true);
        let source = "@font-palette-values --Cool{font-family:Report}.x{font-palette:--Cool;font-variant-alternates:swash(Loop)}";
        assert_eq!(
            namespace_styles(source, &native, 0).unwrap(),
            source.replace(".x{", ".x:not(:where([owner])){")
        );
        assert!(
            value(
                "font-palette",
                &format!("{}--Cool{}", "palette-mix(".repeat(34), ")".repeat(34)),
                "file_"
            )
            .is_err()
        );
        assert!(
            value(
                "font-palette",
                &format!("--{}", "x".repeat(MAX_OUTPUT)),
                "file_"
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    fn fixture() -> String {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import 'shared.css';@font-palette-values --Cool{font-family:Report;base-palette:1;override-colors:0 red,1 blue}@font-feature-values Report,Installed Face{@styleset{Loop:1;loop:2}@swash{Loop:4}}.label{font-family:Report,serif;font-palette:--Cool;font-variant-alternates:styleset(Loop,loop) swash(Loop);font-variation-settings:'wght' 600;font-feature-settings:'swsh' 1}</style><symbol id="badge"><text class="label" font-family="Report,serif" font-variant="styleset(Loop)">Report 🚀</text></symbol></svg>"##;
        let shared = r#"@font-face{font-family:Report;src:url(face.woff2)}@font-feature-values Installed Face{@annotation{Note:2}}.label{font-variant:small-caps annotation(Note)}@supports(font-palette:--Cool){.label{font-palette:--Cool}}"#;
        let mut bundle = Bundle {
            resources: [
                ("a.svg", "image/svg+xml", svg.as_bytes().to_vec()),
                (
                    "b.svg",
                    "image/svg+xml",
                    svg.replace("0 red,1 blue", "0 green,1 black").into_bytes(),
                ),
                ("shared.css", "text/css", shared.as_bytes().to_vec()),
                ("face.woff2", "font/woff2", b"wOF2pinned-font".to_vec()),
            ]
            .into_iter()
            .map(|(path, mime, bytes)| (path.into(), (mime, bytes)))
            .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        };
        let html=bundle.html(r##"<html><head><style>@font-palette-values --Cool{font-family:Report;base-palette:0}.owner{font-palette:--Cool;font-variant-alternates:styleset(Loop)}</style></head><body><span class="owner">Owner</span><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
        assert_eq!(bundle.used.len(), 4);
        html
    }
    #[test]
    fn font_css_publication_keeps_two_pinned_documents_private_and_preserves_font_data() {
        let html = fixture();
        let document = roxmltree::Document::parse(&html).unwrap();
        assert!(html.contains("@font-palette-values --Cool{"));
        assert!(html.contains("font-variant-alternates:styleset(Loop)"));
        for file in ["a.svg", "b.svg"] {
            let prefix = format!("ion_svg_{}_", hash(file.as_bytes()));
            assert!(html.contains(&format!("@font-palette-values --{prefix}font_palette_Cool")));
            assert!(html.contains(&format!(
                "styleset({prefix}font_feature_Loop,{prefix}font_feature_loop)"
            )));
            assert!(html.contains(&format!(
                r#"font-family="&quot;{prefix}font_report&quot;,serif"#
            )));
            assert!(html.contains(&format!(
                r#"font-variant="styleset({prefix}font_feature_Loop)"#
            )));
            let css = document
                .descendants()
                .filter(|n| n.has_tag_name("style"))
                .filter_map(|n| n.text())
                .find(|css| css.contains(&prefix) && css.contains("data:text/css;base64,"))
                .unwrap();
            let encoded = css
                .split("data:text/css;base64,")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let imported = String::from_utf8(STANDARD.decode(encoded).unwrap()).unwrap();
            assert!(imported.contains(&format!(r#"font-family:"{prefix}font_report""#)));
            assert!(imported.contains(&format!("@annotation{{{prefix}font_feature_Note:2}}")));
            assert!(imported.contains(&format!("annotation({prefix}font_feature_Note)")));
            assert!(imported.contains(&format!(
                "@supports(font-palette:--{prefix}font_palette_Cool)"
            )));
            assert!(imported.contains("data:font/woff2;base64,"));
        }
        assert_eq!(
            document
                .descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .filter(|text| *text == "Report 🚀")
                .count(),
            2
        );
        assert!(
            document
                .descendants()
                .filter(|n| n.has_tag_name("style"))
                .filter_map(|n| n.text())
                .any(|css| css.contains(
                    r#"font-variation-settings:"wght" 600;font-feature-settings:"swsh" 1"#
                ))
        );
    }
    #[test]
    #[ignore = "Exports production native font CSS to ION_FONT_CSS_FIXTURE"]
    fn native_font_css_phone_fixture() {
        std::fs::write(
            std::env::var_os("ION_FONT_CSS_FIXTURE").expect("Set ION_FONT_CSS_FIXTURE"),
            fixture(),
        )
        .unwrap();
    }
}
