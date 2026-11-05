//! `dyn_expr::outline`: every gallery system splits into its equations,
//! with the role the analysis gives each line and both sides as typed.

use dyn_expr::{compile, outline, portable_latex, InputFormat, OutlineRole};

const TEMPLATES_JSON: &str = include_str!("../../../app/src/templates/templates.json");

fn templates() -> Vec<(String, Vec<String>)> {
    let doc: serde_json::Value = serde_json::from_str(TEMPLATES_JSON).unwrap();
    doc.as_array()
        .unwrap()
        .iter()
        .filter(|t| t["format"] == "latex")
        .map(|t| {
            (
                t["id"].as_str().unwrap().to_string(),
                t["equations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|l| l.as_str().unwrap().to_string())
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn every_template_has_an_outline_that_matches_its_states() {
    let all = templates();
    assert!(all.len() >= 10);
    for (id, lines) in all {
        let o = outline(&lines).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        let info = compile(&lines.join("\n"), InputFormat::Latex)
            .system
            .unwrap()
            .info;
        assert!(!o.is_empty(), "{id}");
        for e in &o {
            assert!(!e.lhs.is_empty() && !e.rhs.is_empty(), "{id}: {e:?}");
            assert!(!e.lhs.contains('&') && !e.rhs.contains('&'));
            match &e.role {
                OutlineRole::Flow { state, .. } | OutlineRole::Map { state, .. } => {
                    assert!(
                        info.states.iter().any(|s| &s.name == state),
                        "{id}: {state}"
                    );
                }
                OutlineRole::Implicit { state, .. } => {
                    assert!(
                        info.states.iter().any(|s| &s.name == state),
                        "{id}: {state}"
                    );
                }
                OutlineRole::Hamiltonian => assert!(info.hamiltonian, "{id}"),
                OutlineRole::Parameter { name } => {
                    assert!(info.params.iter().any(|p| &p.name == name), "{id}: {name}");
                }
            }
        }
        // One equation per state the user gave an equation for.
        let defining = o
            .iter()
            .filter(|e| !matches!(e.role, OutlineRole::Parameter { .. }))
            .count();
        let typed_states = info.states.iter().filter(|s| !s.derived).count();
        if info.hamiltonian {
            assert_eq!(defining, 1, "{id}");
        } else {
            assert_eq!(defining, typed_states, "{id}");
        }
    }
}

#[test]
fn roles_and_text_for_a_few_systems() {
    let lines = |ls: &[&str]| -> Vec<String> { ls.iter().map(|l| l.to_string()).collect() };
    let o = outline(&lines(&["x_{n+1}=rx_n\\left(1-x_n\\right)"])).unwrap();
    assert_eq!(o.len(), 1);
    match &o[0].role {
        OutlineRole::Map {
            state,
            base_latex,
            sub_latex,
            ..
        } => {
            assert_eq!(state, "x");
            assert_eq!(base_latex, "x");
            assert_eq!(*sub_latex, None);
        }
        r => panic!("{r:?}"),
    }
    assert_eq!(o[0].lhs, "x_{n+1}");
    assert_eq!(o[0].rhs, "rx_n\\left(1-x_n\\right)");

    let o = outline(&lines(&[
        "\\dot{\\theta}_{1}=\\omega_{1}",
        "\\dot{\\omega}_{1}=-\\sin\\theta_{1}",
    ]))
    .unwrap();
    match &o[0].role {
        OutlineRole::Flow {
            base_latex,
            sub_latex,
            state_latex,
            ..
        } => {
            assert_eq!(base_latex, "\\theta");
            assert_eq!(sub_latex.as_deref(), Some("1"));
            assert_eq!(state_latex, "\\theta_{1}");
        }
        r => panic!("{r:?}"),
    }
    // A system that does not compile has no outline.
    assert!(outline(&lines(&["\\dot{x}=y"])).is_ok());
    assert!(outline(&lines(&["\\dot{x}="])).is_err());
}

#[test]
fn portable_latex_leaves_plain_latex_alone() {
    for (_, lines) in templates() {
        for l in lines {
            let p = portable_latex(&l);
            // The gallery is written in plain LaTeX already.
            assert_eq!(p, l);
        }
    }
    assert_eq!(
        portable_latex("\\mleft(\\exponentialE\\mright)"),
        "\\left(\\mathrm{e}\\right)"
    );
}

#[test]
fn derivatives_are_picked_out_of_both_sides() {
    use dyn_expr::LatexPart;
    let lines = |ls: &[&str]| -> Vec<String> { ls.iter().map(|l| l.to_string()).collect() };
    let o = outline(&lines(&["\\ddot{x} = -x - \\mu\\dot{x}(t) &"])).unwrap();
    assert_eq!(
        o[0].rhs_parts,
        vec![
            LatexPart::Text {
                text: "-x - \\mu".into()
            },
            LatexPart::Derivative {
                state: "x".into(),
                base_latex: "x".into(),
                sub_latex: None,
                order: 1,
                of_t: true,
            },
        ]
    );
    assert_eq!(o[0].lhs_parts.len(), 1);
    // The font a state was typed in is kept, on the left as on the right.
    let o = outline(&lines(&["\\dot{\\mathrm{S}}_{1}=-\\mathrm{S}_1"])).unwrap();
    match &o[0].role {
        OutlineRole::Flow { base_latex, .. } => assert_eq!(base_latex, "\\mathrm{S}"),
        r => panic!("{r:?}"),
    }
    // An implicit equation: derivatives on the left too.
    let o = outline(&lines(&["\\ddot{\\theta} + b\\theta' + \\sin\\theta = 0"])).unwrap();
    let orders: Vec<u32> = o[0]
        .lhs_parts
        .iter()
        .filter_map(|p| match p {
            LatexPart::Derivative { order, .. } => Some(*order),
            _ => None,
        })
        .collect();
    assert_eq!(orders, [2, 1]);
}
