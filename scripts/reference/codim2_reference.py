"""Recompute the reference values of docs/codim2-reference.md.

Each point is found by solving its defining equations directly (Newton from
the published value, a closed form, or shooting), never by continuation, so
the result is independent of the code under test.

    uv run --with numpy --with scipy --with sympy python scripts/reference/codim2_reference.py

With --mock-curves PATH it also follows the CO oxidation fold and Hopf curves
by pseudo-arclength continuation, samples the predator–prey map's curves from
their closed forms, and writes them as the browser preview's mock data.
"""

import argparse
import json

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
    i = np.argmin(np.where(w.imag > 0, abs(w.real), np.inf))  # the critical pair, nearest the axis
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
    pts = {}
    for key, (lab, u0) in zip(("LP1", "LP2"), [
        ("LP  (published Q2=1.042049)", [0.024717, 0.450257, 0.375018, 1.042049]),
        ("LP  (published Q2=1.052200)", [0.054030, 0.302241, 0.459807, 1.052200]),
    ]):
        u = solve(lambda v: [*at(v)[0], np.linalg.det(at(v)[1])], u0, lab, lambda v: f"|a|={abs(fold_a(m, [*v, 0.4])):.6e}")
        pts[key] = [*u, 0.4]
    for key, (lab, u0) in zip(("H1", "H2"), [
        ("H   (published Q2=1.051557)", [0.016358, 0.523971, 0.328337, 1.051557]),
        ("H   (published Q2=1.040991)", [0.077929, 0.233063, 0.492149, 1.040991]),
    ]):
        pts[key] = [*solve(
            lambda v: [*at(v)[0], hopf(at(v)[1])],
            u0,
            lab,
            lambda v: "l1=%.6e omega=%.6f l1*omega=%.6e" % (*l1(m, [*v, 0.4]), np.prod(l1(m, [*v, 0.4]))),
        ), 0.4]

    def bt(v):  # double zero eigenvalue: c₀ = c₁ = 0
        F, A, *_ = parts(m, v)
        c = np.poly(A)
        return [*F, c[3], c[2]]

    for key, (lab, u0) in zip(("BT1", "BT2"), [
        ("BT  (published 1.417628, 0.971397)", [0.115909, 0.315467, 0.288437, 1.417628, 0.971397]),
        ("BT  (published 1.161199, 0.722339)", [0.016337, 0.638410, 0.200456, 1.161199, 0.722339]),
    ]):
        pts[key] = solve(bt, u0, lab)
    pts["CP"] = solve(
        lambda v: [*parts(m, v)[0], np.linalg.det(parts(m, v)[1]), fold_a(m, v)],
        [0.035940, 0.352008, 0.451368, 1.006408, 0.355991],
        "CP  (published 1.006408, 0.355991)",
    )
    for key, (lab, u0) in zip(("GH1", "GH2"), [
        ("GH  (published 0.891319, 0.232487)", [0.018022, 0.368238, 0.497968, 0.891319, 0.232487]),
        ("GH  (published 0.924255, 0.305879)", [0.064311, 0.211095, 0.554870, 0.924255, 0.305879]),
    ]):
        pts[key] = solve(lambda v: [*parts(m, v)[0], hopf(parts(m, v)[1]), l1(m, v)[0]], u0, lab, lambda v: "omega=%.6f" % l1(m, v)[1])
    # At K = 2 the Hopf test c₀ = c₁c₂ holds with c₁ < 0: eigenvalues ±√(−c₁), a neutral saddle.
    at2 = lambda v: parts(m, [*v, 2.0])
    for lab, u0 in [
        ("K=2 neutral (published Q2=1.271108)", [0.019639, 0.726320, 0.084680, 1.271108]),
        ("K=2 neutral (published Q2=1.640254)", [0.128699, 0.384105, 0.162399, 1.640254]),
    ]:
        solve(
            lambda v: [*at2(v)[0], hopf(at2(v)[1])],
            u0,
            lab,
            lambda v: "Q2=%.10f c1=%.6f eig=%s" % (v[3], np.poly(at2(v)[1])[2], np.sort(np.linalg.eigvals(at2(v)[1]).real)),
        )
    return m, hopf, {k: np.array(v, float) for k, v in pts.items()}


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

    # λ⁴ + p₁λ³ + p₂λ² + p₃λ + p₄ has a pair ±iω where p₁p₂p₃ − p₃² − p₁²p₄ = 0 (Routh–Hurwitz).
    def hopf(A):
        _, p1, p2, p3, p4 = np.poly(A)
        return p1 * p2 * p3 - p3**2 - p1**2 * p4

    def arclength(G, u0, u1, h, n):
        """Pseudo-arclength with a secant predictor: G(u) has len(u) − 1 equations."""
        path = [np.array(u0, float), np.array(u1, float)]
        for _ in range(n):
            t = path[-1] - path[-2]
            t /= np.linalg.norm(t)
            pred = path[-1] + h * t
            path.append(fsolve(lambda u: [*G(u), t @ (u - pred)], pred, xtol=1e-13))
        return path

    def crossings(path, f):
        return [a for a, b in zip(path, path[1:]) if f(a) * f(b) < 0]

    def solved(g, u0):
        """The root of g near u0, or None where fsolve does not converge (a pole of the test)."""
        u = fsolve(g, u0, xtol=1e-14)
        return u if np.abs(g(u)).max() < 1e-10 else None

    # The branch at T = 0.04 by arclength from F = 3, past its fold: the three Hopf points.
    G = lambda u: parts(m, [*u, 0.04])[0]
    u0 = np.array([*v[:0], *fsolve(lambda q: parts(m, [*q, 3.0, 0.04])[0], [2.9787, -0.0387, 0.0582, -0.0211], xtol=1e-13), 3.0])
    u1 = np.array([*fsolve(lambda q: parts(m, [*q, 2.999, 0.04])[0], u0[:4], xtol=1e-13), 2.999])
    branch = arclength(G, u0, u1, 0.003, 1300)
    on_hopf = lambda u, Tv: [*parts(m, [*u, Tv])[0], hopf(parts(m, [*u, Tv])[1])]
    hopfs = [fsolve(lambda u: on_hopf(u, 0.04), a, xtol=1e-13) for a in crossings(branch, lambda u: hopf(parts(m, [*u, 0.04])[1]))]
    for h_ in hopfs:
        print(f"H at T=0.04: F={h_[4]:.8f} (published 2.85996783, 2.46723305, 1.61975642)")
    # Each Hopf curve in (x, F, T) from T = 0.04 both ways. HH: the other pair on the axis too,
    # where p₁ = p₃ = 0. GH: ℓ₁ = 0 where the critical pair is a Hopf pair.
    poly = lambda u: np.poly(parts(m, list(u))[1])
    for h_ in hopfs:
        hc = lambda u: on_hopf(u[:5], u[5])
        start = np.array([*h_, 0.04])
        for dT in (1e-4, -1e-4):
            u1 = np.array([*fsolve(lambda u: on_hopf(u, 0.04 + dT), h_, xtol=1e-13), 0.04 + dT])
            path = [u for u in arclength(hc, start, u1, 2e-3, 600) if -0.1 < u[5] < 0.2 and 0 < u[4] < 4]
            for a in crossings(path, lambda u: poly(u)[1]):
                u = solved(lambda u: [*parts(m, list(u))[0], poly(u)[1], poly(u)[3]], a)
                if u is None:
                    continue
                ev = np.sort_complex(np.linalg.eigvals(parts(m, list(u))[1]))
                print(f"HH: F={u[4]:.10f} T={u[5]:.10f} x={np.array2string(u[:4], precision=8)} eig={np.round(ev, 6)} (published T=0.02627340)")
            for a in crossings(path, lambda u: l1(m, list(u))[0]):
                u = solved(lambda u: [*on_hopf(u[:5], u[5]), l1(m, list(u))[0]], a)
                if u is None:
                    continue
                print(f"GH: F={u[4]:.10f} T={u[5]:.10f} x={np.array2string(u[:4], precision=8)} omega={l1(m, list(u))[1]:.7f} (published 2.3763590350, 0.0501975132)")
    # ZH on the fold curve: p₄ = 0 and the cubic λ³ + p₁λ² + p₂λ + p₃ has ±iω, p₃ = p₁p₂.
    fc = lambda u: [*parts(m, list(u))[0], np.linalg.det(parts(m, list(u))[1])]
    start = np.array([*w, 0.022])
    u1 = np.array([*fsolve(lambda q: fold(q, 0.0219), w, xtol=1e-13), 0.0219])
    path = [u for u in arclength(fc, start, u1, 2e-3, 1500) if -0.05 < u[5] < 0.05]
    for a in crossings(path, lambda u: poly(u)[3] - poly(u)[1] * poly(u)[2]):
        u = solved(lambda u: [*fc(u), poly(u)[3] - poly(u)[1] * poly(u)[2]], a)
        if u is None:
            continue
        ev = np.sort_complex(np.linalg.eigvals(parts(m, list(u))[1]))
        print(f"ZH: F={u[4]:.10f} T={u[5]:.10f} x={np.array2string(u[:4], precision=8)} eig={np.round(ev, 6)} (published T=0.00012644, mirror -0.00012655)")


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
    # On it F3 = F2 = ((a − 1)/a, 0): a curve of branch points. [g_x | g_a] and [g_x | g_ε]
    # have rank 1 there (every 2 × 2 minor vanishes), so the fold system is singular.
    a_bp = a_p1[0]
    assert sp.simplify(ys.subs(a, a_bp)) == 0 and sp.simplify(xs - (a_bp - 1) / a_bp) == 0
    G = g - sp.Matrix([x, y])
    on_bp = {x: xs, y: 0, a: a_bp}
    for par in (a, e):
        M = G.jacobian([x, y]).row_join(G.diff(par)).subs(on_bp)
        assert all(sp.simplify(M.extract([0, 1], list(c)).det()) == 0 for c in [(0, 1), (0, 2), (1, 2)])
    print("BP curve (F3 = F2): a =", a_bp, ", x =", xs, ", y = 0, multipliers", [sp.simplify(m_) for m_ in J.subs(a, a_bp).eigenvals()])
    print("  box a in [1, 4], eps in [-2, 2.4]: a = 4 at eps =", sp.solve(sp.Eq(a_bp, 4), e), "; eps = -2 at a =", a_bp.subs(e, -2))
    for so in sp.solve([sp.Eq(a, a_bp), sp.Eq(1 + tr + dt, 0)], [a, e], dict=True):
        print(f"LPPD (BP curve meets the flip curve): eps = {so[e]}, a = {so[a]}, F3 = ({xs.subs(so)}, {ys.subs(so)}), multipliers {list(J.subs(so).eigenvals())}")
    pts = {}
    for name, trv in [("R1", 2), ("R2", -2), ("R3", -1), ("R4", 0)]:
        for so in sp.solve([sp.Eq(dt, 1), sp.Eq(tr, trv)], [a, e], dict=True):
            pts[name] = (float(so[e]), float(so[a]))
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
    pts["CH"] = (ev, av)
    return pts


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


