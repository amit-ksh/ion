use super::*;
fn names() -> Names {
    let mut names = Names::native(HashSet::new(), "owner".into(), true);
    names.prefix = "private_".into();
    names
}
#[test]
fn animation_computed_keywords_fill_their_own_slots_before_becoming_names() {
    for word in ["linear", "infinite", "alternate", "both", "paused"] {
        let source = format!(r#"animation:1s ident("{word}") ident("{word}")"#);
        let expected = format!(r#"animation:1s ident("{word}") ident("private_animation_{word}")"#);
        assert_eq!(declarations(&source, &names()).unwrap(), expected);
    }
    let css = declarations(r#"animation:1s steps(2,end) ident("linear"),1s 3 ident("infinite");animation:1s ident("pulse")"#, &names()).unwrap();
    assert!(
        css.contains(r#"steps(2,end) ident("private_animation_linear")"#),
        "{css}"
    );
    assert!(
        css.contains(r#"1s 3 ident("private_animation_infinite")"#),
        "{css}"
    );
    assert!(
        css.contains(r#"1s ident("private_animation_pulse")"#),
        "{css}"
    );
}
#[test]
fn animation_none_fills_the_fill_slot_without_becoming_a_keyframe_name() {
    let css = declarations(r#"animation:1s none both,2s ident("none") ident("backwards"),3s none none;animation-name:none,ident("none"),"none""#, &names()).unwrap();
    for value in [
        "animation:1s none private_animation_both",
        r#"2s ident("none") ident("private_animation_backwards")"#,
        "3s none none",
        r#"animation-name:none,ident("none"),private_animation_none"#,
    ] {
        assert!(css.contains(value), "{value} in {css}");
    }
}
#[test]
fn animation_unknown_substitutions_do_not_guess_keyword_roles_or_expand_fallbacks() {
    let css = declarations(r#"animation:1s linear var(--x,linear) ident("linear");animation:1s var(--x) linear linear;animation:1s var(--x) ident("pulse");animation:1s var(--x) "linear";animation:1s var(--x) ident("ready-" var(--suffix))"#, &names()).unwrap();
    for value in [
        r#"animation:1s linear var(--private_property_x,linear) ident("linear")"#,
        "animation:1s var(--private_property_x) linear private_animation_linear",
        r#"animation:1s var(--private_property_x) ident("private_animation_pulse")"#,
        "animation:1s var(--private_property_x) private_animation_linear",
        r#"animation:1s var(--private_property_x) ident("private_animation_ready-" var(--private_property_suffix))"#,
    ] {
        assert!(css.contains(value), "{value} in {css}");
    }
}
#[test]
fn animation_unknown_identifiers_keep_already_filled_slots_and_do_not_invent_free_ones() {
    let css = declarations(r#"animation:1s linear ident(var(--name)) ident("linear");animation:1s ident(var(--name)) ident("linear");animation:1s calc(var(--n)*1) ident("infinite") ident("infinite")"#, &names()).unwrap();
    for value in [
        r#"linear ident(var(--private_property_name)) ident("private_animation_linear")"#,
        r#"animation:1s ident(var(--private_property_name)) ident("linear")"#,
        r#"calc(var(--private_property_n)*1) ident("infinite") ident("private_animation_infinite")"#,
    ] {
        assert!(css.contains(value), "{value} in {css}");
    }
}
#[test]
fn animation_safe_prefixes_keep_native_suffixes_and_reject_keyword_or_timeline_prefixes() {
    let css = declarations(r#"animation:1s ident("ready-" var(--suffix)),1s ident("ready-" attr(data-name type(<custom-ident>)))"#, &names()).unwrap();
    assert!(
        css.contains(r#"ident("private_animation_ready-" var(--private_property_suffix))"#),
        "{css}"
    );
    assert!(
        css.contains(r#"ident("private_animation_ready-" attr(data-name type(<custom-ident>)))"#),
        "{css}"
    );
    for value in [
        r#"ident("n" var(--x))"#,
        r#"ident("a" var(--x))"#,
        r#"ident("-" var(--x))"#,
        r#"ident("--" var(--x))"#,
        r#"ident("linear-" 1.5)"#,
        r#"ident("")"#,
        r#"ident(pulse,spin)"#,
    ] {
        let body = value
            .strip_prefix("ident(")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        assert_eq!(
            Animation::default().computed(body, "private_").unwrap(),
            body,
            "{value}"
        );
    }
}
#[test]
fn animation_lists_vendor_properties_and_priority_keep_independent_roles() {
    let css = declarations(r#"-webkit-animation:1s ident("linear") ident("linear"),2s ident("linear");animation:none ident("both") ! IMPORTANT"#,&names()).unwrap();
    assert_eq!(
        css,
        r#"-webkit-animation:1s ident("linear") ident("private_animation_linear"),2s ident("linear");animation:none ident("private_animation_both") ! IMPORTANT"#
    );
}
#[test]
fn animation_invalid_global_modern_and_opaque_values_keep_native_boundaries() {
    let n = names();
    for value in [
        r#"ident("inherit")"#,
        r#"ident("default")"#,
        r#"ident("revert-rule")"#,
        r#"ident("auto")"#,
        r#"ident("--flow")"#,
        r#"ident(3px)"#,
        r#"ident(2147483648)"#,
    ] {
        let body = value
            .strip_prefix("ident(")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        assert_eq!(
            Animation::default().computed(body, "private_").unwrap(),
            body,
            "{value}"
        );
    }
    let css=declarations(r#"--opaque:ident("pulse");content:ident("pulse");animation:1s "none";animation:1s ident(7)"#,&n).unwrap();
    assert!(
        css.contains(r#"--private_property_opaque:ident("pulse");content:ident("pulse")"#),
        "{css}"
    );
    assert!(css.contains("animation:1s private_animation_none"), "{css}");
    assert!(
        css.contains(r#"animation:1s ident("private_animation_7")"#),
        "{css}"
    );
    let native = Names::native(HashSet::new(), "owner".into(), true);
    assert_eq!(
        declarations(r#"animation:1s ident("pulse")"#, &native).unwrap(),
        r#"animation:1s ident("pulse")"#
    );
}
fn fixture() -> String {
    let document = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import 'animation.css';@keyframes pulse{from{opacity:.2}to{opacity:1}}@keyframes linear{from{opacity:.2}to{opacity:1}}@keyframes both{from{opacity:.2}to{opacity:1}}@keyframes ready-tail{from{opacity:.2}to{opacity:1}}.only{animation:1s ident("linear")}.named{animation:1s ident("linear") ident("linear");-webkit-animation:2s none ident("both")}.safe{animation:1s ident("ready-" var(--suffix))}.opaque{--data:ident("pulse");animation:1s var(--unknown,linear) ident("linear")}</style><symbol id="badge"><text class="named" style="animation:1s none both;--suffix:tail">Animation Δ</text></symbol></svg>"##;
    let shared = r##".function{animation:1s steps(2,end) ident("linear")}.variables{animation:1s linear var(--unknown,linear) ident("linear");animation:1s var(--unknown) ident("pulse")}@media(min-width:0px){.comma{animation:1s ident("linear") ident("linear"),1s ident("linear")}}"##;
    let mut b = Bundle {
        resources: [
            ("a.svg", "image/svg+xml", document),
            ("b.svg", "image/svg+xml", document),
            ("animation.css", "text/css", shared),
        ]
        .into_iter()
        .map(|(p, m, s)| (p.into(), (m, s.as_bytes().to_vec())))
        .collect(),
        encoded: HashMap::new(),
        visiting: HashSet::new(),
        used: HashSet::new(),
    };
    let html=b.html(r##"<html><head><style>@keyframes pulse{from{opacity:.2}to{opacity:1}}.owner{animation:1s ident("pulse")}</style></head><body><p class="owner">Owner</p><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
    assert_eq!(b.used.len(), 3);
    for file in ["a.svg", "b.svg"] {
        let prefix = format!("ion_svg_{}_", hash(file.as_bytes()));
        assert!(
            html.contains(&format!("ident(&quot;{prefix}animation_linear&quot;)")),
            "{html}"
        );
        assert!(
            html.contains(&format!("none {prefix}animation_both")),
            "{html}"
        );
    }
    html
}
#[test]
fn animation_shorthand_publication_keeps_two_imported_definitions_independent() {
    fixture();
}
#[test]
#[ignore = "Exports the actual production animation shorthand document to ION_ANIMATION_CSS_FIXTURE"]
fn native_animation_css_phone_fixture() {
    std::fs::write(
        std::env::var_os("ION_ANIMATION_CSS_FIXTURE").expect("Set ION_ANIMATION_CSS_FIXTURE"),
        fixture(),
    )
    .unwrap();
}
