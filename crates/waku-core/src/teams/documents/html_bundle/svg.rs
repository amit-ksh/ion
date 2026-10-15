//! Resolve dependencies inside a hash-pinned SVG image before data embedding.
//! Preserve XML case/namespaces; do not reinterpret SVG using an HTML tokenizer.
use super::*;
pub(super) mod sprites;
use sprites::{Names, Sprites};

const NS: &str = "http://www.w3.org/2000/svg";
const XLINK: &str = "http://www.w3.org/1999/xlink";
const XML: &str = "http://www.w3.org/XML/1998/namespace";

impl Bundle {
    pub(super) fn svg(&mut self, source: &str, base: &str) -> Result<String> {
        ensure!(source.len() <= 2 * 1024 * 1024, "SVG source exceeds 2 MiB");
        let document = roxmltree::Document::parse_with_options(
            source,
            roxmltree::ParsingOptions {
                nodes_limit: 50_000,
                ..Default::default()
            },
        )
        .context("Invalid SVG XML; DTD entities are unsupported")?;
        ensure!(
            !document.root().children().any(|node| node.is_pi()),
            "SVG processing instructions are unsupported; use inline style elements"
        );
        let root = document.root_element();
        ensure!(
            root.tag_name().name() == "svg"
                && matches!(root.tag_name().namespace(), None | Some(NS)),
            "Use an SVG document root"
        );
        let ids = sprites::document_ids(root)?;
        let common_marker = format!("data-ion-svg-import-{}", hash(source.as_bytes()));
        let mut sprites = Sprites::new(ids.clone(), common_marker.clone());
        let isolate = self
            .resources
            .values()
            .any(|(mime, _)| *mime == "image/svg+xml");
        let names = Names::native(ids, common_marker, isolate);
        let mut out = String::new();
        self.svg_node(
            root,
            base,
            root.tag_name().namespace(),
            0,
            &names,
            &mut sprites,
            &mut out,
        )?;
        Ok(out)
    }
    pub(super) fn svg_node(
        &mut self,
        node: roxmltree::Node<'_, '_>,
        base: &str,
        namespace: Option<&str>,
        depth: usize,
        names: &Names,
        sprites: &mut Sprites,
        out: &mut String,
    ) -> Result<()> {
        ensure!(depth <= 128, "SVG nesting exceeds its bound");
        if node.is_element() {
            let tag = node.tag_name().name();
            if tag == "metadata" {
                return Ok(());
            }
            ensure!(
                node.tag_name().namespace() == namespace,
                "Unsupported foreign SVG element"
            );
            ensure!(
                !matches!(
                    tag.to_ascii_lowercase().as_str(),
                    "script"
                        | "foreignobject"
                        | "animate"
                        | "animatemotion"
                        | "animatetransform"
                        | "set"
                        | "discard"
                        | "handler"
                ),
                "Portable SVG images must be static drawings"
            );
            ensure!(node.attributes().len() <= 64, "SVG has too many attributes");
            push(out, &format!("<{tag}"))?;
            if depth == 0 {
                push(out, &format!(" xmlns=\"{NS}\" xmlns:xlink=\"{XLINK}\""))?;
            }
            let classes = node.attribute("class").unwrap_or_default();
            if !names.prefix.is_empty() {
                if node.attribute("class").is_some() {
                    push(
                        out,
                        &format!(
                            " class=\"{}\" {}=\"{}\"",
                            attr(&names.classes(classes)),
                            names.original("class"),
                            attr(classes)
                        ),
                    )?;
                }
                push(
                    out,
                    &format!(" {}=\"\" {}=\"\"", names.marker(), names.common_marker),
                )?;
                if depth == 0 {
                    push(out, &format!(" {}=\"\"", names.root_marker()))?;
                }
                if let Some(id) = node.attribute("id") {
                    push(out, &format!(" {}=\"{}\"", names.original("id"), attr(id)))?;
                }
            }
            if depth == 0 && !names.prefix.is_empty() && node.attribute("id").is_none() {
                push(out, &format!(" id=\"{}\"", attr(&names.root)))?;
            }
            for attribute in node.attributes() {
                let name = attribute.name();
                let key = match attribute.namespace() {
                    None => name.to_owned(),
                    Some(XLINK) => format!("xlink:{name}"),
                    Some(XML) if matches!(name, "lang" | "space") => format!("xml:{name}"),
                    // Editor metadata does not participate in image rendering.
                    Some(XML) => anyhow::bail!("SVG xml:base or other XML control is unsupported"),
                    Some(_) => continue,
                };
                ensure!(
                    !name.to_ascii_lowercase().starts_with("on"),
                    "SVG event handlers are unsupported"
                );
                if name == "class" && !names.prefix.is_empty() {
                    continue;
                }
                let mut value = attribute.value().to_owned();
                if name == "font-family" {
                    value = names.family(&value)?;
                }
                if name == "id" {
                    value = names.id(&value);
                }
                if matches!(name, "aria-labelledby" | "aria-describedby") {
                    value = value
                        .split_ascii_whitespace()
                        .map(|id| names.id(id))
                        .collect::<Vec<_>>()
                        .join(" ");
                }
                if name == "href" {
                    if tag == "a" {
                        continue;
                    }
                    if matches!(tag, "image" | "feImage") {
                        value = self.url(base, &value, false)?;
                    } else {
                        value = sprites.reference(self, base, &value, names)?;
                    }
                } else if matches!(
                    name,
                    "style"
                        | "fill"
                        | "stroke"
                        | "filter"
                        | "clip-path"
                        | "mask"
                        | "marker"
                        | "marker-start"
                        | "marker-mid"
                        | "marker-end"
                        | "cursor"
                ) {
                    value =
                        sprites.css(self, &value, base, names, false, svg_presentation(name))?;
                }
                push(out, &format!(" {key}=\"{}\"", attr(&value)))?;
            }
            push(out, ">")?;
            if tag == "style" {
                let text = node
                    .children()
                    .filter_map(|child| child.text())
                    .collect::<String>();
                push(
                    out,
                    &attr(&sprites.css(self, &text, base, names, true, false)?),
                )?;
            } else {
                for child in node.children() {
                    self.svg_node(child, base, namespace, depth + 1, names, sprites, out)?;
                }
            }
            if depth == 0 && names.prefix.is_empty() {
                sprites.write_defs(out)?;
            }
            push(out, &format!("</{tag}>"))?;
        } else if node.is_text() {
            push(out, &attr(node.text().unwrap_or_default()))?;
        } else if node.is_pi() {
            anyhow::bail!("SVG processing instructions are unsupported; use inline style elements");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bundle(items: &[(&str, &str)]) -> Bundle {
        Bundle {
            resources: items
                .iter()
                .map(|(p, v)| (p.to_string(), (mime(p).unwrap(), v.as_bytes().to_vec())))
                .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        }
    }
    fn decoded(value: &str) -> String {
        let data = value
            .strip_prefix("data:image/svg+xml;base64,")
            .unwrap()
            .split('#')
            .next()
            .unwrap();
        String::from_utf8(STANDARD.decode(data).unwrap()).unwrap()
    }
    #[test]
    fn svg_image_dependencies_keep_case_fragments_styles_and_nested_assets() {
        let mut b = bundle(&[
            (
                "drawings/chart.svg",
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><style>@import '../css/theme.css';</style><defs><linearGradient id="shade"><stop offset="1"/></linearGradient></defs><path fill="url(#shade)" d="M0 0L1 1"/><image href="../plot.png"/><image href="badge.svg"/><text xml:lang="en">A &amp; B</text></svg>"##,
            ),
            ("drawings/badge.svg", "<svg><text>徽章 🚀</text></svg>"),
            (
                "css/theme.css",
                "@font-face{font-family:Plot;src:url('../face.woff2')}.plot{color:green}",
            ),
            ("plot.png", "plot"),
            ("face.woff2", "font"),
        ]);
        let first = b
            .url("index.html", "drawings/chart.svg#main", false)
            .unwrap();
        assert!(first.ends_with("#main"));
        let svg = decoded(&first);
        let parsed = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            parsed.root_element().attribute("viewBox"),
            Some("0 0 10 10")
        );
        assert!(svg.contains("<linearGradient"));
        assert!(svg.contains("data:image/png;base64,cGxvdA=="));
        assert!(svg.contains("data:image/svg+xml;base64,"));
        assert!(svg.contains("data:text/css;base64,"));
        assert!(svg.contains("A &amp; B"));
        assert_eq!(
            parsed
                .descendants()
                .find(|node| node.has_tag_name("path"))
                .unwrap()
                .attribute("fill"),
            Some("url(\"#shade\")")
        );
        let second = b
            .url("index.html", "drawings/chart.svg#other", false)
            .unwrap();
        assert_eq!(decoded(&first), decoded(&second));
        assert!(second.ends_with("#other"));
        assert_eq!(b.used.len(), 5);
        let html = b.html("<img src='drawings/chart.svg'><style>.chart{background:url(drawings/chart.svg)}</style>", "index.html").unwrap();
        assert_eq!(html.matches("data:image/svg+xml;base64,").count(), 2);
    }
    #[test]
    fn svg_dependencies_refuse_remote_unlisted_cycles_and_wrong_types() {
        for value in [
            "missing.png",
            "https://outside.invalid/a.png",
            "//outside.invalid/a.png",
            "theme.css",
        ] {
            let svg = format!("<svg><image href='{value}'/></svg>");
            let mut b = bundle(&[("chart.svg", &svg), ("theme.css", "p{color:green}")]);
            assert!(b.url("index.html", "chart.svg", false).is_err(), "{value}");
        }
        let mut b = bundle(&[
            ("a.svg", "<svg><image href='b.svg'/></svg>"),
            ("b.svg", "<svg><style>.x{fill:url(a.svg)}</style></svg>"),
        ]);
        assert!(b.url("index.html", "a.svg", false).is_err());
    }
    #[test]
    fn svg_xml_active_dtd_and_foreign_controls_refuse() {
        for source in [
            "<html/>",
            "<svg><g></svg>",
            "<svg><script>run()</script></svg>",
            "<svg onload='run()'/>",
            "<svg><foreignObject><div/></foreignObject></svg>",
            "<svg><animate attributeName='href'/></svg>",
            "<svg xml:base='https://outside.invalid'/>",
            "<svg><g xmlns='http://www.w3.org/1999/xhtml'/></svg>",
            "<!DOCTYPE svg [<!ENTITY x 'expanded'>]><svg>&x;</svg>",
            "<svg><?xml-stylesheet href='theme.css'?></svg>",
        ] {
            assert!(bundle(&[]).svg(source, "drawing.svg").is_err(), "{source}");
        }
        assert!(
            bundle(&[])
                .svg(
                    &format!("<svg>{}{}</svg>", "<g>".repeat(129), "</g>".repeat(129)),
                    "drawing.svg"
                )
                .is_err()
        );
    }
}
