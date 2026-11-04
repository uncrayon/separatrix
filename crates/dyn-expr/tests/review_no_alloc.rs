//! `VectorField` calls on a compiled system must not allocate (contract in
//! `dyn-core`). A counting global allocator checks that `eval`,
//! `jacobian` and `param_jacobian` make zero heap allocations after the
//! first (warm-up) call, for small programs (stack registers), large ones
//! (thread-local registers) and Hamiltonians (dual and nested-dual runs).

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use dyn_core::VectorField;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: forwards to the system allocator; only counts calls.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc_zeroed(l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        System.realloc(p, l, n)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn allocations_during(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(Cell::get);
    f();
    ALLOCS.with(Cell::get) - before
}

fn check(src: &str) {
    let sys = common::latex(src);
    let n = sys.dim();
    let x: Vec<f64> = (0..n).map(|i| 0.3 + 0.1 * i as f64).collect();
    let p: Vec<f64> = (0..sys.n_params()).map(|k| 1.1 + 0.1 * k as f64).collect();
    let mut out = vec![0.0; n];
    let mut jac = vec![0.0; n * n];
    let mut run = || {
        sys.eval(0.2, &x, &p, &mut out);
        sys.jacobian(0.2, &x, &p, &mut jac);
        for k in 0..p.len() {
            sys.param_jacobian(0.2, &x, &p, k, &mut out);
        }
    };
    run(); // warm-up: thread-local buffers may grow once
    let count = allocations_during(|| {
        for _ in 0..100 {
            run();
        }
    });
    assert_eq!(
        count,
        0,
        "{src}: {count} allocations in 100 rounds ({} instructions)",
        sys.instruction_count()
    );
}

#[test]
fn evaluation_does_not_allocate() {
    check(r"\dot{x}=\sigma\left(y-x\right)\\\dot{y}=x\left(\rho-z\right)-y\\\dot{z}=xy-\beta z");
    check(r"\theta^{\prime\prime}=-\frac{g}{L}\sin\theta");
    check(r"x_{n+1}=rx_n(1-x_n)");
    check(r"H(q,p)=\frac{p^2}{2m}+\frac{1}{2}kq^2");
    check(
        r"H(q_1,q_2,p_1,p_2)=\frac{p_1^2+p_2^2}{2}+\frac{1}{2}\left(q_1^2+q_2^2\right)+q_1^2q_2-\frac{q_2^3}{3}",
    );
    // Large enough (> 32 instructions) to use the thread-local buffer.
    let big = r"\dot{u}=\sin u\cos v+\tanh w-\frac{u^3}{1+v^2}+a\ln\left(2+u^2\right)
\dot{v}=e^{-u^2}\sin\left(\omega t\right)+\arctan\left(vw\right)-bv^5+\sqrt{1+w^2}
\dot{w}=\cosh\left(\frac{u}{3}\right)-\sinh\left(\frac{v}{5}\right)+c\left|u-w\right|+\max\left(u,v\right)";
    let sys = common::latex(big);
    assert!(sys.instruction_count() > 32, "{}", sys.instruction_count());
    check(big);
}
