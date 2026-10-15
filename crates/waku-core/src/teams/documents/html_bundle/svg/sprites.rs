//! Inline SVG reference documents with private IDs/classes and zero-specificity
//! per-compound stylesheet markers. Markers survive native <use> cloning.
use super::*;

#[derive(Clone)]
pub(in crate::teams::documents::html_bundle) struct Names {
    pub prefix: String,
    ids: HashSet<String>,
    pub(super) root: String,
    fonts: HashSet<String>,
    marker_key: String,
    pub(super) common_marker: String,
    isolate: bool,
}
impl Names {
    pub fn native(ids: HashSet<String>, common_marker: String, isolate: bool) -> Self {
        Self {
            prefix: String::new(),
            ids,
            root: String::new(),
            fonts: HashSet::new(),
            marker_key: String::new(),
            common_marker,
            isolate,
        }
    }
    pub fn id(&self, value: &str) -> String {
        format!("{}{value}", self.prefix)
    }
    pub fn marker(&self) -> String {
        format!("data-{}{}_element", self.prefix, self.marker_key)
    }
    pub fn original(&self, attribute: &str) -> String {
        format!(
            "data-{}{}_original_{attribute}",
            self.prefix, self.marker_key
        )
    }
    pub fn classes(&self, source: &str) -> String {
        let mut out = String::new();
        for part in source.split_inclusive(|c: char| c.is_ascii_whitespace()) {
            let word = part.trim_end_matches(|c: char| c.is_ascii_whitespace());
            if !word.is_empty() {
                out.push_str(&self.prefix);
                out.push_str(word);
            }
            out.push_str(&part[word.len()..]);
        }
        out
    }
    pub fn root_marker(&self) -> String {
        format!("data-{}{}_root", self.prefix, self.marker_key)
    }
}
pub(super) fn document_ids(root: roxmltree::Node<'_, '_>) -> Result<HashSet<String>> {
    let mut ids = HashSet::new();
    for node in root.descendants().filter(|node| node.is_element()) {
        if node
            .ancestors()
            .any(|parent| parent.has_tag_name("metadata"))
        {
            continue;
        }
        if let Some(id) = node.attribute("id") {
            ensure!(
                !id.is_empty() && !id.chars().any(char::is_whitespace) && ids.insert(id.to_owned()),
                "Duplicate or invalid SVG ID"
            );
        }
    }
    Ok(ids)
}
pub(in crate::teams::documents::html_bundle) struct Sprites {
    reserved: HashSet<String>,
    imports: HashMap<String, Names>,
    loading: HashSet<String>,
    css_loading: HashSet<String>,
    definitions: Vec<String>,
    expanded: usize,
    common_marker: String,
}
impl Sprites {
    pub fn new(reserved: HashSet<String>, common_marker: String) -> Self {
        Self {
            reserved,
            imports: HashMap::new(),
            loading: HashSet::new(),
            css_loading: HashSet::new(),
            definitions: Vec::new(),
            expanded: 0,
            common_marker,
        }
    }
    pub fn write_defs(&mut self, out: &mut String) -> Result<()> {
        if self.definitions.is_empty() {
            return Ok(());
        }
        push(out, "<defs>")?;
        for value in self.definitions.drain(..) {
            push(out, &value)?;
        }
        push(out, "</defs>")
    }
    pub fn write_html_defs(&mut self, out: &mut String, svg_depth: usize) -> Result<()> {
        if self.definitions.is_empty() {
            return Ok(());
        }
        if svg_depth == 0 {
            push(
                out,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"0\" height=\"0\" aria-hidden=\"true\" focusable=\"false\" style=\"position:absolute;width:0;height:0;overflow:hidden\">",
            )?;
        }
        self.write_defs(out)?;
        if svg_depth == 0 {
            push(out, "</svg>")?;
        }
        Ok(())
    }
    pub fn reference(
        &mut self,
        bundle: &mut Bundle,
        base: &str,
        value: &str,
        names: &Names,
    ) -> Result<String> {
        if let Some(id) = value.strip_prefix('#') {
            return Ok(format!("#{}", names.id(id)));
        }
        let resolved = resource_url(base, value)?;
        ensure!(
            resolved.query().is_none(),
            "SVG reference queries are unsupported"
        );
        let path = resource_path(&resolved)?;
        let fragment = resolved
            .fragment()
            .map(|id| {
                percent_encoding::percent_decode_str(id)
                    .decode_utf8()
                    .map(|id| id.into_owned())
            })
            .transpose()?;
        if path == base && fragment.as_ref().is_some_and(|id| names.ids.contains(id)) {
            return Ok(format!("#{}", names.id(fragment.as_deref().unwrap())));
        }
        ensure!(
            !bundle.visiting.contains(&path) && !self.loading.contains(&path),
            "Cyclic SVG reference document"
        );
        if !self.imports.contains_key(&path) {
            ensure!(
                self.imports.len() < 32 && self.loading.len() < 16,
                "Too many or deeply nested SVG reference documents"
            );
            let (mime, bytes) = bundle
                .resources
                .get(&path)
                .context("Referenced SVG is not explicitly hash-pinned")?;
            ensure!(
                *mime == "image/svg+xml" && bytes.len() <= 2 * 1024 * 1024,
                "SVG references require a pinned SVG document up to 2 MiB"
            );
            let source = std::str::from_utf8(bytes)?.to_owned();
            let document = roxmltree::Document::parse_with_options(
                &source,
                roxmltree::ParsingOptions {
                    nodes_limit: 50_000,
                    ..Default::default()
                },
            )?;
            ensure!(
                !document.root().children().any(|node| node.is_pi()),
                "SVG processing instructions are unsupported"
            );
            let root = document.root_element();
            ensure!(
                root.tag_name().name() == "svg"
                    && matches!(root.tag_name().namespace(), None | Some(NS)),
                "Use an SVG reference document root"
            );
            let ids = document_ids(root)?;
            let base_prefix = format!("ion_svg_{}_", hash(path.as_bytes()));
            let mut prefix = base_prefix.clone();
            let mut serial = 0;
            while self.reserved.iter().any(|id| id.starts_with(&prefix)) {
                serial += 1;
                ensure!(serial <= 50_000, "SVG namespace allocation failed");
                prefix = format!("{base_prefix}{serial}_");
            }
            let root_id = if let Some(id) = root.attribute("id") {
                format!("{prefix}{id}")
            } else {
                let mut id = "__document_root".to_owned();
                while ids.contains(&id) {
                    id.push('_');
                }
                format!("{prefix}{id}")
            };
            let mut fonts = HashSet::new();
            let mut seen = HashSet::new();
            for style in root.descendants().filter(|node| node.has_tag_name("style")) {
                let text = style
                    .children()
                    .filter_map(|node| node.text())
                    .collect::<String>();
                gather_fonts(bundle, &text, &path, &mut fonts, &mut seen, 0)?;
            }
            let imported = Names {
                prefix,
                ids,
                root: root_id,
                fonts,
                marker_key: hash(source.as_bytes()),
                common_marker: self.common_marker.clone(),
                isolate: true,
            };
            self.reserved
                .extend(imported.ids.iter().map(|id| imported.id(id)));
            self.reserved.insert(imported.root.clone());
            self.loading.insert(path.clone());
            let mut output = String::new();
            bundle.svg_node(
                root,
                &path,
                root.tag_name().namespace(),
                0,
                &imported,
                self,
                &mut output,
            )?;
            self.loading.remove(&path);
            bundle.used.insert(path.clone());
            self.expanded = self
                .expanded
                .checked_add(output.len())
                .context("SVG expansion overflow")?;
            ensure!(
                self.expanded <= MAX_OUTPUT,
                "Expanded SVG definitions exceed 16 MiB"
            );
            self.definitions.push(output);
            self.imports.insert(path.clone(), imported);
        }
        let imported = &self.imports[&path];
        match fragment {
            Some(id) => {
                ensure!(
                    imported.ids.contains(&id),
                    "Referenced SVG ID is unavailable"
                );
                Ok(format!("#{}", imported.id(&id)))
            }
            None => Ok(format!("#{}", imported.root)),
        }
    }
    pub fn stylesheet(
        &mut self,
        bundle: &mut Bundle,
        base: &str,
        value: &str,
        names: &Names,
    ) -> Result<String> {
        let source = format!(
            "@import {};",
            CssToken::QuotedString(value.into()).to_css_string()
        );
        let output = self.css(bundle, &source, base, names, true, false)?;
        let mut input = ParserInput::new(&output);
        let mut parser = Parser::new(&mut input);
        ensure!(
            matches!(parser.next(), Ok(CssToken::AtKeyword(name)) if name.eq_ignore_ascii_case("import")),
            "Invalid bundled stylesheet"
        );
        Ok(parser
            .expect_string_cloned()
            .map_err(|_| anyhow::anyhow!("Invalid bundled stylesheet URL"))?
            .to_string())
    }
    pub fn css(
        &mut self,
        bundle: &mut Bundle,
        source: &str,
        base: &str,
        names: &Names,
        stylesheet: bool,
        reference: bool,
    ) -> Result<String> {
        let resolved = bundle.css_using(source, 0, reference, &mut |bundle, value, kind| {
            if value.starts_with('#') {
                return Ok(format!("#{}", names.id(&value[1..])));
            }
            if kind == CssUrlKind::SvgReference && !value.starts_with("data:") {
                let url = resource_url(base, value)?;
                if url.fragment().is_some() && mime(&resource_path(&url)?)? == "image/svg+xml" {
                    return self.reference(bundle, base, value, names);
                }
            }
            if kind == CssUrlKind::Import {
                let (path, bytes) = if let Some(value) = value.strip_prefix("data:text/css;base64,")
                {
                    (base.to_owned(), STANDARD.decode(value)?)
                } else {
                    let path = resource_path(&resource_url(base, value)?)?;
                    let (mime, bytes) = bundle
                        .resources
                        .get(&path)
                        .context("Imported SVG stylesheet is not hash-pinned")?;
                    ensure!(
                        *mime == "text/css",
                        "SVG stylesheets must reference pinned CSS"
                    );
                    bundle.used.insert(path.clone());
                    (path, bytes.clone())
                };
                let key = format!("{}:{path}:{}", names.prefix, hash(&bytes));
                ensure!(
                    self.css_loading.len() < 16 && self.css_loading.insert(key.clone()),
                    "Cyclic or deep SVG stylesheet import"
                );
                let css = self.css(
                    bundle,
                    std::str::from_utf8(&bytes)?,
                    &path,
                    names,
                    true,
                    false,
                )?;
                self.css_loading.remove(&key);
                self.expanded = self
                    .expanded
                    .checked_add(css.len())
                    .context("SVG stylesheet expansion overflow")?;
                ensure!(
                    self.expanded <= MAX_OUTPUT,
                    "Expanded SVG resources exceed 16 MiB"
                );
                return Ok(format!("data:text/css;base64,{}", STANDARD.encode(css)));
            }
            bundle.url(base, value, kind != CssUrlKind::Image)
        })?;
        if stylesheet && names.isolate {
            namespace_styles(&resolved, names, 0)
        } else {
            font_declarations(&resolved, names)
        }
    }
}

