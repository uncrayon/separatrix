"""Recompute the reference values of docs/codim2-reference.md.

Each point is found by solving its defining equations directly (Newton from
the published value, a closed form, or shooting), never by continuation, so
the result is independent of the code under test.

    uv run --with numpy --with scipy --with sympy python scripts/reference/codim2_reference.py
"""

import numpy as np
import sympy as sp
from scipy.integrate import solve_ivp
from scipy.optimize import brentq, fsolve

np.set_printoptions(precision=6, suppress=True, linewidth=150)


def build(states, params, f):
    """Lambdified field, Jacobian, and second and third derivative tensors."""
    X = sp.Matrix(f)
    H = [sp.hessian(fi, states) for fi in f]
    T3 = [[[[sp.diff(fi, a, b, c) for c in states] for b in states] for a in states] for fi in f]
    args = list(states) + list(params)
    return dict(
        n=len(states),
        F=sp.lambdify(args, X, "numpy"),
        J=sp.lambdify(args, X.jacobian(states), "numpy"),
        H=sp.lambdify(args, H, "numpy"),
        T=sp.lambdify(args, T3, "numpy"),
    )


def parts(m, u):
    F = np.array(m["F"](*u), float).ravel()
    A = np.array(m["J"](*u), float)
    H = np.array(m["H"](*u), float)
    T = np.array(m["T"](*u), float)
    B = lambda a, b: np.einsum("ijk,j,k->i", H, a, b)
    C = lambda a, b, c: np.einsum("ijkl,j,k,l->i", T, a, b, c)
    return F, A, B, C


def fold_a(m, u):
    """Fold coefficient a = <p, B(q,q)>/2 with <q,q> = <p,q> = 1. Its sign
    follows the orientation of q."""
    _, A, B, _ = parts(m, u)
    U, _, Vt = np.linalg.svd(A)
    q, p = Vt[-1], U[:, -1]
    return 0.5 * ((p / (p @ q)) @ B(q, q))


def l1(m, u):
    """(l1, omega) at a Hopf point:
    l1 = Re<p, C(q,q,q̄) − 2B(q, A⁻¹B(q,q̄)) + B(q̄, (2iωI − A)⁻¹B(q,q))>/2ω
    with <q,q> = <p,q> = 1 (Kuznetsov, Elements, §5.4). MatCont prints this
    value times omega."""
    _, A, B, C = parts(m, u)
    w, V = np.linalg.eig(A)
    i = np.argmax(w.imag)
    om = w[i].imag
    q = V[:, i] / np.linalg.norm(V[:, i])
    wl, W = np.linalg.eig(A.T)
    p = W[:, np.argmin(abs(wl + 1j * om))]
    p = p / np.conj(np.vdot(p, q))
    I = np.eye(m["n"])
    h11 = np.linalg.solve(A, B(q, np.conj(q)))
    h20 = np.linalg.solve(2j * om * I - A, B(q, q))
    c = np.vdot(p, C(q, q, np.conj(q)) - 2 * B(q, h11) + B(np.conj(q), h20))
    return c.real / (2 * om), om


def ns_d(m, u):
    """(d, mu) at a Neimark–Sacker point: d = Re(μ̄ c1),
    c1 = <p, C(q,q,q̄) + 2B(q, (I − A)⁻¹B(q,q̄)) + B(q̄, (μ²I − A)⁻¹B(q,q))>/2,
    <q,q> = <p,q> = 1 (Kuznetsov, Elements, §4.7). MatContM's coefficient."""
    _, A, B, C = parts(m, u)
    w, V = np.linalg.eig(A)
    i = np.argmax(w.imag)
    mu = w[i]
    q = V[:, i] / np.linalg.norm(V[:, i])
    wl, W = np.linalg.eig(A.T)
    p = W[:, np.argmin(abs(wl - np.conj(mu)))]
    p = p / np.conj(np.vdot(p, q))
    I = np.eye(m["n"])
    qb = np.conj(q)
    h11 = np.linalg.solve(I - A, B(q, qb))
    h20 = np.linalg.solve(mu**2 * I - A, B(q, q))
    c1 = np.vdot(p, C(q, q, qb) + 2 * B(q, h11) + B(qb, h20)) / 2
    return (np.conj(mu) * c1).real, mu