# ---------------------------------------------------------------------------
# Mock curves for the browser preview (app/src/api/mockCurves.generated.json).
# Each curve is a polyline over (across, up) with the state at each point; the
# boxes are the gallery templates' slider ranges.

CO_BOX = ((0.5, 2.5), (0.0, 2.0))  # k₂, K
PP_BOX = ((0.0, 16.0), (-6.0, 3.0))  # a, ε


def inside(box, p1, p2):
    return box[0][0] <= p1 <= box[0][1] and box[1][0] <= p2 <= box[1][1]


def follow_curve(G, u0, box, ds_max=4e-3):
    """Pseudo-arclength continuation of G(u) = 0, u = (state, p₁, p₂), both ways
    from u0, each way up to and including its first point outside `box`."""
    k = len(u0)

    def DG(u, h=1e-7):
        return np.array([(G(u + h * e) - G(u - h * e)) / (2 * h) for e in np.eye(k)]).T

    def tangent(u, prev):
        t = np.linalg.svd(DG(u))[2][-1]
        return -t if t @ prev < 0 else t

    def correct(u, t, ds):
        v = u + ds * t
        for _ in range(12):
            dv = np.linalg.solve(np.vstack([DG(v), t]), -np.array([*G(v), t @ (v - u) - ds]))
            v = v + dv
            if np.linalg.norm(dv) < 1e-12:
                return v
        return v if np.linalg.norm(G(v)) < 1e-9 else None

    u = np.array(u0, float)
    t0 = np.linalg.svd(DG(u))[2][-1]
    halves = []
    for sign in (-1, 1):
        u, t, ds, out = np.array(u0, float), sign * t0, 2e-3, [np.array(u0, float)]
        while inside(box, *u[-2:]):
            v = correct(u, t, ds)
            if v is None:
                ds /= 2
                assert ds > 1e-8, f"stalled at {u}"
                continue
            t, u = tangent(v, t), v
            out.append(u)
            ds = min(1.3 * ds, ds_max)
        halves.append(out)
    return halves[0][::-1] + halves[1][1:]