fn nested_text<'i>(parser: &mut Parser<'i, '_>) -> Result<String> {
    parser
        .parse_nested_block(|inner| {
            let start = inner.position();
            while inner.next_including_whitespace_and_comments().is_ok() {}
            Ok::<_, cssparser::ParseError<'i, ()>>(inner.slice_from(start).to_owned())
        })
        .map_err(|_| anyhow::anyhow!("Invalid nested SVG stylesheet"))
}
fn namespace_styles(source: &str, names: &Names, depth: usize) -> Result<String> {
    ensure!(depth <= 32, "SVG stylesheet nesting exceeds its bound");
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let mut prelude = String::new();
    let mut at_rule: Option<String> = None;
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::CurlyBracketBlock => {
                let body = nested_text(&mut parser)?;
                if let Some(name) = at_rule.take() {
                    push(&mut out, &prelude)?;
                    push(&mut out, "{")?;
                    let body = if matches!(
                        name.as_str(),
                        "media" | "supports" | "container" | "layer" | "scope" | "starting-style"
                    ) {
                        namespace_styles(&body, names, depth + 1)?
                    } else {
                        font_declarations(&body, names)?
                    };
                    push(&mut out, &body)?;
                } else {
                    push(&mut out, &selector(&prelude, names, 0)?)?;
                    push(&mut out, "{")?;
                    push(&mut out, &namespace_styles(&body, names, depth + 1)?)?;
                }
                push(&mut out, "}")?;
                prelude.clear();
            }
            CssToken::Semicolon => {
                push(&mut out, &prelude)?;
                push(&mut out, ";")?;
                prelude.clear();
                at_rule = None;
            }
            CssToken::AtKeyword(name) => {
                at_rule = Some(name.to_ascii_lowercase());
                push(&mut prelude, &CssToken::AtKeyword(name).to_css_string())?;
            }
            CssToken::Function(name) => {
                push(
                    &mut prelude,
                    &format!(
                        "{}({})",
                        CssToken::Ident(name).to_css_string(),
                        nested_text(&mut parser)?
                    ),
                )?;
            }
            CssToken::ParenthesisBlock => {
                push(&mut prelude, &format!("({})", nested_text(&mut parser)?))?;
            }
            CssToken::SquareBracketBlock => {
                push(&mut prelude, &format!("[{}]", nested_text(&mut parser)?))?;
            }
            _ => push(&mut prelude, &token.to_css_string())?,
        }
    }
    push(&mut out, &prelude)?;
    font_declarations(&out, names)
}
fn selector(source: &str, names: &Names, depth: usize) -> Result<String> {
    ensure!(depth <= 32, "SVG selector nesting exceeds its bound");
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let marker = if names.prefix.is_empty() {
        format!(":not(:where([{}]))", names.common_marker)
    } else {
        format!(
            ":where([{}])",
            CssToken::Ident(names.marker().into()).to_css_string()
        )
    };
    let mut compound = false;
    let mut class = false;
    let mut colon = false;
    let mut pseudo_element = false;
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        if colon && !matches!(token, CssToken::Ident(_)) {
            if matches!(token, CssToken::Colon) {
                if compound {
                    push(&mut out, &marker)?;
                }
                compound = false;
                pseudo_element = true;
            }
            push(&mut out, ":")?;
            colon = false;
        }
        match token {
            CssToken::WhiteSpace(_) | CssToken::Comma | CssToken::Delim('>' | '+' | '~') => {
                if compound {
                    push(&mut out, &marker)?;
                    compound = false;
                }
                push(&mut out, &token.to_css_string())?;
                pseudo_element = false;
            }
            CssToken::Colon => {
                if colon {
                    push(&mut out, ":")?;
                }
                colon = true;
                compound = !pseudo_element;
            }
            CssToken::Ident(value) => {
                if colon
                    && !pseudo_element
                    && matches!(
                        value.to_ascii_lowercase().as_str(),
                        "before" | "after" | "first-line" | "first-letter"
                    )
                {
                    if compound {
                        push(&mut out, &marker)?;
                    }
                    pseudo_element = true;
                }
                let value =
                    if colon && value.eq_ignore_ascii_case("root") && !names.prefix.is_empty() {
                        format!(
                            "[{}]",
                            CssToken::Ident(names.root_marker().into()).to_css_string()
                        )
                    } else if class {
                        CssToken::Ident(format!("{}{value}", names.prefix).into()).to_css_string()
                    } else {
                        format!(
                            "{}{}",
                            if colon { ":" } else { "" },
                            CssToken::Ident(value).to_css_string()
                        )
                    };
                push(&mut out, &value)?;
                class = false;
                colon = false;
                compound = !pseudo_element;
            }
            CssToken::IDHash(value) => {
                push(
                    &mut out,
                    &CssToken::IDHash(names.id(&value).into()).to_css_string(),
                )?;
                compound = !pseudo_element;
            }
            CssToken::Delim('.') => {
                push(&mut out, ".")?;
                class = true;
                compound = !pseudo_element;
            }
            CssToken::Function(name) => {
                let inner = nested_text(&mut parser)?;
                let inner = if matches!(
                    name.to_ascii_lowercase().as_str(),
                    "is" | "where" | "not" | "has"
                ) {
                    selector(&inner, names, depth + 1)?
                } else if matches!(
                    name.to_ascii_lowercase().as_str(),
                    "nth-child" | "nth-last-child"
                ) {
                    nth_selector(&inner, names, depth + 1)?
                } else {
                    inner
                };
                push(
                    &mut out,
                    &format!("{}({inner})", CssToken::Ident(name).to_css_string()),
                )?;
                compound = !pseudo_element;
            }
            CssToken::SquareBracketBlock => {
                let inner = nested_text(&mut parser)?;
                push(
                    &mut out,
                    &format!("[{}]", attribute_selector(&inner, names)?),
                )?;
                compound = !pseudo_element;
            }
            _ => {
                push(&mut out, &token.to_css_string())?;
                if !matches!(token, CssToken::Comment(_)) {
                    compound = !pseudo_element;
                }
            }
        }
    }
    if colon {
        push(&mut out, ":")?;
    }
    if compound {
        push(&mut out, &marker)?;
    }
    Ok(out)
}
fn attribute_selector(source: &str, names: &Names) -> Result<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let mut attribute = None;
    let mut operator = String::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::Ident(name) if attribute.is_none() => {
                let key = name.to_ascii_lowercase();
                let emitted = if !names.prefix.is_empty() && matches!(key.as_str(), "id" | "class")
                {
                    names.original(&key)
                } else {
                    name.to_string()
                };
                attribute = Some(key);
                push(&mut out, &CssToken::Ident(emitted.into()).to_css_string())?;
            }
            CssToken::Delim('=')
            | CssToken::IncludeMatch
            | CssToken::PrefixMatch
            | CssToken::SuffixMatch
            | CssToken::SubstringMatch
            | CssToken::DashMatch => {
                operator = token.to_css_string();
                push(&mut out, &operator)?;
            }
            CssToken::QuotedString(value) | CssToken::Ident(value) if !operator.is_empty() => {
                let value = match attribute.as_deref() {
                    Some("id" | "class") => value.to_string(),
                    Some("href") if operator == "=" && value.starts_with('#') => {
                        format!("#{}", names.id(&value[1..]))
                    }
                    _ => value.to_string(),
                };
                push(
                    &mut out,
                    &CssToken::QuotedString(value.into()).to_css_string(),
                )?;
                operator.clear();
            }
            _ => push(&mut out, &token.to_css_string())?,
        }
    }
    Ok(out)
}