def solve(g, u0, label, extra=lambda u: ""):
    u, _, ok, _ = fsolve(g, u0, full_output=True, xtol=1e-13)
    res = np.linalg.norm(g(u))
    print(f"{label:36s} ok={ok == 1} res={res:.1e}  u={np.array2string(u, precision=8)} {extra(u)}")
    return u


def bykov():
    print("===== 1. Bykov–Yablonskii–Kim CO oxidation (MatCont Tutorial IV) =====")
    x, y, s, Q2, K = sp.symbols("x y s Q2 K")
    Q1, Q3, Q4, Q5, Q6 = 2.5, 10, 0.0675, 1, 0.1
    z = 1 - x - y - s
    m = build(
        [x, y, s],
        [Q2, K],
        [2 * Q1 * z**2 - 2 * Q5 * x**2 - Q3 * x * y, Q2 * z - Q6 * y - Q3 * x * y, Q4 * z - K * Q4 * s],
    )

    def hopf(A):  # λ³ + c₂λ² + c₁λ + c₀ has ±iω roots where c₀ = c₁c₂
        c = np.poly(A)
        return c[3] - c[1] * c[2]

    at = lambda v: parts(m, [*v, 0.4])
    for lab, u0 in [
        ("LP  (published Q2=1.042049)", [0.024717, 0.450257, 0.375018, 1.042049]),
        ("LP  (published Q2=1.052200)", [0.054030, 0.302241, 0.459807, 1.052200]),
    ]:
        solve(lambda v: [*at(v)[0], np.linalg.det(at(v)[1])], u0, lab, lambda v: f"|a|={abs(fold_a(m, [*v, 0.4])):.6e}")
    for lab, u0 in [
        ("H   (published Q2=1.051557)", [0.016358, 0.523971, 0.328337, 1.051557]),
        ("H   (published Q2=1.040991)", [0.077929, 0.233063, 0.492149, 1.040991]),
    ]:
        solve(
            lambda v: [*at(v)[0], hopf(at(v)[1])],
            u0,
            lab,
            lambda v: "l1=%.6e omega=%.6f l1*omega=%.6e" % (*l1(m, [*v, 0.4]), np.prod(l1(m, [*v, 0.4]))),
        )

    def bt(v):  # double zero eigenvalue: c₀ = c₁ = 0
        F, A, *_ = parts(m, v)
        c = np.poly(A)
        return [*F, c[3], c[2]]

    for lab, u0 in [
        ("BT  (published 1.417628, 0.971397)", [0.115909, 0.315467, 0.288437, 1.417628, 0.971397]),
        ("BT  (published 1.161199, 0.722339)", [0.016337, 0.638410, 0.200456, 1.161199, 0.722339]),
    ]:
        solve(bt, u0, lab)
    solve(
        lambda v: [*parts(m, v)[0], np.linalg.det(parts(m, v)[1]), fold_a(m, v)],
        [0.035940, 0.352008, 0.451368, 1.006408, 0.355991],
        "CP  (published 1.006408, 0.355991)",
    )
    for lab, u0 in [
        ("GH  (published 0.891319, 0.232487)", [0.018022, 0.368238, 0.497968, 0.891319, 0.232487]),
        ("GH  (published 0.924255, 0.305879)", [0.064311, 0.211095, 0.554870, 0.924255, 0.305879]),
    ]:
        solve(lambda v: [*parts(m, v)[0], hopf(parts(m, v)[1]), l1(m, v)[0]], u0, lab, lambda v: "omega=%.6f" % l1(m, v)[1])


