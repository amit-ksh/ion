//! Preserve arbitrary-substitution wrappers while changing proven name roles.
//! Never substitute custom-property values or repair invalid ident() arguments.
use super::*;

const WIDE: &[&str] = &[
    "initial",
    "inherit",
    "unset",
    "revert",
    "revert-layer",
    "revert-rule",
    "default",
];
pub(super) fn reserved(value: &str, extra: &[&str]) -> bool {
    WIDE.iter()
        .chain(extra)
        .any(|word| value.eq_ignore_ascii_case(word))
}
pub(super) fn quoted(value: &str) -> String {
    CssToken::QuotedString(value.into()).to_css_string()
}
/// A conservative literal subset. Decimal/exponent/math integers stay native;
/// cssparser's saturating integer metadata alone cannot prove their exact value.
pub(super) fn literal(source: &str) -> Result<Option<String>> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let mut saw = false;
    loop {
        let start = parser.position();
        let Ok(token) = parser.next().cloned() else {
            break;
        };
        match token {
            CssToken::Ident(value) | CssToken::QuotedString(value) => push(&mut out, &value)?,
            CssToken::Number {
                int_value: Some(value),
                ..
            } if parser.slice_from(start).trim().parse::<i32>().ok() == Some(value) => {
                push(&mut out, &value.to_string())?;
            }
            _ => return Ok(None),
        }
        saw = true;
    }
    Ok((saw && !out.is_empty()).then_some(out))
}
/// Add an affix inside an existing first literal. Remaining arguments are still
/// evaluated by the browser. A fixed nonempty prefix must rule out *all*
/// property keywords; otherwise a dynamic suffix could produce `none`, etc.
pub(super) fn affix(
    source: &str,
    prefix: &str,
    kind: &str,
    dashed: bool,
    keywords: &[&str],
) -> Result<Option<String>> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let start = parser.position().byte_index();
    let Ok(first) = parser.next().cloned() else {
        return Ok(None);
    };
    let (first_start, first_end, first) = match first {
        CssToken::Ident(name) | CssToken::QuotedString(name) => {
            // parser.next skips comments/whitespace; derive the token start
            // separately so leading authored trivia survives the rewrite.
            let end = parser.position().byte_index();
            let mut input = ParserInput::new(&source[start..end]);
            let mut lead = Parser::new(&mut input);
            let token_start = loop {
                let before = lead.position().byte_index();
                match lead.next_including_whitespace_and_comments() {
                    Ok(CssToken::WhiteSpace(_) | CssToken::Comment(_)) => {}
                    Ok(_) => break start + before,
                    Err(_) => return Ok(None),
                }
            };
            (token_start, end, name.to_string())
        }
        _ => return Ok(None),
    };
    if first.is_empty() || (dashed && (!first.starts_with("--") || first.len() <= 2)) {
        return Ok(None);
    }
    let lower = first.to_ascii_lowercase();
    if !dashed
        && WIDE
            .iter()
            .chain(keywords)
            .any(|word| word.starts_with(&lower))
    {
        return Ok(None);
    }
    // Preserve invalid argument grammar, including commas and noninteger numbers.
    while let Ok(token) = parser.next().cloned() {
        match token {
            CssToken::Ident(_) | CssToken::QuotedString(_) => {}
            CssToken::Number {
                int_value: Some(_), ..
            } => {}
            CssToken::Function(_) => {
                nested_text(&mut parser)?;
            }
            _ => return Ok(None),
        }
    }
    let renamed = if dashed {
        format!("--{prefix}{kind}_{}", &first[2..])
    } else {
        format!("{prefix}{kind}_{first}")
    };
    Ok(Some(format!(
        "{}{}{}",
        &source[..first_start],
        quoted(&renamed),
        &source[first_end..]
    )))
}
/// Only the first argument and comma are consumed. The fallback's token stream
/// remains at its authored location and retains native selection/cycle behavior.
pub(super) fn fallback_offset(source: &str) -> Option<usize> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    if !matches!(parser.next(), Ok(CssToken::Ident(name)) if name.starts_with("--") && name.len()>2)
    {
        return None;
    }
    matches!(parser.next(), Ok(CssToken::Comma)).then(|| parser.position().byte_index())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn computed_ident_literals_preserve_decoding_and_exact_integer_semantics() {
        for (value, expected) in [
            (r#""p\75" lse"#, "pulse"),
            ("-- Cool +0002", "--Cool2"),
            ("'ΟΣ' -0", "ΟΣ0"),
            ("'5'", "5"),
        ] {
            assert_eq!(literal(value).unwrap().as_deref(), Some(expected));
        }
        for value in [
            "",
            "''",
            "a,b",
            "5px",
            "1.0",
            "1e2",
            "2147483648",
            "var(--x)",
            "calc(1)",
            "{}",
            "a !x",
        ] {
            assert!(literal(value).unwrap().is_none(), "{value}");
        }
    }
    #[test]
    fn computed_ident_affixes_keep_unknown_arguments_and_invalid_values_native() {
        assert_eq!(
            affix(
                r#" /*lead*/ "pulse-" var(--suffix)"#,
                "file_",
                "animation",
                false,
                &["none"]
            )
            .unwrap()
            .unwrap(),
            r#" /*lead*/ "file_animation_pulse-" var(--suffix)"#
        );
        assert_eq!(
            affix(
                r#""--Cool" var(--suffix)"#,
                "file_",
                "font_palette",
                true,
                &[]
            )
            .unwrap()
            .unwrap(),
            r#""--file_font_palette_Cool" var(--suffix)"#
        );
        for value in [
            r#""n" var(--x)"#,
            r#""" var(--x)"#,
            "var(--x)",
            "pulse,spin",
            "pulse 2px",
        ] {
            assert!(
                affix(value, "file_", "animation", false, &["none"])
                    .unwrap()
                    .is_none(),
                "{value}"
            );
        }
        assert!(
            affix(r#""--" var(--x)"#, "file_", "property", true, &[])
                .unwrap()
                .is_none()
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    fn names() -> Names {
        let mut names = Names::native(HashSet::new(), "owner".into(), true);
        names.prefix = "file_".into();
        names
            .fonts
            .extend(["report".into(), "report display".into(), "strasse".into()]);
        names
    }
    #[test]
    fn computed_names_follow_property_roles_and_keep_custom_data_opaque() {
        let css = declarations(r#"animation-name:ident("pu" lse),ident("pulse-" var(--suffix)),var(--missing,pulse);font-palette:ident("--" Cool);font-variant-alternates:styleset(ident("Lo" op),ident("Loop-" var(--suffix)));container-name:ident("ca" rd) ident("card-" var(--suffix));transition-property:ident("--" Tone);--data:ident("pulse");content:ident("pulse");"#,&names()).unwrap();
        for expected in [
            r#"ident("file_animation_pulse")"#,
            r#"ident("file_animation_pulse-" var(--file_property_suffix))"#,
            "var(--file_property_missing,file_animation_pulse)",
            r#"ident("--file_font_palette_Cool")"#,
            r#"styleset(ident("file_font_feature_Loop"),ident("file_font_feature_Loop-" var(--file_property_suffix)))"#,
            r#"container-name:ident("file_container_card") ident("file_container_card-" var(--file_property_suffix))"#,
            r#"transition-property:ident("--file_property_Tone")"#,
            r#"--file_property_data:ident("pulse")"#,
            r#"content:ident("pulse")"#,
        ] {
            assert!(css.contains(expected), "{expected} not in {css}");
        }
    }
    #[test]
    fn computed_font_families_match_complete_names_and_preserve_native_fallbacks() {
        let n = names();
        for (source, expected) in [
            (r#"ident("Re" port)"#, r#"ident("file_font_report")"#),
            (
                r#"ident("Report") Display"#,
                r#"ident("file_font_report display")"#,
            ),
            (r#"ident("STR" ASSE)"#, r#"ident("file_font_strasse")"#),
            (
                r#"var(--missing,Report),serif"#,
                r#"var(--missing,"file_font_report"),serif"#,
            ),
        ] {
            assert_eq!(n.family_value(source).unwrap(), expected);
        }
        for source in [
            r#"var(--name,Report) Display"#,
            r#"Report var(--name,Display)"#,
            r#"ident("Installed" " Face")"#,
            r#"ident("serif")"#,
            r#"ident("inherit")"#,
            r#"ident("revert-rule")"#,
            r#"ident("Report-" var(--suffix))"#,
            r#""Report" ident("Display")"#,
            r#"ident("")"#,
            r#"ident(Report,Display)"#,
            r#"ident(2px)"#,
        ] {
            assert_eq!(n.family_value(source).unwrap(), source, "{source}");
        }
        assert_eq!(
            n.family(r#"ident("Report")"#).unwrap(),
            r#"ident("Report")"#
        );
    }
    #[test]
    fn computed_dynamic_property_names_keep_var_wrapper_and_keyword_semantics() {
        let css=declarations(r#"color:var(ident("--" Tone),red);fill:var(ident("--Tone-" var(--suffix)),blue);transition-property:ident("color"),ident("--Tone-" var(--suffix));animation-name:ident("none"),ident("n" var(--suffix)),ident("initial"),ident(2px);container-name:ident("and")"#,&names()).unwrap();
        for expected in [
            r#"var(ident("--file_property_Tone"),red)"#,
            r#"var(ident("--file_property_Tone-" var(--file_property_suffix)),blue)"#,
            r#"transition-property:ident("color"),ident("--file_property_Tone-" var(--file_property_suffix))"#,
            r#"animation-name:ident("none"),ident("n" var(--file_property_suffix)),ident("initial"),ident(2px)"#,
            r#"container-name:ident("and")"#,
        ] {
            assert!(css.contains(expected), "{expected} not in {css}");
        }
        // New functions remain invalid in unsupported rule-name/descriptor syntax.
        let style=namespace_styles(r#"@keyframes ident("pulse"){from{opacity:0}}@property ident("--Tone"){syntax:"*";inherits:true}@font-face{font-family:ident("Report");src:url(data:font/woff2;base64,eA==)}"#,&names(),0).unwrap();
        assert!(style.contains(r#"@keyframes ident("pulse")"#));
        assert!(style.contains(r#"@property ident("--Tone")"#));
        assert!(style.contains(r#"font-family:ident("Report")"#));
        assert_eq!(
            declarations(
                r#"animation:1s var(--x,linear) ident("linear");container:ident("card") / size"#,
                &names()
            )
            .unwrap(),
            r#"animation:1s var(--file_property_x,linear) ident("linear");container:ident("card") / size"#
        );
    }
    #[test]
    fn computed_css_depth_and_output_expansion_are_bounded() {
        let n = names();
        let source = format!("{}pulse{}", "var(--x,".repeat(34), ")".repeat(34));
        assert!(declarations(&format!("animation-name:{source}"), &n).is_err());
        let source = format!("{}Report{}", "var(--x,".repeat(34), ")".repeat(34));
        assert!(n.family_value(&source).is_err());
        assert!(literal(&format!("'{}'", "x".repeat(MAX_OUTPUT + 1))).is_err());
    }
    fn fixture() -> String {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import 'computed.css';@keyframes pulse{from{opacity:.2}to{opacity:1}}@font-palette-values --Cool{font-family:Report;base-palette:0}@font-feature-values Report{@styleset{Loop:1}}@property --Tone{syntax:"&lt;color&gt;";inherits:true;initial-value:green}:root{--suffix:tail;--text:ident("pulse");--font:Report}.shape{animation-name:ident("pu" lse),ident("pulse-" var(--suffix)),var(--missing,pulse);container-name:ident("card-" var(--suffix));transition-property:ident("--" Tone);fill:var(ident("--" Tone),green);font-family:ident("Re" port),serif;font-palette:ident("--" Cool);font-variant-alternates:styleset(ident("Lo" op))}.shape::before{content:'pulse Report --Tone'}</style><symbol id="badge"><path class="shape" d="M0 0L1 1"/><text class="shape" font-family="ident('Report')">Report 🚀</text></symbol></svg>"##;
        let shared = r#"@font-face{font-family:Report;src:url(face.woff2)}.shape{font-family:var(--unknown,Report),serif;--opaque:ident("Report");--partial:var(--font);font-palette:ident("--Cool-" var(--suffix));font-variant-alternates:styleset(ident("Loop-" var(--suffix)));animation-name:ident("none"),ident("n" var(--suffix))}"#;
        let mut bundle = Bundle {
            resources: [
                ("a.svg", "image/svg+xml", svg.as_bytes().to_vec()),
                (
                    "b.svg",
                    "image/svg+xml",
                    svg.replace("base-palette:0", "base-palette:1").into_bytes(),
                ),
                ("computed.css", "text/css", shared.as_bytes().to_vec()),
                ("face.woff2", "font/woff2", b"wOF2pinned-font".to_vec()),
            ]
            .into_iter()
            .map(|(path, mime, bytes)| (path.into(), (mime, bytes)))
            .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        };
        let html=bundle.html(r##"<html><head><style>.owner{animation-name:ident("pulse");font-family:ident("Report"),serif}</style></head><body><div class="owner">Owner</div><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
        assert_eq!(bundle.used.len(), 4);
        let parsed = roxmltree::Document::parse(&html).unwrap();
        for file in ["a.svg", "b.svg"] {
            let prefix = format!("ion_svg_{}_", hash(file.as_bytes()));
            assert!(html.contains(&format!("ident(&quot;{prefix}animation_pulse&quot;)")));
            assert!(
                parsed
                    .descendants()
                    .filter(|n| n.has_tag_name("text"))
                    .any(|n| n.attribute("font-family")
                        == Some(format!("ident(\"{prefix}font_report\")").as_str()))
            );
        }
        html
    }
    #[test]
    fn computed_css_publication_keeps_two_imported_documents_distinct() {
        fixture();
    }
    #[test]
    #[ignore = "Exports a production native computed-name fixture only to ION_COMPUTED_CSS_FIXTURE"]
    fn native_computed_css_phone_fixture() {
        let path =
            std::env::var_os("ION_COMPUTED_CSS_FIXTURE").expect("Set ION_COMPUTED_CSS_FIXTURE");
        std::fs::write(path, fixture()).unwrap();
    }
}