fn nth_selector(source: &str, names: &Names, depth: usize) -> Result<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let start = parser.position();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        if matches!(token, CssToken::Ident(ref name) if name.eq_ignore_ascii_case("of")) {
            let head = parser.slice_from(start).to_owned();
            let tail = parser.position();
            while parser.next_including_whitespace_and_comments().is_ok() {}
            return Ok(format!(
                "{head}{}",
                selector(parser.slice_from(tail), names, depth)?
            ));
        }
    }
    Ok(source.to_owned())
}
fn family_name(source: &str) -> Option<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut words = Vec::new();
    while let Ok(token) = parser.next().cloned() {
        match token {
            CssToken::Ident(word) | CssToken::QuotedString(word) => words.push(word.to_string()),
            _ => return None,
        }
    }
    (!words.is_empty()).then(|| words.join(" ").to_ascii_lowercase())
}
impl Names {
    pub fn family(&self, source: &str) -> Result<String> {
        if self.prefix.is_empty() {
            return Ok(source.to_owned());
        }
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        let mut out = String::new();
        let mut part = String::new();
        let append = |out: &mut String, part: &str| -> Result<()> {
            if let Some(name) = family_name(part).filter(|name| self.fonts.contains(name)) {
                push(
                    out,
                    &CssToken::QuotedString(format!("{}font_{name}", self.prefix).into())
                        .to_css_string(),
                )
            } else {
                push(out, part)
            }
        };
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            match token {
                CssToken::Comma => {
                    append(&mut out, &part)?;
                    push(&mut out, ",")?;
                    part.clear();
                }
                CssToken::Function(name) => {
                    push(
                        &mut part,
                        &format!(
                            "{}({})",
                            CssToken::Ident(name).to_css_string(),
                            nested_text(&mut parser)?
                        ),
                    )?;
                }
                _ => push(&mut part, &token.to_css_string())?,
            }
        }
        append(&mut out, &part)?;
        Ok(out)
    }
}
fn font_declarations(source: &str, names: &Names) -> Result<String> {
    if names.fonts.is_empty() {
        return Ok(source.to_owned());
    }
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut out = String::new();
    let mut property = String::new();
    let mut value = String::new();
    let mut colon = false;
    let append = |out: &mut String, property: &str, value: &str, colon: bool| -> Result<()> {
        push(out, property)?;
        if colon {
            push(out, ":")?;
            let css = if property.trim().eq_ignore_ascii_case("font-family") {
                // Keep !important outside the family token list.
                if let Some(body) = value.strip_suffix("!important") {
                    format!("{}!important", names.family(body)?)
                } else {
                    names.family(value)?
                }
            } else if property.trim().eq_ignore_ascii_case("font") {
                font_shorthand(value, names)?
            } else {
                value.to_owned()
            };
            push(out, &css)?;
        }
        Ok(())
    };
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        if matches!(token, CssToken::Colon) && !colon {
            colon = true;
            continue;
        }
        if matches!(token, CssToken::Semicolon) {
            append(&mut out, &property, &value, colon)?;
            push(&mut out, ";")?;
            property.clear();
            value.clear();
            colon = false;
            continue;
        }
        let target = if colon { &mut value } else { &mut property };
        match token {
            CssToken::Function(name) => push(
                target,
                &format!(
                    "{}({})",
                    CssToken::Ident(name).to_css_string(),
                    nested_text(&mut parser)?
                ),
            )?,
            CssToken::CurlyBracketBlock => {
                // This is a rule list. Its declarations were already handled recursively.
                push(target, &format!("{{{}}}", nested_text(&mut parser)?))?;
            }
            CssToken::SquareBracketBlock => {
                push(target, &format!("[{}]", nested_text(&mut parser)?))?
            }
            CssToken::ParenthesisBlock => {
                push(target, &format!("({})", nested_text(&mut parser)?))?
            }
            _ => push(target, &token.to_css_string())?,
        }
    }
    append(&mut out, &property, &value, colon)?;
    Ok(out)
}
fn gather_fonts(
    bundle: &Bundle,
    source: &str,
    base: &str,
    fonts: &mut HashSet<String>,
    seen: &mut HashSet<String>,
    depth: usize,
) -> Result<()> {
    ensure!(
        depth <= 16 && source.len() <= MAX_OUTPUT,
        "SVG font stylesheet depth/size exceeds its bound"
    );
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut kind = String::new();
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            CssToken::AtKeyword(name) => kind = name.to_ascii_lowercase(),
            CssToken::CurlyBracketBlock => {
                let body = nested_text(&mut parser)?;
                if kind == "font-face" {
                    let mut input = ParserInput::new(&body);
                    let mut declarations = Parser::new(&mut input);
                    let mut font = false;
                    while let Ok(token) = declarations.next().cloned() {
                        if matches!(token, CssToken::Ident(ref name) if name.eq_ignore_ascii_case("font-family"))
                        {
                            font = true;
                        }
                        if font && matches!(token, CssToken::Colon) {
                            let start = declarations.position();
                            while declarations
                                .next_including_whitespace_and_comments()
                                .is_ok_and(|token| !matches!(token, CssToken::Semicolon))
                            {
                            }
                            let value = declarations.slice_from(start).trim_end_matches(';');
                            if let Some(name) = family_name(value) {
                                fonts.insert(name);
                            }
                            font = false;
                        }
                    }
                } else {
                    gather_fonts(bundle, &body, base, fonts, seen, depth + 1)?;
                }
                kind.clear();
            }
            CssToken::QuotedString(value) | CssToken::UnquotedUrl(value) if kind == "import" => {
                gather_import(bundle, &value, base, fonts, seen, depth + 1)?;
                kind.clear();
            }
            CssToken::Function(name) if kind == "import" && name.eq_ignore_ascii_case("url") => {
                let raw = nested_text(&mut parser)?;
                let mut input = ParserInput::new(&raw);
                let mut url = Parser::new(&mut input);
                let value = url
                    .expect_string_cloned()
                    .map_err(|_| anyhow::anyhow!("Invalid SVG CSS import"))?;
                gather_import(bundle, &value, base, fonts, seen, depth + 1)?;
                kind.clear();
            }
            CssToken::Semicolon => kind.clear(),
            _ => {}
        }
    }
    Ok(())
}
fn gather_import(
    bundle: &Bundle,
    value: &str,
    base: &str,
    fonts: &mut HashSet<String>,
    seen: &mut HashSet<String>,
    depth: usize,
) -> Result<()> {
    let (path, bytes) = if let Some(data) = value.strip_prefix("data:text/css;base64,") {
        (
            format!("data:{}", hash(data.as_bytes())),
            STANDARD.decode(data)?,
        )
    } else {
        let path = resource_path(&resource_url(base, value)?)?;
        let (mime, bytes) = bundle
            .resources
            .get(&path)
            .context("SVG font stylesheet is not hash-pinned")?;
        ensure!(*mime == "text/css", "SVG font stylesheet must be CSS");
        (path, bytes.clone())
    };
    if seen.insert(path.clone()) {
        gather_fonts(
            bundle,
            std::str::from_utf8(&bytes)?,
            if path.starts_with("data:") {
                base
            } else {
                &path
            },
            fonts,
            seen,
            depth,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bundle(items: &[(&str, &str)]) -> Bundle {
        Bundle {
            resources: items
                .iter()
                .map(|(path, source)| {
                    (
                        path.to_string(),
                        (mime(path).unwrap(), source.as_bytes().to_vec()),
                    )
                })
                .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        }
    }
    fn assets() -> Bundle {
        bundle(&[
            (
                "icons/a.svg",
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><style>@import '../styles.css';.shape:not(.hidden),#badge{fill:url(#paint);stroke:#abc;font:italic 12px/1.2 Report,serif}.shape:nth-child(2n of .shape){opacity:.8}.shape::before{content:'x'}.shape:after{content:'y'}</style><defs><linearGradient id="paint"><stop offset="1" stop-color="red"/></linearGradient></defs><symbol id="badge" viewBox="0 0 20 20"><path class="shape __element" d="M0 0L10 10"/><text font-family="Report">徽章 🚀</text><use href="b.svg#badge"/></symbol></svg>"##,
            ),
            (
                "icons/b.svg",
                r##"<svg xmlns="http://www.w3.org/2000/svg"><style>.shape{fill:blue;stroke:#abc}</style><symbol id="badge"><path class="shape" d="M0 0L20 20"/></symbol><path id="route" d="M0 0L1 1"/><mask id="cut"><rect width="1" height="1"/></mask></svg>"##,
            ),
            (
                "styles.css",
                "@font-face{font-family:Report;src:url(face.woff2)}@media(max-width:600px){.shape{font-family:Report,serif}}[id='badge'],[class~='shape'],[class*='ion'],[class='shape __element']{color:green}",
            ),
            ("face.woff2", "font"),
        ])
    }
    fn html_fixture() -> String {
        let mut b = assets();
        let html = b.html(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><use href="icons/a.svg#badge" x="3" width="20"/><use xlink:href="icons/b.svg#badge"/><text><textPath href="icons/b.svg#route">Along route</textPath></text><rect fill="url(icons/a.svg#paint)" mask="url(icons/b.svg#cut)"/></svg>"##, "index.html").unwrap();
        assert_eq!(b.used.len(), 4);
        html
    }
    #[test]
    fn sprite_documents_preserve_unique_ids_styles_geometry_fonts_and_repeated_references() {
        let html = html_fixture();
        let parsed = roxmltree::Document::parse(&html).unwrap();
        let ids = parsed
            .descendants()
            .filter_map(|node| node.attribute("id"))
            .collect::<Vec<_>>();
        assert_eq!(ids.len(), ids.iter().collect::<HashSet<_>>().len());
        let a = format!("ion_svg_{}_", hash(b"icons/a.svg"));
        let b = format!("ion_svg_{}_", hash(b"icons/b.svg"));
        assert!(ids.contains(&format!("{a}badge").as_str()));
        assert!(ids.contains(&format!("{b}badge").as_str()));
        assert_eq!(html.matches(&format!("id=\"{a}badge\"")).count(), 1);
        assert_eq!(html.matches(&format!("id=\"{b}badge\"")).count(), 1);
        assert!(html.contains(&format!("href=\"#{a}badge\" x=\"3\" width=\"20\"")));
        assert!(html.contains(&format!("xlink:href=\"#{b}badge\"")));
        assert!(html.contains(&format!("href=\"#{b}route\"")));
        assert!(html.contains(&format!("font-family=\"&quot;{a}font_report&quot;\"")));
        assert!(html.contains("stroke:#abc"));
        assert!(html.contains(&format!("12px/1.2 &quot;{a}font_report&quot;,serif")));
        assert!(html.contains(&format!(".{a}shape:not(.{a}hidden:where(")));
        assert!(html.contains(&format!("2n of .{a}shape:where(")));
        assert!(html.contains("::before{content:"));
        assert!(!html.contains("::before:where"));
        assert!(!html.contains(":after:where"));
        assert!(html.contains("]):after{content:"));
        assert!(
            parsed
                .descendants()
                .filter_map(|node| node.attribute("class"))
                .all(|classes| classes.starts_with("ion_svg_"))
        );
        assert!(html.contains(&format!(".{a}shape")) && html.contains(&format!(".{b}shape")));
        for node in parsed.descendants().filter(|node| node.is_element()) {
            for attribute in node.attributes().filter(|attr| attr.name() == "href") {
                let target = attribute.value().strip_prefix('#').unwrap();
                assert!(ids.contains(&target), "{target}");
            }
        }
        // Imported styles remain scoped after inlining and preserve font aliases.
        let mut scoped_fonts = false;
        for node in parsed
            .descendants()
            .filter(|node| node.has_tag_name("style"))
        {
            let style = node.text().unwrap_or_default();
            if let Some(data) = style.split("data:text/css;base64,").nth(1) {
                let data = data.split('"').next().unwrap();
                let css = String::from_utf8(STANDARD.decode(data).unwrap()).unwrap();
                assert!(css.contains(&format!("{a}font_report")));
                assert!(css.contains(&format!(".{a}shape:where(")));
                assert!(css.contains("data:font/woff2;base64,"));
                scoped_fonts = true;
            }
        }
        assert!(scoped_fonts);
    }
    #[test]
    fn svg_image_sprites_and_main_document_references_remain_local() {
        let mut b = assets();
        let image = b.svg(r##"<svg xmlns="http://www.w3.org/2000/svg"><path id="own"/><use href="chart.svg#own"/><use href="icons/a.svg#badge"/><rect style="fill:url(icons/a.svg#paint);background:url(icons/b.svg#route)"/></svg>"##, "chart.svg").unwrap();
        let parsed = roxmltree::Document::parse(&image).unwrap();
        assert!(image.contains("href=\"#own\""));
        assert!(image.contains("data:image/svg+xml;base64,")); // background stays an image, not a paint reference
        assert!(
            parsed
                .descendants()
                .any(|node| node.tag_name().name() == "symbol")
        );
        let html = b.html(r##"<svg><path id="own"/><use href="index.html#own"/><style>rect{fill:url(icons/a.svg#paint)}</style></svg>"##, "index.html").unwrap();
        assert!(html.contains("href=\"#own\""));
        assert!(html.contains("<defs>"));
        assert!(!html.contains("url(icons/a.svg"));
        let html = b.html(r##"<style>.report{filter:url(icons/b.svg#cut)}</style><div style="clip-path:url(icons/b.svg#cut)">Report</div>"##, "index.html").unwrap();
        assert!(html.contains("aria-hidden=\"true\""));
        assert!(html.contains("<defs>"));
        assert!(html.contains(":not(:where([data-ion-svg-import-"));
        assert!(!html.contains("icons/b.svg"));
    }
    #[test]
    fn sprite_prefixes_do_not_rebind_owner_ids_and_root_imports_have_unique_targets() {
        let prefix = format!("ion_svg_{}_", hash(b"icon.svg"));
        let mut b = bundle(&[("icon.svg", "<svg><symbol id='badge'/></svg>")]);
        let html = b.html(&format!("<svg><path id='{prefix}unknown'/><use href='icon.svg#badge'/><use href='icon.svg'/></svg>"), "index.html").unwrap();
        assert!(html.contains(&format!("href=\"#{prefix}1_badge\"")));
        assert!(html.contains(&format!("href=\"#{prefix}1___document_root\"")));
        let parsed = roxmltree::Document::parse(&html).unwrap();
        let roots = parsed
            .descendants()
            .filter(|node| {
                node.attribute("id") == Some(format!("{prefix}1___document_root").as_str())
            })
            .count();
        assert_eq!(roots, 1);
    }
    #[test]
    fn sprites_refuse_unpinned_remote_missing_duplicate_and_cyclic_targets() {
        for value in [
            "missing.svg#x",
            "https://outside.invalid/icon.svg#x",
            "//outside.invalid/icon.svg#x",
            "icon.svg#missing",
            "icon.svg?query#x",
            "data:image/svg+xml;base64,AAAA",
            "theme.css#x",
        ] {
            let mut b = bundle(&[
                ("icon.svg", "<svg><symbol id='x'/></svg>"),
                ("theme.css", "p{color:red}"),
            ]);
            assert!(
                b.html(&format!("<svg><use href='{value}'/></svg>"), "index.html")
                    .is_err(),
                "{value}"
            );
        }
        for source in [
            "<svg><g id='x'/><g id='x'/></svg>",
            "<svg><script id='x'/></svg>",
            "<svg><use id='x' href='other.svg#x'/></svg>",
        ] {
            let mut b = bundle(&[
                ("icon.svg", source),
                ("other.svg", "<svg><use id='x' href='icon.svg#x'/></svg>"),
            ]);
            assert!(
                b.html("<svg><use href='icon.svg#x'/></svg>", "index.html")
                    .is_err()
            );
        }
        let mut b = bundle(&[
            (
                "icon.svg",
                "<svg><style>@import 'a.css';</style><g id='x'/></svg>",
            ),
            ("a.css", "@import 'b.css';"),
            ("b.css", "@import 'a.css';"),
        ]);
        assert!(
            b.html("<svg><use href='icon.svg#x'/></svg>", "index.html")
                .is_err()
        );
    }
    #[test]
    #[ignore = "Exports a production native sprite fixture only to ION_SVG_SPRITE_FIXTURE"]
    fn native_svg_sprite_phone_fixture() {
        let path = std::env::var_os("ION_SVG_SPRITE_FIXTURE").expect("Set ION_SVG_SPRITE_FIXTURE");
        std::fs::write(path, html_fixture()).unwrap();
    }
}

fn font_shorthand(source: &str, names: &Names) -> Result<String> {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let start = parser.position();
    while let Ok(token) = parser.next().cloned() {
        let size = matches!(
            token,
            CssToken::Dimension { .. } | CssToken::Percentage { .. }
        ) || matches!(token, CssToken::Ident(ref value) if matches!(value.to_ascii_lowercase().as_str(), "xx-small" | "x-small" | "small" | "medium" | "large" | "x-large" | "xx-large" | "xxx-large" | "smaller" | "larger"));
        if size {
            let mut end = parser.position();
            if matches!(parser.next(), Ok(CssToken::Delim('/'))) {
                let height = parser
                    .next()
                    .cloned()
                    .map_err(|_| anyhow::anyhow!("Missing SVG font line height"))?;
                if matches!(height, CssToken::Function(_)) {
                    nested_text(&mut parser)?;
                }
                end = parser.position();
            }
            let head = parser.slice(start..end).to_owned();
            while parser.next_including_whitespace_and_comments().is_ok() {}
            return Ok(format!("{head} {}", names.family(parser.slice_from(end))?));
        }
    }
    Ok(source.to_owned())
}

#[cfg(test)]
mod namespace_tests {
    use super::*;
    #[test]
    fn stylesheet_namespace_uris_are_identifiers_and_source_rules_exclude_imports() {
        let mut b = Bundle { resources: [("icons.svg".into(), ("image/svg+xml", br##"<svg xmlns="http://www.w3.org/2000/svg"><style>@namespace svg url('http://www.w3.org/2000/svg');svg|path{fill:red}</style><symbol id="badge"><path class="shape" id="plot"/></symbol></svg>"##.to_vec()))].into_iter().collect(), encoded:HashMap::new(), visiting:HashSet::new(), used:HashSet::new() };
        let html = b.html(r##"<style>@namespace url('http://www.w3.org/2000/svg');path{fill:blue}</style><svg><use href="icons.svg#badge"/></svg>"##, "index.html").unwrap();
        assert!(html.contains("@namespace url(\"http://www.w3.org/2000/svg\")"));
        assert!(html.contains("path:not(:where([data-ion-svg-import-"));
        assert!(html.contains("svg|path:where([data-ion_svg_"));
        assert_eq!(b.used.len(), 1);
    }
}