def lorenz84():
    print("\n===== 2. Extended Lorenz-84 (BifurcationKit docs) =====")
    X, Y, Z, U, F, T = sp.symbols("X Y Z U F T")
    al, be, G, de, ga = sp.Rational(1, 4), 1, 0.25, 1.04, 0.987
    m = build(
        [X, Y, Z, U],
        [F, T],
        [-Y**2 - Z**2 - al * X + al * F - ga * U**2, X * Y - be * X * Z - Y + G, be * X * Y + X * Z - Z, -de * U + ga * U * X + T],
    )
    # Natural continuation in F from the docs' start at T = 0.04, to seed the fold.
    v = np.array([2.9787004394953343, -0.03868302503393752, 0.058232737694740085, -0.02105288273117459])
    Fv = last = 3.0
    while Fv > 1.3:
        w, _, ok, _ = fsolve(lambda w: parts(m, [*w, Fv, 0.04])[0], v, full_output=True, xtol=1e-13)
        if ok != 1 or np.linalg.norm(w - v) > 0.2:
            break
        v, last, Fv = w, Fv, Fv - 0.002

    def fold(q, Tv):
        F_, A, *_ = parts(m, [*q, Tv])
        return [*F_, np.linalg.det(A)]

    w = solve(lambda q: fold(q, 0.04), [*v, last], "LP at T=0.04 (published F=1.54664839)")
    for Tv in np.linspace(0.04, 0.022, 10):
        w = fsolve(lambda q: fold(q, Tv), w, xtol=1e-13)

    def bt(q):
        F_, A, *_ = parts(m, q)
        c = np.poly(A)
        return [*F_, c[4], c[3]]

    solve(bt, [*w, 0.022], "BT (published F=1.4467165, T=0.0209401)")


def predator_prey_map():
    print("\n===== 3. Predator–prey map (MatContM Tutorial M-II), b=3, d=7/2 =====")
    a, e, x, y = sp.symbols("a epsilon x y")
    b, d = 3, sp.Rational(7, 2)
    g = sp.Matrix([a * x * (1 - x) - b * x * y / (1 + e * x), d * x * y / (1 + e * x)])
    xs = 1 / (d - e)
    ys = d * xs * (a * (1 - xs) - 1) / b
    assert sp.simplify(g.subs({x: xs, y: ys}) - sp.Matrix([xs, ys])) == sp.zeros(2, 1)
    J = g.jacobian([x, y]).subs({x: xs, y: ys})
    tr, dt = sp.simplify(J.trace()), sp.simplify(J.det())
    print("F3 at a=4.1, eps=1:", [float(t.subs({a: 4.1, e: 1})) for t in (xs, ys)], "(published 0.4000 0.6813)")
    a_ns = sp.solve(sp.Eq(dt, 1), a)
    print("NS curve (det = 1): a =", a_ns, "; at eps=1:", [t.subs(e, 1) for t in a_ns], "(published a=5)")
    print("PD curve (1 + tr + det = 0): a =", sp.solve(sp.Eq(1 + tr + dt, 0), a))
    a_p1 = sp.solve(sp.Eq(1 - tr + dt, 0), a)
    print("mu = +1 curve: a =", a_p1, "; at eps=1:", [t.subs(e, 1) for t in a_p1], "(published BP a=5/3)")
    for name, trv in [("R1", 2), ("R2", -2), ("R3", -1), ("R4", 0)]:
        for so in sp.solve([sp.Eq(dt, 1), sp.Eq(tr, trv)], [a, e], dict=True):
            print(f"{name}: eps = {so[e]} = {float(so[e]):.10f}, a = {so[a]} = {float(so[a]):.10f}, F3 = ({float(xs.subs(so)):.10f}, {float(ys.subs(so)):.10f})")
    for so in sp.solve([sp.Eq(dt, -1), sp.Eq(tr, 0)], [a, e], dict=True):
        print(f"mu = +1 and -1: eps = {so[e]}, a = {so[a]}, F3 = ({float(xs.subs(so)):.6f}, {float(ys.subs(so)):.6f})")

    # Chenciner: d = 0 on the Neimark–Sacker curve.
    m = build([x, y], [a, e], list(g.subs({b: 3})))

    def on_ns(ev):
        av = (2 * ev - 7) / (2 * ev - 3)
        xv = 1 / (3.5 - ev)
        yv = 3.5 * xv * (av * (1 - xv) - 1) / 3
        return ns_d(m, [xv, yv, av, ev]), av, xv, yv

    print("d on the NS curve at eps=1, a=5:", on_ns(1.0)[0][0])
    ev = brentq(lambda v: on_ns(v)[0][0], -5, -3, xtol=1e-14)
    (dv, mu), av, xv, yv = on_ns(ev)
    print(f"CH: eps = {ev:.10f}, a = {av:.10f}, F3 = ({xv:.10f}, {yv:.10f}), theta = {np.angle(mu):.8f}, d = {dv:.1e}")


