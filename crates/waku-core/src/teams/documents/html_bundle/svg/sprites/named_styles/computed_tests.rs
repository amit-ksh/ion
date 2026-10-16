use super::*;
fn names() -> Names {
    let mut names = Names::native(HashSet::new(), "owner".into(), true);
    names.prefix = "private_".into();
    for name in [
        "marks",
        "mark-tail",
        "mark-none",
        "inside",
        "outside",
        "lower-roman",
        "lower-custom",
        "Σ",
        r"\37",
    ] {
        names.globals.collect_rule("counter-style", name).unwrap();
    }
    names
}
#[test]
fn computed_counter_literal_results_follow_style_argument_roles() {
    let css=declarations(r#"list-style-type:ident("ma" rks);list-style:inside ident("marks");content:counter(ident("marks"),ident("marks")) counters(section,"marks",ident("marks"));"#,&names()).unwrap();
    assert!(
        css.contains(r#"list-style-type:ident("private_counter_style_marks")"#),
        "{css}"
    );
    assert!(
        css.contains(r#"list-style:inside ident("private_counter_style_marks")"#),
        "{css}"
    );
    assert!(css.contains(r#"counter(ident("marks"),ident("private_counter_style_marks")) counters(section,"marks",ident("private_counter_style_marks"))"#),"{css}");
}
#[test]
fn computed_counter_literal_fallbacks_preserve_var_selection_and_opaque_data() {
    let css=declarations(r#"list-style-type:var(--style,ident("marks"));list-style:var(--style,inside marks);content:counter(var(--step,marks),var(--style,ident("marks")));--data:var(--missing,marks) ident("marks");--caption:counter(step,ident("marks"))"#,&names()).unwrap();
    for value in [
        r#"list-style-type:var(--private_property_style,ident("private_counter_style_marks"))"#,
        r#"list-style:var(--private_property_style,inside private_counter_style_marks)"#,
        r#"counter(var(--private_property_step,marks),var(--private_property_style,ident("private_counter_style_marks")))"#,
        r#"--private_property_data:var(--private_property_missing,marks) ident("marks")"#,
        r#"--private_property_caption:counter(step,ident("private_counter_style_marks"))"#,
    ] {
        assert!(css.contains(value), "{value} in {css}");
    }
}
#[test]
fn computed_counter_safe_authored_prefix_keeps_suffix_evaluation_native() {
    let css=declarations(r#"list-style-type:ident("mark-" var(--suffix));content:counter(step,ident("mark-" attr(data-kind type(<custom-ident>))));list-style:inside ident("mark-" var(--suffix));list-style-type:ident("unknown-" var(--suffix))"#,&names()).unwrap();
    assert!(
        css.contains(r#"ident("private_counter_style_mark-" var(--private_property_suffix))"#),
        "{css}"
    );
    assert!(
        css.contains(
            r#"ident("private_counter_style_mark-" attr(data-kind type(<custom-ident>)))"#
        ),
        "{css}"
    );
    assert!(
        css.contains(r#"ident("unknown-" var(--private_property_suffix))"#),
        "{css}"
    );
}
#[test]
fn computed_counter_predefined_names_canonicalize_only_complete_results() {
    let n = names();
    let css=declarations(r#"list-style-type:ident("LOWER-" ROMAN);list-style-type:ident("DISC");list-style-type:ident("lower-" var(--suffix));list-style-type:ident("LOWER-" var(--suffix));content:counter(step,ident("inside"));list-style:ident("inside") ident("marks")"#,&n).unwrap();
    for value in [
        r#"ident("private_counter_style_lower-roman")"#,
        r#"ident("DISC")"#,
        r#"ident("lower-" var(--private_property_suffix))"#,
        r#"ident("LOWER-" var(--private_property_suffix))"#,
        r#"counter(step,ident("private_counter_style_inside"))"#,
        r#"list-style:ident("inside") ident("private_counter_style_marks")"#,
    ] {
        assert!(css.contains(value), "{value} in {css}");
    }
    assert!(
        n.globals.baseline(&n.prefix).unwrap().contains(
            "@counter-style private_counter_style_lower-roman{system:extends lower-roman}"
        )
    );
}
#[test]
fn computed_counter_invalid_reserved_and_unknown_results_remain_native() {
    let n = names();
    for value in [
        r#"ident("none")"#,
        r#"ident("inherit")"#,
        r#"ident("revert-rule")"#,
        r#"ident("")"#,
        r#"ident("marks",tail)"#,
        r#"ident(3px)"#,
        r#"ident(1.5)"#,
        r#"ident(2147483648)"#,
        r#"ident("Marks")"#,
        r#"ident("unknown")"#,
        r#"ident(var(--name))"#,
    ] {
        let actual = n
            .globals
            .value("list-style-type", value, &n.prefix, false)
            .unwrap();
        assert_eq!(actual, value, "{value}");
    }
    let css=declarations(r#"list-style-type:ident(7);list-style-type:ident("Σ");content:"marks";list-style-type:"marks""#,&n).unwrap();
    assert!(css.contains(r#"ident("private_counter_style_7")"#), "{css}");
    assert!(css.contains(r#"ident("private_counter_style_Σ")"#), "{css}");
    assert!(
        css.contains(r#"content:"marks";list-style-type:"marks""#),
        "{css}"
    );
}
#[test]
fn computed_counter_descriptors_and_rule_names_do_not_gain_substitution_support() {
    let n = names();
    let css=namespace_styles(r#"@counter-style marks{system:extends marks;fallback:ident("marks");speak-as:var(--voice,marks);symbols:ident("marks")}@counter-style ident("marks"){system:cyclic;symbols:"x"}"#,&n,0).unwrap();
    for value in [
        "@counter-style private_counter_style_marks",
        r#"system:extends private_counter_style_marks;fallback:ident("marks")"#,
        r#"speak-as:var(--private_property_voice,marks)"#,
        r#"symbols:ident("marks")"#,
        r#"@counter-style ident("marks")"#,
    ] {
        assert!(css.contains(value), "{value} in {css}");
    }
}
#[test]
fn computed_counter_depth_and_definition_budgets_are_enforced() {
    let n = names();
    let value = format!("{}marks{}", "var(--style,".repeat(34), ")".repeat(34));
    assert!(
        n.globals
            .value("list-style-type", &value, &n.prefix, false)
            .is_err()
    );
    let mut globals = Globals::default();
    for index in 0..4096 {
        globals
            .collect_rule("counter-style", &format!("mark-{index}"))
            .unwrap();
    }
    assert!(globals.collect_rule("counter-style", "overflow").is_err());
}
fn fixture() -> String {
    let document = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import 'counter.css';.shape{list-style-type:ident("ma" rks);list-style:inside var(--unknown,ident("marks"));content:counter(step,ident("mark-" var(--suffix))) counters(chapter,".",var(--unknown,marks));--opaque:ident("marks");--caption:counter(step,ident("marks"))}</style><symbol id="badge"><text class="shape" style="list-style-type:ident(7);--suffix:tail">Counter Δ 7</text></symbol></svg>"##;
    let shared = r##"@counter-style marks{system:cyclic;symbols:"A" "B";fallback:decimal}@counter-style mark-tail{system:extends marks}@counter-style \37 {system:cyclic;symbols:"7"}@media(min-width:999999px){@counter-style LOWER-ROMAN{system:cyclic;symbols:"R"}}.shape{list-style-type:ident("LOWER-" ROMAN);content:counter(var(--step,marks),var(--unknown,ident("marks")));list-style-type:ident("lower-" var(--suffix));list-style-type:ident("none");--opaque:ident("marks")}"##;
    let mut bundle = Bundle {
        resources: [
            ("a.svg", "image/svg+xml", document),
            ("b.svg", "image/svg+xml", document),
            ("counter.css", "text/css", shared),
        ]
        .into_iter()
        .map(|(p, m, s)| (p.into(), (m, s.as_bytes().to_vec())))
        .collect(),
        encoded: HashMap::new(),
        visiting: HashSet::new(),
        used: HashSet::new(),
    };
    let html=bundle.html(r##"<html><head><style>@counter-style marks{system:cyclic;symbols:"O"}.owner{list-style-type:ident("marks")}</style></head><body><div class="owner">Owner</div><svg xmlns="http://www.w3.org/2000/svg"><use href="a.svg#badge"/><use href="b.svg#badge"/></svg></body></html>"##,"index.html").unwrap();
    assert_eq!(bundle.used.len(), 3);
    for file in ["a.svg", "b.svg"] {
        let prefix = format!("ion_svg_{}_", hash(file.as_bytes()));
        assert!(
            html.contains(&format!("ident(&quot;{prefix}counter_style_marks&quot;)")),
            "{html}"
        );
    }
    html
}
#[test]
fn computed_counter_publication_keeps_two_imported_styles_independent() {
    fixture();
}
#[test]
#[ignore = "Exports actual native counter-style output to ION_COUNTER_CSS_FIXTURE"]
fn native_counter_css_phone_fixture() {
    std::fs::write(
        std::env::var_os("ION_COUNTER_CSS_FIXTURE").expect("Set ION_COUNTER_CSS_FIXTURE"),
        fixture(),
    )
    .unwrap();
}
