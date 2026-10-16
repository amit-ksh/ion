//! Keep imported custom-property identities private; the browser still performs
//! native substitution, dependency-cycle handling, registration and animation.
use super::*;

fn custom(name: &str) -> bool {
    name.starts_with("--") && name.len() > 2
}
fn identifier(name: &str, prefix: &str) -> String {
    CssToken::Ident(format!("--{prefix}property_{}", &name[2..]).into()).to_css_string()
}
fn first_ident(source: &str) -> Option<(usize, usize, String)> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    loop {
        let start = parser.position().byte_index();
        match parser
            .next_including_whitespace_and_comments()
            .ok()?
            .clone()
        {
            CssToken::WhiteSpace(_) | CssToken::Comment(_) => {}
            CssToken::Ident(name) => {
                return Some((start, parser.position().byte_index(), name.to_string()));
            }
            _ => return None,
        }
    }
}
fn single(source: &str, prefix: &str) -> Option<String> {
    let (start, end, name) = first_ident(source)?;
    if !custom(&name) {
        return None;
    }
    let mut input = ParserInput::new(&source[end..]);
    Parser::new(&mut input).expect_exhausted().ok()?;
    Some(format!(
        "{}{}{}",
        &source[..start],
        identifier(&name, prefix),
        &source[end..]
    ))
}
pub(super) fn custom_declaration(source: &str) -> bool {
    let Some((_, end, name)) = first_ident(source).filter(|(_, _, name)| custom(name)) else {
        return false;
    };
    let _ = name;
    let mut input = ParserInput::new(&source[end..]);
    matches!(Parser::new(&mut input).next(), Ok(CssToken::Colon))
}
pub(super) fn declaration(source: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return source.into();
    }
    single(source, prefix).unwrap_or_else(|| source.into())
}
fn property_references(property: &str) -> bool {
    matches!(
        property,
        "transition"
            | "transition-property"
            | "-webkit-transition"
            | "-webkit-transition-property"
            | "will-change"
    )
}
pub(super) fn value(source: &str, prefix: &str, property: &str) -> Result<String> {
    if prefix.is_empty() {
        return Ok(source.into());
    }
    values(source, prefix, property_references(property), 0)
}
fn variable(source: &str, prefix: &str, references: bool, depth: usize) -> Result<String> {
    let mut argument_input = ParserInput::new(source);
    let mut argument = Parser::new(&mut argument_input);
    let argument_start = argument.position().byte_index();
    if matches!(argument.next().cloned(), Ok(CssToken::Function(name)) if name.eq_ignore_ascii_case("ident"))
    {
        let body = nested_text(&mut argument)?;
        let end = argument.position().byte_index();
        let tail = match argument.next().cloned() {
            Ok(CssToken::Comma) => {
                let offset = argument.position().byte_index();
                format!(
                    "{}{}",
                    &source[end..offset],
                    values(&source[offset..], prefix, references, depth + 1)?
                )
            }
            Err(_) => source[end..].into(),
            _ => return Ok(source.into()),
        };
        let mapped = computed_property(&body, prefix)?;
        let mapped = values(&mapped, prefix, false, depth + 1)?;
        return Ok(format!(
            "{}ident({mapped}){tail}",
            &source[..argument_start]
        ));
    }
    let Some((start, end, name)) = first_ident(source).filter(|(_, _, name)| custom(name)) else {
        return Ok(source.into());
    };
    let mut input = ParserInput::new(&source[end..]);
    let mut parser = Parser::new(&mut input);
    let tail = match parser.next().cloned() {
        Ok(CssToken::Comma) => {
            let offset = end + parser.position().byte_index();
            format!(
                "{}{}",
                &source[end..offset],
                values(&source[offset..], prefix, references, depth + 1)?
            )
        }
        Err(_) => source[end..].into(),
        _ => return Ok(source.into()), // Keep invalid var() argument grammar invalid.
    };
    Ok(format!(
        "{}{}{tail}",
        &source[..start],
        identifier(&name, prefix)
    ))
}
fn computed_property(source: &str, prefix: &str) -> Result<String> {
    if let Some(value) = computed_names::literal(source)? {
        return Ok(if custom(&value) {
            computed_names::quoted(&format!("--{prefix}property_{}", &value[2..]))
        } else {
            source.into()
        });
    }
    Ok(
        computed_names::affix(source, prefix, "property", true, &[])?
            .unwrap_or_else(|| source.into()),
    )
}
fn values(source: &str, prefix: &str, references: bool, depth: usize) -> Result<String> {
    ensure!(depth <= 32, "CSS custom-property nesting exceeds its bound");
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    loop {
        let start = parser.position().byte_index();
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            break;
        };
        match token {
            CssToken::Ident(name) if references && custom(&name) => {
                push(&mut out, &identifier(&name, prefix))?
            }
            CssToken::Function(name) => {
                let opening = parser.position().byte_index();
                let body = nested_text(&mut parser)?;
                let rendered = if name.eq_ignore_ascii_case("var") {
                    variable(&body, prefix, references, depth)?
                } else if references && name.eq_ignore_ascii_case("ident") {
                    values(&computed_property(&body, prefix)?, prefix, false, depth + 1)?
                } else {
                    values(&body, prefix, false, depth + 1)?
                };
                let end = parser.position().byte_index();
                if rendered == body {
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
                let opening = parser.position().byte_index();
                let close = match token {
                    CssToken::ParenthesisBlock => ')',
                    CssToken::SquareBracketBlock => ']',
                    _ => '}',
                };
                let body = nested_text(&mut parser)?;
                let rendered = values(&body, prefix, false, depth + 1)?;
                let end = parser.position().byte_index();
                if rendered == body {
                    push(&mut out, &source[start..end])?;
                } else {
                    let tail = if source.as_bytes().get(end.wrapping_sub(1)) == Some(&(close as u8))
                    {
                        &source[end - 1..end]
                    } else {
                        ""
                    };
                    push(
                        &mut out,
                        &format!("{}{rendered}{tail}", &source[start..opening]),
                    )?;
                }
            }
            _ => push(&mut out, &source[start..parser.position().byte_index()])?,
        }
    }
    Ok(out)
}
#[derive(Clone, Copy, PartialEq)]
enum Query {
    Supports,
    Container,
    Import,
}
fn condition(
    source: &str,
    prefix: &str,
    kind: Query,
    feature: bool,
    depth: usize,
) -> Result<String> {
    ensure!(
        depth <= 32,
        "CSS custom-property condition nesting exceeds its bound"
    );
    if feature && let Some((start, end, name)) = first_ident(source) {
        let mut input = ParserInput::new(&source[end..]);
        let mut parser = Parser::new(&mut input);
        let colon = matches!(parser.next(), Ok(CssToken::Colon));
        if colon {
            let offset = end + parser.position().byte_index();
            let property = if custom(&name) {
                identifier(&name, prefix)
            } else {
                source[start..end].into()
            };
            return Ok(format!(
                "{}{property}{}{}",
                &source[..start],
                &source[end..offset],
                value(
                    &font_styles::value(&name.to_ascii_lowercase(), &source[offset..], prefix)?,
                    prefix,
                    &name.to_ascii_lowercase()
                )?
            ));
        }
        if custom(&name) && kind == Query::Container && parser.is_exhausted() {
            return Ok(format!(
                "{}{}{}",
                &source[..start],
                identifier(&name, prefix),
                &source[end..]
            ));
        }
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::ParenthesisBlock => {
                let body = condition(
                    &nested_text(&mut parser)?,
                    prefix,
                    kind,
                    kind == Query::Supports || feature,
                    depth + 1,
                )?;
                push(&mut out, &format!("({body})"))?;
            }
            CssToken::Function(name) => {
                let body = nested_text(&mut parser)?;
                let body = if kind == Query::Container && name.eq_ignore_ascii_case("style") {
                    condition(&body, prefix, kind, true, depth + 1)?
                } else if kind == Query::Import && name.eq_ignore_ascii_case("supports") {
                    condition(&body, prefix, Query::Supports, true, depth + 1)?
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
pub(super) fn prelude(kind: &str, source: &str, prefix: &str) -> Result<String> {
    if prefix.is_empty() {
        return Ok(source.into());
    }
    if kind == "property" {
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        if matches!(parser.next(),Ok(CssToken::AtKeyword(name))if name.eq_ignore_ascii_case("property"))
        {
            let offset = parser.position().byte_index();
            if let Some(tail) = single(&source[offset..], prefix) {
                return Ok(format!("{}{tail}", &source[..offset]));
            }
        }
        return Ok(source.into());
    }
    match kind {
        "supports" => condition(source, prefix, Query::Supports, false, 0),
        "container" => condition(source, prefix, Query::Container, false, 0),
        "import" => condition(source, prefix, Query::Import, false, 0),
        _ => Ok(source.into()),
    }
}
pub(super) fn presentation(name: &str, source: &str, prefix: &str) -> Result<String> {
    if matches!(
        name,
        "alignment-baseline"
            | "baseline-shift"
            | "clip"
            | "clip-path"
            | "clip-rule"
            | "color"
            | "color-interpolation"
            | "color-interpolation-filters"
            | "color-profile"
            | "color-rendering"
            | "cursor"
            | "direction"
            | "display"
            | "dominant-baseline"
            | "fill"
            | "fill-opacity"
            | "fill-rule"
            | "filter"
            | "flood-color"
            | "flood-opacity"
            | "font-family"
            | "font-size"
            | "font-size-adjust"
            | "font-stretch"
            | "font-style"
            | "font-variant"
            | "font-weight"
            | "glyph-orientation-horizontal"
            | "glyph-orientation-vertical"
            | "image-rendering"
            | "letter-spacing"
            | "lighting-color"
            | "marker-end"
            | "marker-mid"
            | "marker-start"
            | "mask"
            | "opacity"
            | "overflow"
            | "paint-order"
            | "pointer-events"
            | "shape-rendering"
            | "stop-color"
            | "stop-opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-miterlimit"
            | "stroke-opacity"
            | "stroke-width"
            | "text-anchor"
            | "text-decoration"
            | "text-rendering"
            | "transform"
            | "transform-origin"
            | "unicode-bidi"
            | "vector-effect"
            | "visibility"
            | "word-spacing"
            | "writing-mode"
            | "x"
            | "y"
            | "cx"
            | "cy"
            | "r"
            | "rx"
            | "ry"
            | "width"
            | "height"
            | "d"
    ) {
        value(source, prefix, name)
    } else {
        Ok(source.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names() -> Names {
        let mut names = Names::native(HashSet::new(), "owner".into(), true);
        names.prefix = "file_".into();
        names
    }
    #[test]
    fn property_css_case_escapes_unicode_comments_and_reserved_name_keep_identity() {
        assert_eq!(
            declaration(" /* keep */ --Tone/**/ ", "file_"),
            " /* keep */ --file_property_Tone/**/ "
        );
        assert_eq!(declaration(r"--\54 one", "file_"), "--file_property_Tone");
        assert_eq!(declaration("--é", "file_"), "--file_property_é");
        assert_ne!(declaration("--é", "file_"), declaration("--é", "file_"));
        for invalid in ["--", "--a --b", r#""--a""#, "color"] {
            assert_eq!(declaration(invalid, "file_"), invalid);
        }
        let css = declarations(
            "--Tone:RED;--tone:blue;color:var(--Tone);fill:var(--tone)",
            &names(),
        )
        .unwrap();
        assert!(css.contains("--file_property_Tone:RED"));
        assert!(css.contains("--file_property_tone:blue"));
        assert!(css.contains("var(--file_property_Tone)"));
        assert!(css.contains("var(--file_property_tone)"));
    }
    #[test]
    fn property_css_native_fallback_cycles_priority_and_opaque_data_are_retained() {
        let css=declarations(r#"--a:var(--b,var(--missing,));--b:var(--a);--text:"var(--a)";--data:--a;color:VAR( /*var*/ --a/**/,var(--b,red))!important"#,&names()).unwrap();
        assert!(
            css.contains("--file_property_a:var(--file_property_b,var(--file_property_missing,))")
        );
        assert!(css.contains("--file_property_b:var(--file_property_a)"));
        assert!(css.contains(r#"--file_property_text:"var(--a)""#));
        assert!(css.contains("--file_property_data:--a"));
        assert!(
            css.contains(
                "VAR( /*var*/ --file_property_a/**/,var(--file_property_b,red))!important"
            )
        );
        for invalid in [
            "var(--,red)",
            "var(not-custom,red)",
            "var(--a extra,red)",
            "var('--a',red)",
        ] {
            assert_eq!(value(invalid, "file_", "fill").unwrap(), invalid);
        }
        assert_eq!(
            value("fn(--a, var(--b))", "file_", "transition-property").unwrap(),
            "fn(--a, var(--file_property_b))"
        );
        assert_eq!(
            value("env(--a,var(--b))", "file_", "fill").unwrap(),
            "env(--a,var(--file_property_b))"
        );
        let animated = namespace_styles(
            "@keyframes fade{from{--Tone:red}to{--Tone:var(--tone,blue)}}",
            &names(),
            0,
        )
        .unwrap();
        assert!(animated.contains("@keyframes file_animation_fade{from{--file_property_Tone:red}to{--file_property_Tone:var(--file_property_tone,blue)}}"));
        let nested=namespace_styles("path:hover{--data:{keep:var(--Tone);nested:{literal:--Tone}};fill:var(--Tone);@media all{path{opacity:var(--alpha)}}}",&names(),0).unwrap();
        assert!(nested.contains(
            "--file_property_data:{keep:var(--file_property_Tone);nested:{literal:--Tone}}"
        ));
        assert!(nested.contains("fill:var(--file_property_Tone)"));
        assert!(nested.contains("opacity:var(--file_property_alpha)"));
        assert!(!nested.contains("property_file_property"));
    }
    #[test]
    fn property_css_registrations_keep_descriptors_order_and_native_invalidity() {
        let source = r#"@PROPERTY/**/--Tone {syntax:"<color>";inherits:false;initial-value:RED}@property --Tone{syntax:"<number>";inherits:true;initial-value:1}@property --missing{syntax:"*"}@property -- {syntax:"<color>";inherits:false;initial-value:green}@property --a extra{syntax:"*";inherits:true}"#;
        let css = namespace_styles(source, &names(), 0).unwrap();
        assert!(css.contains(r#"@PROPERTY/**/--file_property_Tone {syntax:"<color>";inherits:false;initial-value:RED}"#));
        assert!(css.contains(
            r#"@property --file_property_Tone{syntax:"<number>";inherits:true;initial-value:1}"#
        ));
        assert!(css.contains(r#"@property --file_property_missing{syntax:"*"}"#)); // Missing descriptors stay missing.
        assert!(css.contains("@property -- {"));
        assert!(css.contains("@property --a extra{"));
        assert!(!css.contains("--file_property_file_property"));
    }
    #[test]
    fn property_css_queries_import_conditions_and_literal_property_references_follow_private_names()
    {
        let css=namespace_styles(r#"@supports ((--Tone:var(--tone)) and (transition-property:--Tone)) or selector([data-name="--Tone"]){.x{--Tone:red}}@container card style((--Tone:var(--tone)) and (--Flag)){.x{transition:--Tone 1s ease,--tone 2s;will-change:--Tone,transform;--data:--Tone}}"#,&names(),0).unwrap();
        assert!(css.contains("(--file_property_Tone:var(--file_property_tone))"));
        assert!(css.contains("(transition-property:--file_property_Tone)"));
        assert!(css.contains(r#"selector([data-name="--Tone"])"#));
        assert!(css.contains(
            "style((--file_property_Tone:var(--file_property_tone)) and (--file_property_Flag))"
        ));
        assert!(css.contains("transition:--file_property_Tone 1s ease,--file_property_tone 2s"));
        assert!(css.contains("will-change:--file_property_Tone,transform"));
        assert!(css.contains("--file_property_data:--Tone"));
        assert_eq!(
            prelude(
                "import",
                r#"@import "safe.css" layer(base) supports(--Tone:red) screen"#,
                "file_"
            )
            .unwrap(),
            r#"@import "safe.css" layer(base) supports(--file_property_Tone:red) screen"#
        );
        assert_eq!(
            prelude("container", "@container card style(--Flag)", "file_").unwrap(),
            "@container card style(--file_property_Flag)"
        );
    }
    #[test]
    fn property_css_presentation_values_keep_geometry_and_non_css_attributes() {
        let n = names();
        for (property, raw) in [
            ("d", "M0 0L20 20"),
            ("transform", "translate(1.00,2e1)"),
            ("fill", r#""var(--Tone)""#),
            ("data-label", "var(--Tone)"),
            ("aria-label", "var(--Tone)"),
        ] {
            assert_eq!(n.presentation(property, raw).unwrap(), raw);
        }
        for property in ["fill", "opacity", "stop-color", "x", "font-family"] {
            assert_eq!(
                n.presentation(property, "var(--Tone,var(--tone,red))")
                    .unwrap(),
                "var(--file_property_Tone,var(--file_property_tone,red))"
            );
        }
        assert_eq!(
            n.presentation("transform", r"translate(calc(var(--x) * 1.00),2e1)")
                .unwrap(),
            "translate(calc(var(--file_property_x) * 1.00),2e1)"
        );
    }
    #[test]
    fn property_css_source_html_stays_native_and_depth_output_bounds_hold() {
        let native = Names::native(HashSet::new(), "owner".into(), true);
        let source = r#"@property --Tone{syntax:"<color>";inherits:true;initial-value:red}.owner{--Tone:blue;color:var(--Tone);transition-property:--Tone}"#;
        let result = namespace_styles(source, &native, 0).unwrap();
        assert!(result.contains("@property --Tone{"));
        assert!(result.contains("--Tone:blue;color:var(--Tone);transition-property:--Tone"));
        assert!(
            value(
                &format!("{}var(--a){}", "calc(".repeat(34), ")".repeat(34)),
                "file_",
                "fill"
            )
            .is_err()
        );
        assert!(
            prelude(
                "supports",
                &format!("{}--a:red{}", "(".repeat(34), ")".repeat(34)),
                "file_"
            )
            .is_err()
        );
        assert!(
            value(
                &format!("var(--{})", "x".repeat(MAX_OUTPUT)),
                "file_",
                "fill"
            )
            .is_err()
        );
    }
}
#[cfg(test)]
mod publication_tests {
    use super::*;
    fn fixture() -> String {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" style="--Tone:green;--tone:var(--Tone);--opacity:.7"><style>@import 'shared.css' supports(--Tone:green);@property --Tone{syntax:"&lt;color&gt;";inherits:false;initial-value:green}:root{--Tone:green;--fallback:var(--missing,var(--Tone));--cycle:var(--cycle)}@supports (--Tone:green){.shape{fill:var(--Tone)}}</style><symbol id="badge"><path class="shape" d="M0 0L20 20" fill="var(--Tone)" opacity="var(--opacity)" data-label="var(--Tone)"/><text font-family="var(--family,serif)">Report</text></symbol></svg>"##;
        let shared = r#"@property --opacity{syntax:"<number>";inherits:true;initial-value:1}.shape{fill:var(--tone,var(--Tone,red));opacity:var(--opacity);transition-property:--Tone;will-change:--opacity;--text:"--Tone";--data:--Tone}@container card style(--Tone:green){.shape{stroke:var(--tone)}}@container card style(--Flag:ready){.shape{stroke-width:2}}"#;
        let mut bundle = Bundle {
            resources: [
                ("a.svg", "image/svg+xml", svg.to_owned()),
                ("b.svg", "image/svg+xml", svg.replace("green", "blue")),
                ("shared.css", "text/css", shared.to_owned()),
            ]
            .into_iter()
            .map(|(path, mime, text)| (path.into(), (mime, text.into_bytes())))
            .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        };
        let html=bundle.html(r##"<html><head><style>@property --Tone{syntax:"*";inherits:true;initial-value:black}.owner{--Tone:black;color:var(--Tone)}</style></head><body><span class="owner">Owner</span><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
        assert_eq!(bundle.used.len(), 3);
        html
    }
    #[test]
    fn property_css_publication_keeps_two_pinned_documents_private_and_preserves_values() {
        let html = fixture();
        let document = roxmltree::Document::parse(&html).unwrap();
        assert!(html.contains("@property --Tone{"));
        assert!(html.contains("color:var(--Tone)"));
        for file in ["a.svg", "b.svg"] {
            let prefix = format!("ion_svg_{}_", hash(file.as_bytes()));
            assert!(html.contains(&format!("@property --{prefix}property_Tone")));
            assert!(html.contains(&format!(r#"fill="var(--{prefix}property_Tone)""#)));
            assert!(html.contains(&format!(r#"opacity="var(--{prefix}property_opacity)""#)));
            assert!(
                html.contains(&format!("supports(--{prefix}property_Tone:green)"))
                    || html.contains(&format!("supports(--{prefix}property_Tone:blue)"))
            );
            let styles: Vec<_> = document
                .descendants()
                .filter(|n| n.has_tag_name("style"))
                .filter_map(|n| n.text())
                .filter(|css| css.contains(&format!("property_Tone")) && css.contains(&prefix))
                .collect();
            let css = styles
                .iter()
                .find(|css| css.contains("data:text/css;base64,"))
                .unwrap();
            let encoded = css
                .split("data:text/css;base64,")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let imported = String::from_utf8(STANDARD.decode(encoded).unwrap()).unwrap();
            assert!(imported.contains(&format!("@property --{prefix}property_opacity")));
            assert!(imported.contains(&format!(
                "var(--{prefix}property_tone,var(--{prefix}property_Tone,red))"
            )));
            assert!(imported.contains(&format!("transition-property:--{prefix}property_Tone")));
            assert!(imported.contains(&format!("style(--{prefix}property_Tone:green)")));
            assert!(imported.contains(r#"--Tone""#));
        }
        assert_eq!(
            document
                .descendants()
                .filter_map(|n| n.attribute("d"))
                .filter(|value| *value == "M0 0L20 20")
                .count(),
            2
        );
        assert_eq!(
            document
                .descendants()
                .filter_map(|n| n.attribute("data-label"))
                .filter(|value| *value == "var(--Tone)")
                .count(),
            2
        );
    }
    #[test]
    #[ignore = "Exports actual production native CSS properties to ION_PROPERTY_CSS_FIXTURE"]
    fn native_property_css_phone_fixture() {
        std::fs::write(
            std::env::var_os("ION_PROPERTY_CSS_FIXTURE").expect("Set ION_PROPERTY_CSS_FIXTURE"),
            fixture(),
        )
        .unwrap();
    }
}
