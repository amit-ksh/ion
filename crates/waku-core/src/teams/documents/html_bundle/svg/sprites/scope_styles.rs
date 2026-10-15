//! Preserve native @scope roots/limits when a reference SVG is inlined. The
//! browser still owns scope proximity, nesting, and implicit stylesheet owners.
use super::*;

pub(super) fn prelude(source: &str, names: &Names, depth: usize) -> Result<String> {
    ensure!(depth <= 32, "SVG scope nesting exceeds its bound");
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::ParenthesisBlock => {
                let inner = nested_text(&mut parser)?;
                push(&mut out, &format!("({})", selector(&inner, names, depth)?))?;
            }
            // Only top-level parentheses contain scope selectors. Unknown
            // future/invalid prelude tokens must not become selector syntax.
            CssToken::Function(name) => {
                push(
                    &mut out,
                    &format!(
                        "{}({})",
                        CssToken::Ident(name).to_css_string(),
                        nested_text(&mut parser)?
                    ),
                )?;
            }
            CssToken::SquareBracketBlock => {
                push(&mut out, &format!("[{}]", nested_text(&mut parser)?))?;
            }
            _ => push(&mut out, &token.to_css_string())?,
        }
    }
    Ok(out)
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
    fn scope_roots_limits_and_nested_selectors_use_the_reference_document() {
        let n = names();
        let css = namespace_styles(r##"@scope (#panel.card, :root) to (:scope > .stop, [class~="stop"]) { @supports (display:grid) { .shape {fill:red} } @scope (:scope > .inner) to (& > [id="end"]) { & > .shape {stroke:blue} } }"##, &n, 0).unwrap();
        assert!(css.contains("@scope (#file_panel.file_card:where([data-file__element]), [data-file__root]:where([data-file__element])) to (:scope:where([data-file__element]) > .file_stop:where([data-file__element]), [data-file__original_class~=\"stop\"]:where([data-file__element]))"));
        assert!(css.contains(
            "@supports (display:grid) { .file_shape:where([data-file__element]) {fill:red}"
        ));
        assert!(css.contains("@scope (:scope:where([data-file__element]) > .file_inner:where([data-file__element])) to (&:where([data-file__element]) > [data-file__original_id=\"end\"]:where([data-file__element]))"));
        assert!(css.contains(
            "&:where([data-file__element]) > .file_shape:where([data-file__element]) {stroke:blue}"
        ));
        assert!(!css.contains(".file_to"));
    }
    #[test]
    fn scope_implicit_roots_comments_escapes_unknown_tokens_and_bounds_are_preserved() {
        let n = names();
        assert_eq!(
            prelude("@scope /* owner */ ", &n, 0).unwrap(),
            "@scope /* owner */ "
        );
        let css = namespace_styles(
            r"@ScOpE/**/to ( .st\6fp:not(.hidden) ){path{fill:red}}",
            &n,
            0,
        )
        .unwrap();
        assert!(css.starts_with("@ScOpE/**/to ( .file_stop:not(.file_hidden:where([data-file__element])):where([data-file__element]) )"));
        assert_eq!(
            prelude("@scope invalid(.original) [data-name='.original']", &n, 0).unwrap(),
            "@scope invalid(.original) [data-name='.original']"
        );
        assert!(prelude("@scope (.shape)", &n, 33).is_err());
        let deep = format!("@scope ({} .shape {})", ":is(".repeat(34), ")".repeat(34));
        assert!(prelude(&deep, &n, 0).is_err());
        let native = Names::native(HashSet::new(), "owner".into(), true);
        let css = prelude("@scope (#panel) to (.stop)", &native, 0).unwrap();
        assert!(css.contains("#panel:not(:where([owner]))"));
        assert!(css.contains(".stop:not(:where([owner]))"));
    }
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    fn fixture() -> String {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import 'scope.css';@scope (#badge.panel) to (:scope > .stop){.shape{fill:red}@scope (:scope > .inner) to ([data-stop]){&amp; > .shape{stroke:green}}}@scope to (.stop){path{opacity:.75}}</style><symbol id="badge" class="panel" data-label="panel"><path class="shape" data-label="outer"/><g class="inner" data-label="inner"><style>@scope {path{stroke-width:2}}</style><path class="shape" data-label="nested"/><g data-stop="" data-label="limit"><path class="shape" data-label="excluded"/></g></g><g class="stop" data-label="stop"><path class="shape" data-label="stopped"/></g></symbol></svg>"##;
        let mut bundle = Bundle {
            resources: [
                ("a.svg", "image/svg+xml", source.to_owned()),
                (
                    "b.svg",
                    "image/svg+xml",
                    source
                        .replace("fill:red", "fill:blue")
                        .replace("stroke:green", "stroke:purple"),
                ),
                (
                    "scope.css",
                    "text/css",
                    r##"@scope (:root) to ([class~="stop"]){.shape{stroke-width:1}}"##.to_owned(),
                ),
            ]
            .into_iter()
            .map(|(path, mime, text)| (path.into(), (mime, text.into_bytes())))
            .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        };
        let html = bundle.html(r##"<html><head><style>@scope (.owner) to (.stop){.shape{fill:black}}</style></head><body><div class="owner"><span class="shape">Owner</span></div><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
        assert_eq!(bundle.used.len(), 3);
        html
    }
    #[test]
    fn scope_publication_keeps_roots_limits_stylesheet_owners_and_private_imports() {
        let html = fixture();
        let parsed = roxmltree::Document::parse(&html).unwrap();
        let styles = parsed
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .collect::<Vec<_>>();
        assert_eq!(styles.len(), 5);
        assert!(styles.iter().any(|n| {
            n.text()
                .unwrap_or_default()
                .contains("@scope (.owner:not(:where(")
        }));
        for file in ["a.svg", "b.svg"] {
            let prefix = format!("ion_svg_{}_", hash(file.as_bytes()));
            let root = parsed
                .descendants()
                .find(|n| n.attribute("id") == Some(format!("{prefix}badge").as_str()))
                .unwrap();
            assert_eq!(
                root.attribute("class"),
                Some(format!("{prefix}panel").as_str())
            );
            let outer_style = styles
                .iter()
                .find(|n| {
                    n.text()
                        .unwrap_or_default()
                        .contains(&format!("@scope (#{prefix}badge.{prefix}panel"))
                })
                .unwrap();
            let parent = outer_style.parent_element().unwrap();
            assert_eq!(parent.tag_name().name(), "svg");
            assert!(parent.attribute("id").unwrap().starts_with(&prefix));
            let local = styles
                .iter()
                .find(|n| {
                    n.parent_element().and_then(|p| p.attribute("class"))
                        == Some(format!("{prefix}inner").as_str())
                })
                .unwrap();
            assert!(local.text().unwrap().starts_with("@scope {path:where("));
            let css = outer_style.text().unwrap();
            assert!(css.contains(&format!(".{}stop:where(", prefix)));
            assert!(css.contains(":scope:where("));
            assert!(css.contains("&:where("));
            let encoded = css
                .split("data:text/css;base64,")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let imported = String::from_utf8(STANDARD.decode(encoded).unwrap()).unwrap();
            assert!(imported.contains(&format!("@scope ([data-{prefix}")));
            assert!(imported.contains("_original_class~=\"stop\""));
            assert!(imported.contains(&format!(".{}shape:where(", prefix)));
        }
    }
    #[test]
    #[ignore = "Exports production scope CSS to ION_SCOPE_CSS_FIXTURE"]
    fn native_scope_css_phone_fixture() {
        std::fs::write(
            std::env::var_os("ION_SCOPE_CSS_FIXTURE").expect("Set ION_SCOPE_CSS_FIXTURE"),
            fixture(),
        )
        .unwrap();
    }
}
