//! Portable HTML publication from explicitly hash-pinned local resources.
//! No network, subprocess, filesystem mutation or model-sized byte payloads.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use cssparser::{Parser, ParserInput, ToCss, Token as CssToken};
use html5ever::tokenizer::{
    BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    states::RawKind,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::Path,
};

mod svg;

const MAX_OUTPUT: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Resource {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Args {
    pub path: String,
    pub sha256: String,
    pub name: String,
    pub summary: String,
    pub resources: Vec<Resource>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn seal(artifact: &Artifact, args: &Args) -> Result<String> {
    Ok(hash(&serde_json::to_vec(&(artifact, args))?))
}
fn validate(args: &Args) -> Result<()> {
    text(&args.path, 512, "HTML source path")?;
    text(&args.summary, 4096, "Document summary")?;
    text(&args.name, 160, "Document name")?;
    ensure!(
        !args
            .summary
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')),
        "Use a readable summary"
    );
    ensure!(
        args.name.to_ascii_lowercase().ends_with(".html") && args.resources.len() <= 32,
        "Use an HTML filename and at most 32 explicit resources"
    );
    for (path, sha) in std::iter::once((&args.path, &args.sha256))
        .chain(args.resources.iter().map(|r| (&r.path, &r.sha256)))
    {
        ensure!(
            path.len() <= 512
                && !path.is_empty()
                && !path.contains(['\\', '?', '#', ':'])
                && Path::new(path)
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "Use a normalized relative workspace path"
        );
        ensure!(
            sha.len() == 64
                && sha
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Use inspected SHA-256 values"
        );
    }
    Ok(())
}
pub(super) fn verify(
    db: &Connection,
    artifact: &Artifact,
    bytes: &[u8],
) -> Result<(String, String)> {
    let (arguments,signature):(String,String)=db.query_row("SELECT arguments,signature FROM ion_team_html_bundles WHERE artifact_id=? AND length(arguments)<=65536",[artifact.id.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).context("HTML bundle receipt is unavailable")?;
    let args: Args = serde_json::from_str(&arguments)?;
    validate(&args)?;
    ensure!(
        artifact
            .evidence
            .as_ref()
            .and_then(|e| e.producer.activity_id.as_deref())
            == Some("ion-html-bundle")
            && artifact.format == "html"
            && artifact.name == args.name
            && seal(artifact, &args)? == signature
            && artifact.evidence.as_ref().is_some_and(
                |e| e.matches(bytes) && e.producer.origin == ArtifactOrigin::TeamAssignment
            ),
        "HTML bundle evidence changed"
    );
    Ok((args.path, args.summary))
}
pub(in crate::teams) fn recover(
    db: &Connection,
    artifact: &Artifact,
    args: &serde_json::Value,
    bytes: &[u8],
) -> Result<()> {
    verify(db, artifact, bytes)?;
    let saved: String = db.query_row(
        "SELECT arguments FROM ion_team_html_bundles WHERE artifact_id=?",
        [artifact.id.to_string()],
        |r| r.get(0),
    )?;
    ensure!(
        serde_json::from_value::<Args>(args.clone())? == serde_json::from_str::<Args>(&saved)?,
        "Pending HTML bundle differs from its durable receipt"
    );
    Ok(())
}
impl TeamCoordinator {
    pub(crate) fn publish_html_bundle(
        &self,
        lease: &AssignmentLease,
        request: Uuid,
        root: &Path,
        args: &Args,
        now: u64,
    ) -> Result<Artifact> {
        validate(args)?;
        let started = std::time::Instant::now();
        let mut db = self.connect()?;
        {
            let tx = db.transaction()?;
            let run = load(&tx, lease.run_id)?;
            active(&run, now)?;
            let work = checked_lease(&run, lease, now, false)?;
            ensure!(work.access.workspace_read, "Workspace read access denied");
            let old:Option<String>=tx.query_row("SELECT artifact_id FROM ion_team_document_receipts WHERE work_id=? AND request_id=?",params![work.id.to_string(),request.to_string()],|r|r.get(0)).optional()?;
            if let Some(id) = old {
                let body: String = tx.query_row(
                    "SELECT body FROM ion_control_records WHERE id=? AND kind='artifact'",
                    [id],
                    |r| r.get(0),
                )?;
                let ControlRecord::Artifact(artifact) = serde_json::from_str(&body)? else {
                    anyhow::bail!("Publication identity changed")
                };
                ensure!(
                    artifact
                        .evidence
                        .as_ref()
                        .and_then(|e| e.producer.activity_id.as_deref())
                        == Some("ion-html-bundle"),
                    "Publication operation changed"
                );
                let bytes = crate::control::verified_document(&tx, &artifact)?;
                recover(&tx, &artifact, &serde_json::to_value(args)?, &bytes)?;
                return Ok(artifact);
            }
        }
        let source = files::read(root, &args.path)?;
        ensure!(
            source.len() <= 1024 * 1024 && hash(&source) == args.sha256,
            "HTML source size/hash changed"
        );
        let source = std::str::from_utf8(&source).context("HTML source must be UTF-8")?;
        let mut resources = HashMap::new();
        let mut total = 0usize;
        for resource in &args.resources {
            ensure!(
                resource.path != args.path && !resources.contains_key(&resource.path),
                "Duplicate bundle resource path"
            );
            let bytes = files::read(root, &resource.path)?;
            total = total
                .checked_add(bytes.len())
                .context("Resource size overflow")?;
            ensure!(
                bytes.len() <= 2 * 1024 * 1024
                    && total <= 8 * 1024 * 1024
                    && hash(&bytes) == resource.sha256,
                "Resource size/hash changed"
            );
            let mime = mime(&resource.path)?;
            resources.insert(resource.path.clone(), (mime, bytes));
        }
        let mut bundle = Bundle {
            resources,
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        };
        let bytes = bundle.html(source, &args.path)?.into_bytes();
        ensure!(
            bundle.used.len() == args.resources.len(),
            "Bundle contains unused resources; list only referenced local files"
        );
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // File reads and CSS expansion may outlive the call's initial timestamp.
        // Recheck the lease against elapsed monotonic time inside the commit lock.
        let now = now.saturating_add(started.elapsed().as_secs());
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        let work = checked_lease(&run, lease, now, false)?;
        ensure!(work.access.workspace_read, "Workspace read access denied");
        // Reconcile a concurrent publication before modifying any existing artifact.
        let existing:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_document_receipts WHERE work_id=? AND request_id=?)",params![work.id.to_string(),request.to_string()],|r|r.get(0))?;
        let mut artifact =
            publish_bytes(&tx, &run, work, request, &args.name, "html", &bytes, now)?;
        if existing {
            recover(&tx, &artifact, &serde_json::to_value(args)?, &bytes)?;
        } else {
            artifact
                .evidence
                .as_mut()
                .context("Bundle evidence missing")?
                .producer
                .activity_id = Some("ion-html-bundle".into());
            tx.execute(
                "UPDATE ion_control_records SET body=? WHERE id=? AND kind='artifact'",
                params![
                    serde_json::to_string(&ControlRecord::Artifact(artifact.clone()))?,
                    artifact.id.to_string()
                ],
            )?;
            tx.execute(
                "INSERT INTO ion_team_html_bundles VALUES(?,?,?)",
                params![
                    artifact.id.to_string(),
                    serde_json::to_string(args)?,
                    seal(&artifact, args)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(artifact)
    }
}
fn mime(path: &str) -> Result<&'static str> {
    Ok(
        match path
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "css" => "text/css",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "ttf" => "font/ttf",
            "otf" => "font/otf",
            _ => anyhow::bail!(
                "Bundle resources must be local CSS, SVG/PNG/JPEG/GIF/WebP or WOFF/WOFF2/TTF/OTF"
            ),
        },
    )
}
struct Bundle {
    resources: HashMap<String, (&'static str, Vec<u8>)>,
    encoded: HashMap<String, String>,
    visiting: HashSet<String>,
    used: HashSet<String>,
}
fn push(out: &mut String, value: &str) -> Result<()> {
    ensure!(
        out.len().saturating_add(value.len()) <= MAX_OUTPUT,
        "Portable HTML exceeds 16 MiB"
    );
    out.push_str(value);
    Ok(())
}
fn attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
impl Bundle {
    fn url(&mut self, base: &str, value: &str, css: bool) -> Result<String> {
        if value.starts_with('#') || value.starts_with("data:") {
            return Ok(value.to_owned());
        }
        ensure!(
            !value.chars().any(char::is_control) && !value.contains('\\'),
            "Invalid bundle resource URL"
        );
        let origin = url::Url::parse("https://ion-bundle.invalid/")?;
        let resolved = origin.join(base)?.join(value)?;
        ensure!(
            resolved.origin() == origin.origin()
                && resolved.username().is_empty()
                && resolved.password().is_none(),
            "Bundle resources must be explicitly pinned local files"
        );
        let path = percent_encoding::percent_decode_str(resolved.path().trim_start_matches('/'))
            .decode_utf8()?
            .into_owned();
        ensure!(
            !path.contains(['\\', ':', '\0']),
            "Invalid bundle resource path"
        );
        self.used.insert(path.clone());
        let mime = self
            .resources
            .get(&path)
            .context("Referenced local resource is not listed with its inspected hash")?
            .0;
        ensure!(
            css || mime.starts_with("image/"),
            "An image URL must refer to a supported image"
        );
        let fragment = resolved.fragment().filter(|_| mime == "image/svg+xml");
        let suffix = fragment.map(|id| format!("#{id}")).unwrap_or_default();
        if let Some(encoded) = self.encoded.get(&path) {
            return Ok(format!("{encoded}{suffix}"));
        }
        ensure!(
            self.visiting.len() < 16 && self.visiting.insert(path.clone()),
            "Cyclic or deeply nested CSS/SVG resource"
        );
        let bytes = self.resources.get(&path).unwrap().1.clone();
        let bytes = if mime == "text/css" {
            self.css(
                std::str::from_utf8(&bytes).context("Stylesheet must be UTF-8")?,
                &path,
                0,
            )?
            .into_bytes()
        } else if mime == "image/svg+xml" {
            self.svg(
                std::str::from_utf8(&bytes).context("SVG must be UTF-8")?,
                &path,
            )?
            .into_bytes()
        } else {
            bytes
        };
        let value = format!("data:{mime};base64,{}", STANDARD.encode(bytes));
        ensure!(
            value.len() <= MAX_OUTPUT,
            "Encoded resource exceeds its bound"
        );
        ensure!(
            self.encoded
                .values()
                .map(String::len)
                .sum::<usize>()
                .saturating_add(value.len())
                <= MAX_OUTPUT,
            "Expanded resource cache exceeds 16 MiB"
        );
        self.visiting.remove(&path);
        self.encoded.insert(path, value.clone());
        Ok(format!("{value}{suffix}"))
    }
    fn css(&mut self, text: &str, base: &str, depth: usize) -> Result<String> {
        let mut input = ParserInput::new(text);
        let mut parser = Parser::new(&mut input);
        self.css_tokens(&mut parser, base, depth, false)
    }
    fn css_tokens<'i>(
        &mut self,
        parser: &mut Parser<'i, '_>,
        base: &str,
        depth: usize,
        image_set: bool,
    ) -> Result<String> {
        ensure!(depth <= 32, "CSS nesting exceeds its bound");
        let mut out = String::new();
        let mut import = false;
        while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
            match token {
                CssToken::UnquotedUrl(value) => {
                    let value = self.url(base, &value, true)?;
                    push(
                        &mut out,
                        &format!(
                            "url({})",
                            cssparser::Token::QuotedString(value.into()).to_css_string()
                        ),
                    )?;
                    import = false;
                }
                CssToken::QuotedString(value) if import || image_set => {
                    let value = self.url(base, &value, import)?;
                    ensure!(
                        !import || value.starts_with("data:text/css;base64,"),
                        "CSS imports must reference pinned CSS"
                    );
                    push(
                        &mut out,
                        &CssToken::QuotedString(value.into()).to_css_string(),
                    )?;
                    import = false;
                }
                CssToken::Function(ref name) if name.eq_ignore_ascii_case("url") => {
                    let value = parser
                        .parse_nested_block(|nested| {
                            let value = nested.expect_string_cloned()?;
                            nested.expect_exhausted()?;
                            Ok::<_, cssparser::ParseError<'i, ()>>(value.to_string())
                        })
                        .map_err(|_| anyhow::anyhow!("Invalid CSS resource URL"))?;
                    let value = self.url(base, &value, true)?;
                    push(
                        &mut out,
                        &format!(
                            "url({})",
                            CssToken::QuotedString(value.into()).to_css_string()
                        ),
                    )?;
                    import = false;
                }
                CssToken::Function(_)
                | CssToken::ParenthesisBlock
                | CssToken::CurlyBracketBlock
                | CssToken::SquareBracketBlock => {
                    let (open, close) = match &token {
                        CssToken::Function(name) => (
                            format!("{}(", cssparser::Token::Ident(name.clone()).to_css_string()),
                            ")",
                        ),
                        CssToken::ParenthesisBlock => ("(".into(), ")"),
                        CssToken::CurlyBracketBlock => ("{".into(), "}"),
                        _ => ("[".into(), "]"),
                    };
                    let image_strings = matches!(&token, CssToken::Function(name) if name.eq_ignore_ascii_case("image-set") || name.eq_ignore_ascii_case("-webkit-image-set"));
                    let inner = parser
                        .parse_nested_block(|nested| {
                            self.css_tokens(nested, base, depth + 1, image_strings)
                                .map_err(|error| {
                                    nested.new_custom_error::<String, String>(error.to_string())
                                })
                        })
                        .map_err(|_| anyhow::anyhow!("Invalid nested stylesheet"))?;
                    push(&mut out, &open)?;
                    push(&mut out, &inner)?;
                    push(&mut out, close)?;
                    import = false;
                }
                CssToken::BadUrl(_) | CssToken::BadString(_) => {
                    anyhow::bail!("Malformed CSS resource")
                }
                _ => {
                    if let CssToken::AtKeyword(ref value) = token {
                        import = value.eq_ignore_ascii_case("import");
                    } else if !matches!(token, CssToken::WhiteSpace(_) | CssToken::Comment(_)) {
                        import = false;
                    }
                    push(&mut out, &token.to_css_string())?;
                }
            }
        }
        Ok(out)
    }
    fn srcset(&mut self, base: &str, source: &str) -> Result<String> {
        let mut remaining = source;
        let mut output = String::new();
        let mut count = 0;
        loop {
            remaining = remaining.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
            if remaining.is_empty() {
                break;
            }
            count += 1;
            ensure!(count <= 32, "At most 32 responsive image candidates");
            let end = remaining
                .find(|c: char| c.is_ascii_whitespace())
                .unwrap_or(remaining.len());
            let raw = &remaining[..end];
            remaining = &remaining[end..];
            let url = raw.trim_end_matches(',');
            ensure!(!url.is_empty(), "Responsive image URL is empty");
            let descriptor = if raw.ends_with(',') {
                ""
            } else {
                let end = remaining.find(',').unwrap_or(remaining.len());
                let value = remaining[..end].trim();
                remaining = &remaining[end..];
                value
            };
            let mut kinds = HashSet::new();
            for value in descriptor.split_ascii_whitespace() {
                let kind = value.chars().last().context("Empty image descriptor")?;
                ensure!(
                    matches!(kind, 'w' | 'h' | 'x') && kinds.insert(kind),
                    "Invalid or repeated image descriptor"
                );
                let number = &value[..value.len() - 1];
                ensure!(
                    !number.is_empty()
                        && number
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (kind == 'x' && b == b'.')),
                    "Invalid image descriptor number"
                );
                let parsed = number
                    .parse::<f64>()
                    .context("Invalid image descriptor number")?;
                ensure!(
                    parsed.is_finite() && parsed > 0.0,
                    "Use positive image descriptors"
                );
            }
            ensure!(
                !(kinds.contains(&'x') && (kinds.contains(&'w') || kinds.contains(&'h')))
                    && !(kinds.contains(&'h') && !kinds.contains(&'w')),
                "Conflicting image descriptors"
            );
            if !output.is_empty() {
                push(&mut output, ", ")?;
            }
            push(&mut output, &self.url(base, url, false)?)?;
            if !descriptor.is_empty() {
                push(&mut output, " ")?;
                push(&mut output, descriptor)?;
            }
        }
        ensure!(count > 0, "Responsive image candidates are empty");
        Ok(output)
    }
    fn html(&mut self, source: &str, base: &str) -> Result<String> {
        let tokenizer = Tokenizer::new(Tokens::default(), TokenizerOpts::default());
        let input = BufferQueue::default();
        input.push_back(source.into());
        let _ = tokenizer.feed(&input);
        tokenizer.end();
        let tokens = tokenizer.sink.tokens.into_inner();
        ensure!(tokens.len() <= 50_000, "HTML token count exceeds its bound");
        let mut out = String::new();
        let mut raw: Option<String> = None;
        let mut style = String::new();
        for token in tokens {
            match token {
                Token::TagToken(tag) => {
                    let name = tag.name.as_ref();
                    ensure!(
                        name.chars()
                            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ':')),
                        "Malformed HTML tag name"
                    );
                    ensure!(
                        tag.attrs.iter().all(|a| a
                            .name
                            .local
                            .chars()
                            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))),
                        "Malformed HTML attribute name"
                    );
                    if tag.kind == TagKind::EndTag && name == "style" {
                        let css = self.css(&style, base, 0)?.replace('<', "\\3c ");
                        push(&mut out, &css)?;
                        style.clear();
                    }
                    if tag.kind == TagKind::EndTag {
                        push(&mut out, &format!("</{name}>"))?;
                        raw = None;
                        continue;
                    }
                    ensure!(
                        name != "base",
                        "HTML bundle URLs resolve from the source file; remove base elements before publishing"
                    );
                    if name == "link"
                        && tag.attrs.iter().any(|a| {
                            a.name.local.as_ref() == "rel"
                                && a.value
                                    .split_ascii_whitespace()
                                    .any(|r| r.eq_ignore_ascii_case("stylesheet"))
                        })
                    {
                        let href = tag
                            .attrs
                            .iter()
                            .find(|a| a.name.local.as_ref() == "href")
                            .context("Stylesheet link has no href")?;
                        let value = self.url(base, &href.value, true)?;
                        ensure!(
                            value.starts_with("data:text/css;base64,"),
                            "Stylesheet must reference pinned CSS"
                        );
                        push(&mut out, "<style")?;
                        for a in &tag.attrs {
                            if matches!(a.name.local.as_ref(), "media" | "title" | "disabled") {
                                push(
                                    &mut out,
                                    &format!(" {}=\"{}\"", a.name.local, attr(&a.value)),
                                )?;
                            }
                        }
                        let decoded =
                            STANDARD.decode(value.trim_start_matches("data:text/css;base64,"))?;
                        push(&mut out, ">")?;
                        push(
                            &mut out,
                            &std::str::from_utf8(&decoded)?.replace('<', "\\3c "),
                        )?;
                        push(&mut out, "</style>")?;
                        continue;
                    }
                    push(&mut out, &format!("<{name}"))?;
                    for a in &tag.attrs {
                        let key = a.name.local.as_ref();
                        let mut value = a.value.to_string();
                        if key == "style" {
                            value = self.css(&value, base, 0)?;
                        }
                        if (key == "src" && name == "img")
                            || (matches!(key, "href" | "xlink:href")
                                && matches!(name, "image" | "feimage"))
                        {
                            value = self.url(base, &value, false)?;
                        }
                        if key == "srcset" && matches!(name, "img" | "source") {
                            value = self.srcset(base, &value)?;
                        }
                        push(&mut out, &format!(" {}=\"{}\"", key, attr(&value)))?;
                    }
                    push(&mut out, if tag.self_closing { " />" } else { ">" })?;
                    if matches!(
                        name,
                        "style" | "script" | "xmp" | "iframe" | "noembed" | "noframes"
                    ) {
                        raw = Some(name.into());
                    }
                }
                Token::CharacterTokens(value) => {
                    if raw.as_deref() == Some("style") {
                        push(&mut style, &value)?;
                    } else if raw.is_some() {
                        push(&mut out, &value)?;
                    } else {
                        push(
                            &mut out,
                            &value
                                .replace('&', "&amp;")
                                .replace('<', "&lt;")
                                .replace('>', "&gt;"),
                        )?;
                    }
                }
                Token::DoctypeToken(_) => push(&mut out, "<!doctype html>")?,
                Token::NullCharacterToken => push(&mut out, "�")?,
                _ => {}
            }
        }
        if !style.is_empty() {
            push(&mut out, &self.css(&style, base, 0)?.replace('<', "\\3c "))?;
        }
        Ok(out)
    }
}
#[derive(Default)]
struct Tokens {
    tokens: RefCell<Vec<Token>>,
}
impl TokenSink for Tokens {
    type Handle = ();
    fn process_token(&self, token: Token, _line: u64) -> TokenSinkResult<()> {
        let mode = match &token {
            Token::TagToken(tag) if tag.kind == TagKind::StartTag => match tag.name.as_ref() {
                "script" => TokenSinkResult::RawData(RawKind::ScriptData),
                "style" | "xmp" | "iframe" | "noembed" | "noframes" => {
                    TokenSinkResult::RawData(RawKind::Rawtext)
                }
                "textarea" | "title" => TokenSinkResult::RawData(RawKind::Rcdata),
                "plaintext" => TokenSinkResult::Plaintext,
                _ => TokenSinkResult::Continue,
            },
            _ => TokenSinkResult::Continue,
        };
        let mut tokens = self.tokens.borrow_mut();
        if tokens.len() <= 50_000 {
            tokens.push(token);
        }
        mode
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
    #[test]
    fn portable_html_embeds_css_imports_images_fonts_and_media() {
        let mut b = bundle(&[
            (
                "reports/css/main.css",
                "@import './theme.css' screen;@font-face{font-family:Report;src:url('../../font.woff2')} .hero{background:url(../plot.png)}",
            ),
            (
                "reports/css/theme.css",
                "@media(max-width:600px){.hero{display:grid}}",
            ),
            ("reports/plot.png", "raster"),
            ("font.woff2", "font"),
        ]);
        let html=b.html("<!doctype html><link rel=stylesheet media=screen href='css/main.css'><img src='plot.png' alt='Plot'><p>A &amp; B</p>","reports/index.html").unwrap();
        assert!(html.contains("<style media=\"screen\">"));
        assert!(html.contains("data:text/css;base64,"));
        assert!(html.contains("data:image/png;base64,cmFzdGVy"));
        assert!(html.contains("data:font/woff2;base64,Zm9udA=="));
        assert!(html.contains("A &amp; B"));
        assert_eq!(b.used.len(), 4);
        assert!(!html.contains("href=\"css/main.css\""));
    }
    #[test]
    fn css_escaped_urls_and_inline_styles_use_the_same_pinned_resources() {
        let mut b = bundle(&[("images/plot one.png", "plot")]);
        let html=b.html(r#"<style>.a{background:u\72l("images/plot%20one.png")}</style><div style="background:url('images/plot%20one.png')">Text</div>"#,"index.html").unwrap();
        assert_eq!(html.matches("data:image/png;base64,cGxvdA==").count(), 2);
        assert_eq!(b.used.len(), 1);
    }
    #[test]
    fn unlisted_remote_cyclic_and_deep_resources_refuse_without_network() {
        for source in [
            "<img src='missing.png'>",
            "<img src='https://outside.invalid/a.png'>",
            "<img src='//outside.invalid/a.png'>",
            "<link rel=stylesheet href='data:text/css,x'>",
        ] {
            assert!(bundle(&[]).html(source, "index.html").is_err());
        }
        let mut b = bundle(&[("a.css", "@import 'b.css';"), ("b.css", "@import 'a.css';")]);
        assert!(
            b.html("<link rel=stylesheet href='a.css'>", "index.html")
                .is_err()
        );
        assert!(bundle(&[]).css(&"a{".repeat(34), "a.css", 0).is_err());
    }
    #[test]
    fn imported_css_cannot_terminate_the_html_style_element() {
        let mut b = bundle(&[(
            "theme.css",
            r#".a::after{content:"</style><script>x()</script>"}"#,
        )]);
        let html = b
            .html("<link rel=stylesheet href=theme.css>", "index.html")
            .unwrap();
        assert!(!html.contains("<script>"));
        assert_eq!(html.matches("</style>").count(), 1);
        assert!(html.contains("\\3c /style>"));
    }
}

#[cfg(test)]
mod responsive_tests {
    use super::*;
    fn fixture() -> Bundle {
        Bundle {
            resources: [
                ("small.png".into(), ("image/png", b"small".to_vec())),
                ("large.png".into(), ("image/png", b"large".to_vec())),
            ]
            .into_iter()
            .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        }
    }
    #[test]
    fn bundle_responsive_images_and_quoted_css_image_sets_remain_portable() {
        let mut bundle = fixture();
        let html=bundle.html(r#"<picture><source media="(max-width:600px)" srcset="small.png 1x, large.png 2x"><img src="small.png" srcset="small.png 400w, large.png 800w"></picture><style>.hero{background:image-set("small.png" 1x,"large.png" type("image/png") 2x)}</style>"#,"index.html").unwrap();
        assert!(html.contains("data:image/png;base64,c21hbGw= 1x"));
        assert!(html.contains("data:image/png;base64,bGFyZ2U= 800w"));
        assert!(html.contains("image-set(\"data:image/png;base64,c21hbGw=\" 1x"));
        assert!(html.contains("type(\"image/png\")"));
        assert_eq!(bundle.used.len(), 2);
    }
    #[test]
    fn responsive_image_descriptors_and_remote_candidates_refuse() {
        for value in [
            "small.png 0w",
            "small.png 1x 2x",
            "small.png 400w 2x",
            "small.png 1h",
            "https://outside.invalid/a 1x",
            "small.png 1x, https://outside.invalid/b 2x",
        ] {
            assert!(fixture().srcset("index.html", value).is_err(), "{value}");
        }
        assert!(
            fixture()
                .css(
                    ".a{background:image-set(\"https://outside.invalid/a.png\" 1x)}",
                    "a.css",
                    0
                )
                .is_err()
        );
    }
}

#[cfg(test)]
mod svg_resource_tests {
    use super::*;
    fn fixture() -> Bundle {
        Bundle {
            resources: [
                ("plot.png".into(), ("image/png", b"plot".to_vec())),
                ("theme.css".into(), ("text/css", b"p{color:green}".to_vec())),
                ("face.woff2".into(), ("font/woff2", b"font".to_vec())),
            ]
            .into_iter()
            .collect(),
            encoded: HashMap::new(),
            visiting: HashSet::new(),
            used: HashSet::new(),
        }
    }
    #[test]
    fn svg_image_and_filter_hrefs_embed_pinned_rasters_and_keep_fragments() {
        let mut bundle = fixture();
        let html = bundle.html(r##"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image href="plot.png"/><image xlink:href="plot.png"/><filter id="f"><feImage href="plot.png"/><feImage xlink:href="plot.png"/><feImage href="#local"/></filter><use href="#shape"/></svg>"##, "index.html").unwrap();
        assert_eq!(html.matches("data:image/png;base64,cGxvdA==").count(), 4);
        assert!(html.contains(r#"xlink:href="data:image/png;base64,cGxvdA==""#));
        assert!(html.contains(r##"href="#local""##));
        assert!(html.contains(r##"href="#shape""##));
        assert_eq!(bundle.used.len(), 1);
    }
    #[test]
    fn svg_rasters_refuse_remote_unlisted_and_cached_nonimage_resources() {
        for path in [
            "missing.png",
            "https://outside.invalid/plot.png",
            "//outside.invalid/plot.png",
            "theme.css",
            "face.woff2",
        ] {
            let mut bundle = fixture();
            if matches!(path, "theme.css" | "face.woff2") {
                bundle.url("index.html", path, true).unwrap();
            }
            for tag in ["image", "feImage"] {
                assert!(
                    bundle
                        .html(
                            &format!("<svg><{tag} href=\"{path}\"/></svg>"),
                            "index.html"
                        )
                        .is_err(),
                    "{tag} {path}"
                );
            }
        }
    }
}