def delayed_logistic():
    print("\n===== 3b. Delayed logistic map with migration (MatContM Tutorial M-I) =====")
    x, y, r, ep = sp.symbols("x y r epsilon")
    m = build([x, y], [r, ep], [r * x * (1 - y) + ep, x])
    d, mu = ns_d(m, [0.5, 0.5, 2.0, 0.0])
    print(f"NS at r=2, eps=0: d = {d:.6f}, theta = {np.degrees(np.angle(mu)):.4f} deg (published -1.0, 60 deg)")
    print("NS curve: x = 1/r, eps = 2/r - 1, 2 cos(theta) = r - 1")
    for q in (4, 5, 6, 7):
        rv = 1 + 2 * np.cos(2 * np.pi / q)
        print(f"  1:{q} resonance: r = {rv:.10f}, eps = {2 / rv - 1:.10f}")


def freire():
    print("\n===== 4. Freire circuit (AUTO-07p HomCont demo cir) =====")
    r, a3, b3 = 0.6, 0.328578, 0.933578

    def f(t, u, nu, be):
        x, y, z = u
        return [(-(be + nu) * x + be * y - a3 * x**3 + b3 * (y - x) ** 3) / r, be * x - be * y - z - b3 * (y - x) ** 3, y]

    def spec(nu, be):
        A = np.array([[-(be + nu) / r, be / r, 0], [be, -be, -1], [0, 1, 0]])
        w, V = np.linalg.eig(A)
        i = np.argmax(w.real)
        wl, W = np.linalg.eig(A.T)
        v = V[:, i].real
        v /= np.linalg.norm(v)
        l = W[:, np.argmax(wl.real)].real
        return w, i, v, l / (l @ v)

    for lab, nu, be in [
        ("start point", -0.7213093, 0.6),
        ("neutral saddle-focus (label 10)", -0.7204001, 0.5912315),
        ("neutrally-divergent (label 5)", -0.7256925, 0.4535645),
        ("Bogdanov–Takens, nu=-beta=-sqrt(r)", -np.sqrt(r), np.sqrt(r)),
    ]:
        w, i, *_ = spec(nu, be)
        rest = np.delete(w, i)
        sigma = f" sigma={(w[i] + rest[0]).real:.5f}" if abs(rest[0].imag) > 1e-6 else ""
        print(f"{lab:36s} nu={nu:.7f} beta={be:.7f} eig={np.array2string(w, precision=5)}{sigma}")

    def split(nu, be, eps=1e-6, ball=0.05):
        """The unstable coordinate of the unstable manifold when it first
        comes back within `ball` of the origin: zero on a homoclinic orbit, up
        to the curvature of the stable manifold inside the ball."""
        _, _, v, l = spec(nu, be)
        leave = lambda t, u, *_: np.linalg.norm(u) - ball
        leave.direction, leave.terminal = 1, True
        kw = dict(args=(nu, be), rtol=1e-11, atol=1e-14, method="DOP853")
        s1 = solve_ivp(f, [0, 400], eps * v, events=leave, **kw)
        s2 = solve_ivp(f, [0, 60], s1.y[:, -1], dense_output=True, **kw)
        t = np.linspace(0, 60, 60001)
        U = s2.sol(t)
        n = np.linalg.norm(U, axis=0)
        if not (n > 0.3).any():
            return None
        far = np.argmax(n > 0.3)
        if not (n[far:] < ball).any():
            return None
        k = far + np.argmax(n[far:] < ball)
        return l @ U[:, k], t[k]

    for be, lo, hi, pts in ((0.6, -0.7240, -0.7190, 26), (0.4535645, -0.7300, -0.7200, 41)):
        nus = np.linspace(lo, hi, pts)
        res = [split(n, be) for n in nus]
        for i in range(pts - 1):
            if res[i] and res[i + 1] and res[i][0] * res[i + 1][0] < 0:
                a, b, sa = nus[i], nus[i + 1], res[i][0]
                for _ in range(40):
                    mid = 0.5 * (a + b)
                    sm = split(mid, be)
                    if sm is None:
                        break
                    a, b = (mid, b) if sm[0] * sa > 0 else (a, mid)
                print(f"beta={be}: homoclinic by shooting at nu = {0.5 * (a + b):.8f}, return time {sm[1]:.2f}")


if __name__ == "__main__":
    bykov()
    lorenz84()
    predator_prey_map()
    delayed_logistic()
    freire()
