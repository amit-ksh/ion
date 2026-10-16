use super::*;
fn names() -> Names {
    let mut n = Names::native(HashSet::new(), "owner".into(), true);
    n.prefix = "private_".into();
    n
}
#[test]
fn animation_whole_var_fallback_uses_complete_shorthand_roles() {
    let css=declarations(r#"animation:var(--motion,1s linear pulse);-webkit-animation:var(--motion,2s none ident("both"))"#,&names()).unwrap();
    assert_eq!(
        css,
        r#"animation:var(--private_property_motion,1s linear private_animation_pulse);-webkit-animation:var(--private_property_motion,2s none ident("private_animation_both"))"#
    );
}
#[test]
fn animation_whole_fallback_lists_nested_variables_and_authored_members_keep_their_boundaries() {
    let css = declarations(
        r#"animation:var(--x,var(--y,1s pulse,2s linear linear)),var(--z,3s none both),4s pulse"#,
        &names(),
    )
    .unwrap();
    assert_eq!(
        css,
        "animation:var(--private_property_x,var(--private_property_y,1s private_animation_pulse,2s linear private_animation_linear)),var(--private_property_z,3s none private_animation_both),4s private_animation_pulse"
    );
}
#[test]
fn animation_partial_var_fragments_and_opaque_data_do_not_gain_whole_value_rewriting() {
    let css=declarations(r#"animation:1s var(--x,pulse);animation:var(--x,pulse) 1s linear;--data:var(--x,1s pulse);content:var(--x,pulse);animation:var(--x,1s pulse)/**/pulse"#,&names()).unwrap();
    for value in [
        "animation:1s var(--private_property_x,pulse)",
        "animation:var(--private_property_x,pulse) 1s linear",
        "--private_property_data:var(--private_property_x,1s pulse)",
        "content:var(--private_property_x,pulse)",
        "animation:var(--private_property_x,1s pulse)/**/private_animation_pulse",
    ] {
        assert!(css.contains(value), "{value} in {css}");
    }
}
#[test]
fn animation_var_first_argument_and_raw_wrapper_are_not_flattened() {
    let n = names();
    for source in [
        r#" /*lead*/ VAR(/*first*/--motion,1s pulse) /*tail*/"#,
        r#"var(ident("--motion"),1s pulse)"#,
        r#"var(var(--selected,--motion),1s pulse)"#,
        r#"var("not-a-name",1s pulse)"#,
    ] {
        let rendered = n.globals.animation(source, &n.prefix, true, 0).unwrap();
        assert_eq!(
            rendered,
            source.replace("1s pulse", "1s private_animation_pulse")
        );
    }
    let css = declarations(r#"animation:var(ident("--motion"),1s ident("pulse"))"#, &n).unwrap();
    assert_eq!(
        css,
        r#"animation:var(ident("--private_property_motion"),1s ident("private_animation_pulse"))"#
    );
}
#[test]
fn animation_empty_global_quoted_comma_and_unclosed_fallbacks_keep_native_structure() {
    let n = names();
    for (source, expected) in [
        ("var(--x,)", "var(--x,)"),
        ("var(--x)", "var(--x)"),
        ("var(--x,initial)", "var(--x,initial)"),
        (
            "var(--x,inherit),1s pulse",
            "var(--x,inherit),1s private_animation_pulse",
        ),
        (
            r#"var(--x,1s "a,b")"#,
            r#"var(--x,1s private_animation_a\,b)"#,
        ),
        (
            "var(--x,1s steps(2,end) linear)",
            "var(--x,1s steps(2,end) private_animation_linear)",
        ),
        ("var(--x,1s pulse", "var(--x,1s private_animation_pulse"),
    ] {
        assert_eq!(
            n.globals.animation(source, &n.prefix, true, 0).unwrap(),
            expected,
            "{source}"
        );
    }
    assert_eq!(
        declarations("animation:var(--x,1s pulse) ! IMPORTANT", &n).unwrap(),
        "animation:var(--private_property_x,1s private_animation_pulse) ! IMPORTANT"
    );
}
#[test]
fn animation_whole_fallback_recursion_remains_bounded_without_variable_expansion() {
    let n = names();
    let source = format!("{}1s pulse{}", "var(--x,".repeat(32), ")".repeat(32));
    assert!(n.globals.animation(&source, &n.prefix, true, 0).is_ok());
    let source = format!("{}1s pulse{}", "var(--x,".repeat(33), ")".repeat(33));
    assert!(n.globals.animation(&source, &n.prefix, true, 0).is_err());
}
fn fixture() -> String {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import 'fallback.css';@keyframes pulse{from{opacity:.2}to{opacity:1}}@keyframes linear{from{opacity:.2}to{opacity:1}}@keyframes both{from{opacity:.2}to{opacity:1}}.full{animation:var(--missing,1s linear pulse);-webkit-animation:var(--missing,var(--other,2s none both))}.lists{animation:var(--multi,1s pulse,2s linear linear),var(--last,3s none both)}.present{animation:var(--present,1s pulse)}.empty{animation:var(--empty,)}.partial{animation:1s var(--unknown,linear) ident("linear");animation:var(--unknown,linear) 1s linear}.opaque{--data:var(--missing,1s pulse);content:var(--unknown,pulse)}</style><symbol id="badge"><text class="full" style="animation:var(--inline,1s pulse)">Fallback Δ</text></symbol></svg>"##;
    let css = r##".computed{animation:var(ident("--computed"),1s ident("pulse"))}.comma{animation:var(--pair,1s steps(2,end) linear,1s none both)}"##;
    let mut b = Bundle {
        resources: [
            ("a.svg", "image/svg+xml", svg),
            ("b.svg", "image/svg+xml", svg),
            ("fallback.css", "text/css", css),
        ]
        .into_iter()
        .map(|(p, m, s)| (p.into(), (m, s.as_bytes().to_vec())))
        .collect(),
        encoded: HashMap::new(),
        visiting: HashSet::new(),
        used: HashSet::new(),
    };
    let html=b.html(r##"<html><head><style>@keyframes pulse{from{opacity:.2}to{opacity:1}}.owner{animation:var(--outer,1s pulse)}</style></head><body><p class="owner">Owner</p><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
    assert_eq!(b.used.len(), 3);
    for name in ["a.svg", "b.svg"] {
        let prefix = format!("ion_svg_{}_", hash(name.as_bytes()));
        assert!(
            html.contains(&format!(
                "var(--{prefix}property_missing,1s linear {prefix}animation_pulse)"
            )),
            "{html}"
        );
    }
    html
}
#[test]
fn animation_fallback_publication_keeps_two_imported_documents_and_owner_context_separate() {
    fixture();
}
#[test]
#[ignore = "Exports actual whole-value fallback output to ION_ANIMATION_FALLBACK_FIXTURE"]
fn native_animation_fallback_phone_fixture() {
    std::fs::write(
        std::env::var_os("ION_ANIMATION_FALLBACK_FIXTURE")
            .expect("Set ION_ANIMATION_FALLBACK_FIXTURE"),
        fixture(),
    )
    .unwrap();
}