def distance_to(poly, q, box):
    """Distance from q to the polyline over (p₁, p₂), both scaled by the box."""
    w = np.array([box[0][1] - box[0][0], box[1][1] - box[1][0]])
    P, q = poly / w, np.asarray(q) / w
    a, b = P[:-1], P[1:]
    d = b - a
    t = np.clip(np.einsum("ij,ij->i", q - a, d) / np.maximum(np.einsum("ij,ij->i", d, d), 1e-300), 0, 1)
    return float(np.min(np.linalg.norm(a + t[:, None] * d - q, axis=1)))


def curve_json(kind, p1, p2, x, neutral, special, box, stops=("range", "range")):
    poly = np.column_stack([p1, p2])
    for sp_ in special:
        gap = distance_to(poly, sp_["p"], box)
        print(f"  {kind:14s} {sp_['kind']:15s} at {np.round(sp_['p'], 8)}: {gap:.1e} from the polyline")
        assert gap < 1e-5, "a reference point is off its curve"
    r = lambda v: float(f"{v:.8g}")
    return {
        "kind": kind,
        "period": 1,
        "p1": [r(v) for v in p1],
        "p2": [r(v) for v in p2],
        "x": [r(v) for v in np.ravel(x)],
        "neutral": [bool(v) for v in neutral],
        "special": [{"kind": q["kind"], "p": [r(v) for v in q["p"]], "x": [r(v) for v in q["x"]]} for q in special],
        "stops": list(stops),
    }


