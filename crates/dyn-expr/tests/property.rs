mod common;

use common::*;
use dyn_core::VectorField;
use dyn_expr::CompiledSystem;

fn richardson(f: impl Fn(f64) -> Vec<f64>, x0: f64) -> (Vec<f64>, Vec<f64>) {
    let scale = x0.abs().max(1.0);
    let d = |h: f64| -> Vec<f64> {
        let (a, b) = (f(x0 + h), f(x0 - h));
        a.iter().zip(&b).map(|(a, b)| (a - b) / (2.0 * h)).collect()
    };
    let mut best: Option<(Vec<f64>, Vec<f64>)> = None;
    for h in [1e-2, 1e-3, 1e-4].map(|h| h * scale) {
        let (d1, d2, d3) = (d(h), d(h / 2.0), d(h / 4.0));
        let mut est = Vec::new();
        let mut err = Vec::new();
        for i in 0..d1.len() {
            let r1 = (4.0 * d2[i] - d1[i]) / 3.0;
            let r2 = (4.0 * d3[i] - d2[i]) / 3.0;
            let r = (16.0 * r2 - r1) / 15.0;
            est.push(r);
            err.push((r - r2).abs());
        }
        best = Some(match best {
            None => (est, err),
            Some((mut be, mut br)) => {
                for i in 0..est.len() {
                    if err[i] < br[i] {
                        be[i] = est[i];
                        br[i] = err[i];
                    }
                }
                (be, br)
            }
        });
    }
    best.expect("at least one step")
}

fn assert_matches_fd(ad: &[f64], (est, err): (Vec<f64>, Vec<f64>), what: &str) {
    for i in 0..ad.len() {
        let tol = 10.0 * err[i] + 1e-7 * (1.0 + est[i].abs());
        assert!(
            (ad[i] - est[i]).abs() <= tol,
            "{what}[{i}]: AD {} vs FD {} (±{})",
            ad[i],
            est[i],
            err[i]
        );
    }
}

fn check_ad(sys: &CompiledSystem, x: &[f64], p: &[f64]) {
    let n = sys.dim();
    let j = jac(sys, 0.0, x, p);
    for c in 0..n {
        let fd = richardson(
            |v| {
                let mut xx = x.to_vec();
                xx[c] = v;
                sys.eval_alloc(0.0, &xx, p)
            },
            x[c],
        );
        let col: Vec<f64> = (0..n).map(|r| j[r * n + c]).collect();
        assert_matches_fd(&col, fd, &format!("column {c}"));
    }
    for k in 0..sys.n_params() {
        let mut ad = vec![0.0; n];
        sys.param_jacobian(0.0, x, p, k, &mut ad);
        let fd = richardson(
            |v| {
                let mut pp = p.to_vec();
                pp[k] = v;
                sys.eval_alloc(0.0, x, &pp)
            },
            p[k],
        );
        assert_matches_fd(&ad, fd, &format!("param {k}"));
    }
}

#[test]
fn random_expressions_ad_matches_finite_differences() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let mut checked = 0;
    for case in 0..300 {
        let f = gen(&mut r, 4);
        let g = gen(&mut r, 4);
        let txt = format!("x' = {} + 0*y*a\ny' = {} + 0*x", to_text(&f), to_text(&g));
        let ltx = format!(
            "\\dot{{x}}={}+0ya\\\\\\dot{{y}}={}+0x",
            to_latex(&f),
            to_latex(&g)
        );
        let st = text(&txt);
        let sl = latex(&ltx);
        assert_eq!(names(&st), names(&sl), "case {case}");
        assert_eq!(names(&st).1, ["a"], "case {case}");
        for _ in 0..3 {
            let x = [r.uniform(-1.5, 1.5), r.uniform(-1.5, 1.5)];
            let p = [r.uniform(0.5, 1.5)];
            let want = [eval(&f, x[0], x[1], p[0]), eval(&g, x[0], x[1], p[0])];
            let vt = st.eval_alloc(0.0, &x, &p);
            let vl = sl.eval_alloc(0.0, &x, &p);
            assert_close(&vt, &want, 1e-12);
            assert_close(&vl, &want, 1e-12);
            check_ad(&st, &x, &p);
            assert_close(&jac(&sl, 0.0, &x, &p), &jac(&st, 0.0, &x, &p), 1e-12);
            checked += 1;
        }
    }
    assert_eq!(checked, 900);
}

#[test]
fn random_hamiltonians_match_finite_differences() {
    let mut r = Rng(0xD1B5_4A32_D192_ED03);
    for case in 0..100 {
        let h = gen(&mut r, 3);
        let src = format!("H(x, y) = {} + 0*x*y*a", to_text(&h));
        let sys = text(&src);
        for _ in 0..3 {
            let x = [r.uniform(-1.2, 1.2), r.uniform(-1.2, 1.2)];
            let p = [r.uniform(0.5, 1.5)];
            let hv = |q: f64, m: f64| eval(&h, q, m, p[0]);
            assert_close(
                &[sys.hamiltonian_value(0.0, &x, &p).unwrap()],
                &[hv(x[0], x[1])],
                1e-12,
            );
            let f = sys.eval_alloc(0.0, &x, &p);
            assert_matches_fd(&[f[0]], richardson(|m| vec![hv(x[0], m)], x[1]), "dH/dp");
            assert_matches_fd(&[-f[1]], richardson(|q| vec![hv(q, x[1])], x[0]), "dH/dq");
            check_ad(&sys, &x, &p);
            let j = jac(&sys, 0.0, &x, &p);
            assert!(
                (j[0] + j[3]).abs() < 1e-9 * (1.0 + j[0].abs()),
                "case {case}: {j:?}"
            );
        }
    }
}
