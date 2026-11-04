mod common;

use std::hint::black_box;
use std::time::Instant;

use dyn_core::VectorField;

const LORENZ: &str = "\\dot{x}=\\sigma\\left(y-x\\right)\\\\\\dot{y}=x\\left(\\rho-z\\right)-y\\\\\\dot{z}=xy-\\beta z\n\\sigma=10\n\\rho=28\n\\beta=\\frac{8}{3}";

fn time_per_call(n: u32, mut f: impl FnMut(u32)) -> f64 {
    for i in 0..n / 10 {
        f(i);
    }
    let round = n / 5;
    (0..5)
        .map(|_| {
            let start = Instant::now();
            for i in 0..round {
                f(i);
            }
            start.elapsed().as_nanos() as f64 / f64::from(round)
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
#[ignore = "benchmark: cargo test -p dyn-expr --release --test bench -- --ignored --nocapture"]
fn bench_lorenz() {
    let sys = common::latex(LORENZ);
    let p = sys.default_params();
    let mut out = [0.0; 3];
    let mut jac = [0.0; 9];
    let eval_ns = time_per_call(10_000_000, |i| {
        let x = [f64::from(i % 7), 1.0, 2.0];
        sys.eval(0.0, black_box(&x), black_box(&p), &mut out);
        black_box(&out);
    });
    let jac_ns = time_per_call(2_000_000, |i| {
        let x = [f64::from(i % 7), 1.0, 2.0];
        sys.jacobian(0.0, black_box(&x), black_box(&p), &mut jac);
        black_box(&jac);
    });
    let native_ns = time_per_call(10_000_000, |i| {
        let x = black_box([f64::from(i % 7), 1.0, 2.0]);
        let p = black_box(&p);
        out[0] = p[0] * (x[1] - x[0]);
        out[1] = x[0] * (p[1] - x[2]) - x[1];
        out[2] = x[0] * x[1] - p[2] * x[2];
        black_box(&out);
    });
    println!(
        "Lorenz ({} instructions): eval {eval_ns:.1} ns, jacobian {jac_ns:.1} ns, \
         native eval {native_ns:.1} ns",
        sys.instruction_count()
    );
}

#[test]
#[ignore = "benchmark: cargo test -p dyn-expr --release --test bench -- --ignored --nocapture"]
fn bench_hamiltonian() {
    let sys = common::text("H(q, p) = p^2/2 - cos(q)");
    let mut out = [0.0; 2];
    let mut jac = [0.0; 4];
    let eval_ns = time_per_call(5_000_000, |i| {
        sys.eval(0.0, black_box(&[f64::from(i % 5), 0.3]), &[], &mut out);
        black_box(&out);
    });
    let jac_ns = time_per_call(2_000_000, |i| {
        sys.jacobian(0.0, black_box(&[f64::from(i % 5), 0.3]), &[], &mut jac);
        black_box(&jac);
    });
    println!("Hamiltonian pendulum: eval {eval_ns:.1} ns, jacobian {jac_ns:.1} ns");
}