def resample(p1, p2, box, x=None, h=5e-3):
    """Indices of a dense polyline at about every h of arclength, the parameters
    in the box's scale and the state x as it is, with its ends."""
    w = (box[0][1] - box[0][0], box[1][1] - box[1][0])
    steps = [np.diff(p1) / w[0], np.diff(p2) / w[1], *(np.diff(np.zeros((len(p1), 0)) if x is None else x, axis=0).T)]
    s = np.concatenate([[0], np.cumsum(np.sqrt(np.sum(np.square(steps), axis=0)))])
    keep = np.unique(np.searchsorted(s, np.arange(0, s[-1], h)))
    return np.unique(np.concatenate([keep, [len(s) - 1]]))


def co_curves(m, hopf, pts):
    print("\n===== Mock curves: CO oxidation, k₂ across, K up =====")

    def G(test):
        return lambda u: np.array([*np.array(m["F"](*u), float).ravel(), test(np.array(m["J"](*u), float))])

    def point(kind, key):
        u = pts[key]
        return {"kind": kind, "p": u[3:5], "x": u[:3]}

    out = []
    for kind, seed, also, test, special in [
        ("fold", "LP1", "LP2", np.linalg.det, [point("cusp", "CP"), point("bogdanovTakens", "BT1"), point("bogdanovTakens", "BT2")]),
        ("hopf", "H1", "H2", hopf, [point("bogdanovTakens", "BT1"), point("bogdanovTakens", "BT2"), point("bautin", "GH1"), point("bautin", "GH2")]),
    ]:
        U = np.array(follow_curve(G(test), pts[seed], CO_BOX, ds_max=1e-3))
        U = U[resample(U[:, 3], U[:, 4], CO_BOX, U[:, :3])]
        # On the Hopf curve c₀ = c₁c₂ also holds at a neutral saddle λ = ±√(−c₁): there c₁ < 0.
        neutral = [kind == "hopf" and np.poly(np.array(m["J"](*u), float))[2] < 0 for u in U]
        poly = U[:, 3:5]
        print(f"  {kind}: {len(U)} points, ends {np.round(poly[0], 4)} and {np.round(poly[-1], 4)}, x in [{U[:, 0].min():.4f}, {U[:, 0].max():.4f}]")
        gap = distance_to(poly, pts[also][3:5], CO_BOX)
        print(f"  {kind:14s} {also} (K = 0.4) is {gap:.1e} from the curve followed from {seed}")
        assert gap < 1e-5
        out.append(curve_json(kind, U[:, 3], U[:, 4], U[:, :3], neutral, special, CO_BOX))
    return {"free": [0, 1], "curves": out}


def pp_curves(pts):
    print("\n===== Mock curves: predator–prey map, a across, ε up =====")

    def F3(a, e):
        x = 1 / (3.5 - e)
        return x, 3.5 * x * (a * (1 - x) - 1) / 3

    def half_trace(a, e, x, y):
        return (a * (1 - 2 * x) - 3 * y / (1 + e * x) ** 2 + 3.5 * x / (1 + e * x)) / 2

    def piece(e_lo, e_hi, a_of):
        """The run of ε in (e_lo, e_hi) whose (a, ε) is in the box, with the first
        point outside it at each end that leaves the box."""
        e = np.linspace(e_lo, e_hi, 400001)
        a = a_of(e)
        ok = (a >= PP_BOX[0][0]) & (a <= PP_BOX[0][1]) & (e >= PP_BOX[1][0]) & (e <= PP_BOX[1][1])
        i = np.flatnonzero(ok)
        assert len(i) and np.all(np.diff(i) == 1), "one run per piece"
        lo, hi = max(i[0] - 1, 0), min(i[-1] + 1, len(e) - 1)
        e, a = e[lo : hi + 1], a[lo : hi + 1]
        k = resample(a, e, PP_BOX)
        return a[k], e[k]

    def special(kind, key):
        e, a = pts[key]
        return {"kind": kind, "p": (a, e), "x": F3(a, e)}

    def on_f3(kind, a, e, neutral, sp_, stops=("range", "range")):
        x, y = F3(a, e)
        return curve_json(kind, a, e, np.column_stack([x, y]), neutral, sp_, PP_BOX, stops)

    a_ns = lambda e: (2 * e - 7) / (2 * e - 3)
    a_pd = lambda e: (4 * e**2 - 56 * e + 147) / (4 * e**2 + 4 * e - 7)
    pole = (-1 + 2 * np.sqrt(2)) / 2, (-1 - 2 * np.sqrt(2)) / 2
    lppd = {"kind": "foldFlip", "p": (3.0, 2.0), "x": (2 / 3, 0.0)}
    out = []
    a, e = piece(-6.05, 1.49, a_ns)
    # On det = 1 the multipliers are e^{±iθ} with cos θ = tr/2; past |tr/2| = 1 they are real, μ and 1/μ.
    neutral = np.abs(half_trace(a, e, *F3(a, e))) > 1
    out.append(on_f3("neimarkSacker", a, e, neutral, [special("resonance4", "R4"), special("resonance3", "R3"), special("resonance2", "R2"), special("chenciner", "CH")]))
    a, e = piece(pole[0] + 1e-6, 3.05, a_pd)
    out.append(on_f3("flip", a, e, np.zeros(len(a), bool), [special("resonance2", "R2"), lppd]))
    a, e = piece(-6.05, pole[1] - 1e-6, a_pd)
    out.append(on_f3("flip", a, e, np.zeros(len(a), bool), []))
    # F2 = ((a − 1)/a, 0) has the multiplier 2 − a, so it flips on the line a = 3 whatever ε is.
    # The engine refuses that seed (ε doesn't move the flip), so the preview has no curve there.
    # F2's other multiplier, 3.5x/(1 + εx), is −1 where x = −1/(ε + 7/2), so a = (ε + 7/2)/(ε + 9/2).
    # For ε > −7/2 that F2 has x < 0 and a → 0 as ε → −7/2, the corner of the box, only as
    # x → −∞: a follower runs out of points there before it reaches the box's edge.
    # Its flip coefficient vanishes at x = −(2√2 + 1)/7, a generalized flip (derived in
    # crates/dyn-analysis/tests/curves.rs). Where it crosses a = 3, at ε = −5, both multipliers
    # are −1 and D f = [[−1, 6/7], [0, −1]] is a Jordan block: a 1:2 resonance.
    a_f2 = lambda e: (e + 3.5) / (e + 4.5)
    r2 = np.sqrt(2)
    gpd = {"kind": "generalizedFlip", "p": (1 - r2 / 4, 2 * r2 - 4.5), "x": (-(2 * r2 + 1) / 7, 0.0)}
    r2_f2 = {"kind": "resonance2", "p": (3.0, -5.0), "x": (2 / 3, 0.0)}
    for e_lo, e_hi, stops, sp_ in [
        (-3.5 + 1e-4, 3.05, ("maxPoints", "range"), [gpd]),
        (-6.05, -4.5 - 1e-6, ("range", "range"), [r2_f2]),
    ]:
        a, e = piece(e_lo, e_hi, a_f2)
        x = np.column_stack([1 - 1 / a, np.zeros_like(a)])
        out.append(curve_json("flip", a, e, x, np.zeros(len(a), bool), sp_, PP_BOX, stops))
    # F3 = F2 where F3's y is 0, at a = (2ε − 7)/(2ε − 5): a curve of branch points. It is never
    # neutral, and past its pole at ε = 5/2 it has a < 0.
    a, e = piece(-6.05, 2.5 - 1e-6, lambda e: (2 * e - 7) / (2 * e - 5))
    x = np.column_stack([1 / (3.5 - e), np.zeros_like(e)])
    out.append(curve_json("branchPoint", a, e, x, np.zeros(len(a), bool), [lppd], PP_BOX))
    for c in out:
        print(f"  {c['kind']}: {len(c['p1'])} points")
    return {"free": [0, 1], "curves": out}


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--mock-curves", metavar="PATH", help="also write the preview's mock curves as JSON to PATH")
    path = ap.parse_args().mock_curves
    co = bykov()
    lorenz84()
    pp = predator_prey_map()
    delayed_logistic()
    freire()
    if path:
        data = {"co-oxidation": co_curves(*co), "predator-prey-map": pp_curves(pp)}
        with open(path, "w") as f:
            json.dump(data, f, separators=(",", ":"))
            f.write("\n")
        print(f"\nwrote {path}")
