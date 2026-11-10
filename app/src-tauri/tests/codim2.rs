//! The codimension-2 gate: the points of docs/codim2-reference.md, systems 1
//! to 3, located on curves the way the Parameter plane follows them. Each
//! curve starts from a special point of a Branch view run, seeded as
//! `planeSeed` (app/src/state/curves.ts) seeds it, and is followed through
//! `service::compute_curve` with the requests the plane sends.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use separatrix_lib::ipc::{
    BranchOut, Codim2KindOut, CompileRequest, ContinuationDirection, ContinuationRequest,
    ContinuationStart, CurveKindOut, CurveOut, CurveRequest, CurveSeed, InputFormat, NormalFormOut,
    Severity, SpecialKindOut,
};
use separatrix_lib::service::{self, SystemStore};
use serde::Deserialize;

const TEMPLATES_JSON: &str = include_str!("../../src/templates/templates.json");

/// The Parameter plane's request (app/src/state/curves.ts): `ds`,
/// `dsMax = 2·ds`, `maxPoints` in each direction, direction `both`.
const UI_DS: f64 = 0.01;
const UI_MAX_POINTS: usize = 2000;

/// The Branch view's request (app/src/state/model.ts): `ds` a hundredth of
/// the range to one digit, `dsMax = 2·ds`, `maxPoints`, direction `both`.
const BRANCH_MAX_POINTS: usize = 500;

type Box2 = ((f64, f64), (f64, f64));

enum Source {
    Template(&'static str),
    Text(&'static str),
}

/// A Branch view run, which seeds the Parameter plane.
struct Branch {
    label: &'static str,
    source: Source,
    /// Every parameter's slider value.
    params: &'static [(&'static str, f64)],
    /// The parameter swept, drawn across in the plane.
    param: &'static str,
    /// The slider's range, which the Branch view sweeps.
    range: (f64, f64),
    /// A fixed point in view, where the branch starts.
    x0: &'static [f64],
}

/// A curve of the Parameter plane: from `branch`'s special point of `from`
/// nearest `near`, with `up` drawn up, in the box `ranges`.
struct Run {
    label: &'static str,
    branch: &'static Branch,
    from: SpecialKindOut,
    near: f64,
    up: &'static str,
    ranges: Box2,
}

const CO: Branch = Branch {
    label: "CO oxidation, K = 0.4",
    source: Source::Template("co-oxidation"),
    params: &[("k_2", 1.05), ("K", 0.4)],
    param: "k_2",
    range: (0.5, 2.5),
    x0: &[0.02, 0.5, 0.33],
};

const CO_BOX: Box2 = ((0.5, 2.5), (0.0, 2.0));

const CO_FOLD: Run = Run {
    label: "CO fold",
    branch: &CO,
    from: SpecialKindOut::Fold,
    near: 1.042,
    up: "K",
    ranges: CO_BOX,
};

const CO_HOPF: Run = Run {
    label: "CO Hopf",
    from: SpecialKindOut::Hopf,
    near: 1.0516,
    ..CO_FOLD
};

const LORENZ: Branch = Branch {
    label: "Lorenz-84, T = 0.04",
    source: Source::Text(
        "X' = -Y^2 - Z^2 - 0.25*X + 0.25*F - 0.987*U^2\n\
         Y' = X*Y - X*Z - Y + 0.25\n\
         Z' = X*Y + X*Z - Z\n\
         U' = -1.04*U + 0.987*U*X + T",
    ),
    params: &[("F", 3.0), ("T", 0.04)],
    param: "F",
    range: (0.0, 10.0),
    x0: &[
        2.978_700_439_495_334_3,
        -0.038_683_025_033_937_52,
        0.058_232_737_694_740_085,
        -0.021_052_882_731_174_59,
    ],
};

/// The sliders the interface gives F = 3 and T = 0.04, [0, 10] and
/// [0, 0.1], with T's widened below 0 to hold the mirror points.
const LORENZ_BOX: Box2 = ((0.0, 10.0), (-0.1, 0.1));

const LORENZ_FOLD: Run = Run {
    label: "Lorenz fold",
    branch: &LORENZ,
    from: SpecialKindOut::Fold,
    near: 1.5466,
    up: "T",
    ranges: LORENZ_BOX,
};

/// Through the two slower Hopf points of the branch.
const LORENZ_HOPF: Run = Run {
    label: "Lorenz Hopf, slow",
    from: SpecialKindOut::Hopf,
    near: 1.6197,
    ..LORENZ_FOLD
};

const LORENZ_FAST: Run = Run {
    label: "Lorenz Hopf, fast",
    from: SpecialKindOut::Hopf,
    near: 2.8599,
    ..LORENZ_FOLD
};

/// F3 = (1/(3.5 − ε), 3.5x(a(1 − x) − 1)/3) at a = 5.3.
const PP: Branch = Branch {
    label: "predator–prey, ε = 1, F3",
    source: Source::Template("predator-prey-map"),
    params: &[("a", 5.3), ("varepsilon", 1.0)],
    param: "a",
    range: (0.0, 16.0),
    x0: &[0.4, 1.017_333_333_333_333_4],
};

/// F3's flip is at a = 95 when ε = 1, past the slider, so the Branch view
/// finds it with ε moved to 1.5, at a = 9.
const PP_TILTED: Branch = Branch {
    label: "predator–prey, ε = 1.5, F3",
    params: &[("a", 5.3), ("varepsilon", 1.5)],
    x0: &[0.5, 0.9625],
    ..PP
};

/// F2 = ((a − 1)/a, 0), whose `y` multiplier is −1 at a = 9/11.
const PP_F2: Branch = Branch {
    label: "predator–prey, ε = 1, F2",
    x0: &[0.811_320_754_716_981_1, 0.0],
    ..PP
};

/// F2's other flip curve, below ε = −4.5, where the Branch view finds its
/// `y` multiplier at −1 at a = 2.
const PP_F2_LOW: Branch = Branch {
    label: "predator–prey, ε = −5.5, F2",
    params: &[("a", 5.3), ("varepsilon", -5.5)],
    ..PP_F2
};

const PP_BOX: Box2 = ((0.0, 16.0), (-6.0, 3.0));

const PP_NS: Run = Run {
    label: "PP Neimark–Sacker",
    branch: &PP,
    from: SpecialKindOut::NeimarkSacker,
    near: 5.0,
    up: "varepsilon",
    ranges: PP_BOX,
};

const PP_FLIP: Run = Run {
    label: "PP flip of F3",
    branch: &PP_TILTED,
    from: SpecialKindOut::Flip,
    near: 9.0,
    ..PP_NS
};

const PP_BP: Run = Run {
    label: "PP branch point",
    from: SpecialKindOut::BranchPoint,
    near: 5.0 / 3.0,
    ..PP_NS
};

const PP_F2_FLIP: Run = Run {
    label: "PP flip of F2",
    branch: &PP_F2,
    from: SpecialKindOut::Flip,
    near: 9.0 / 11.0,
    ..PP_NS
};

const PP_F2_FLIP_LOW: Run = Run {
    label: "PP flip of F2, low",
    branch: &PP_F2_LOW,
    near: 2.0,
    ..PP_F2_FLIP
};

const RUNS: &[&Run] = &[
    &CO_FOLD,
    &CO_HOPF,
    &LORENZ_FOLD,
    &LORENZ_HOPF,
    &LORENZ_FAST,
    &PP_NS,
    &PP_FLIP,
    &PP_BP,
    &PP_F2_FLIP,
    &PP_F2_FLIP_LOW,
];

const PP_RUNS: &[&Run] = &[&PP_NS, &PP_FLIP, &PP_BP, &PP_F2_FLIP, &PP_F2_FLIP_LOW];

enum Measure {
    /// Parameter `axis` (0 across, 1 up) of the point of `kind` nearest
    /// `near`.
    P {
        run: &'static Run,
        kind: Codim2KindOut,
        near: (f64, f64),
        axis: usize,
    },
    /// The larger difference in a parameter between the points of `kind`
    /// nearest `near` on two curves.
    Agree {
        runs: [&'static Run; 2],
        kind: Codim2KindOut,
        near: (f64, f64),
    },
    /// How many points the curve has.
    Count(&'static Run),
    /// How many points of `kinds` lie on a neutral stretch: the point, or a
    /// point beside it, is a neutral saddle.
    OnNeutral {
        run: &'static Run,
        kinds: &'static [Codim2KindOut],
    },
    /// On how many of `runs` of kind `curve` a point of `kind` lies within
    /// `REF_TOL` of `near` in both parameters.
    On {
        runs: &'static [&'static Run],
        curve: CurveKindOut,
        kind: Codim2KindOut,
        near: (f64, f64),
    },
    /// How many points of every run are not at their own vertex: a wrong
    /// `index` after the halves are joined.
    Misplaced,
    /// A normal-form coefficient of the point of `kind` nearest `near`.
    Coefficient {
        run: &'static Run,
        kind: Codim2KindOut,
        near: (f64, f64),
        which: Coefficient,
    },
}

/// What of a normal form is compared: `a` and `b` flip with `q₀`, so only
/// `|a|`, `|b|` and the sign of `ab` (brief §6). The cusp's `c` does not.
#[derive(Clone, Copy)]
enum Coefficient {
    BtA,
    BtB,
    BtSignAb,
    CuspC,
}

enum Expect {
    Near { value: f64, tol: f64 },
    AtMost { bound: f64 },
    Exactly { count: usize },
}

struct Check {
    text: &'static str,
    measure: Measure,
    expect: Expect,
}

/// Six digits where the reference page gives eight.
const REF_TOL: f64 = 1e-6;
/// The cusp, the least well conditioned, to five.
const CUSP_TOL: f64 = 1e-5;
/// MatCont's coefficients come from finite differences: against the
/// reference script's exact derivatives its Bogdanov–Takens `a` is 2e-8 off,
/// its `b` 6e-7 and its cusp `c` 4e-6.
const BT_A_TOL: f64 = 1e-7;
const BT_B_TOL: f64 = 1e-6;
const CUSP_C_TOL: f64 = 1e-5;
/// One point found on two curves: each is within Newton's tolerance
/// `1e-10 · (1 + |z|∞)` of it in the box's scale, and the boxes are at most
/// 10 wide.
const AGREE_TOL: f64 = 1e-8;

const fn near(value: f64, tol: f64) -> Expect {
    Expect::Near { value, tol }
}

/// Parameter `axis` of the point of `kind` at `at` on `run`, to `tol`.
const fn at(
    text: &'static str,
    run: &'static Run,
    kind: Codim2KindOut,
    at: (f64, f64),
    axis: usize,
    tol: f64,
) -> Check {
    Check {
        text,
        measure: Measure::P {
            run,
            kind,
            near: at,
            axis,
        },
        expect: near(if axis == 0 { at.0 } else { at.1 }, tol),
    }
}

const fn agree(
    text: &'static str,
    runs: [&'static Run; 2],
    kind: Codim2KindOut,
    at: (f64, f64),
) -> Check {
    Check {
        text,
        measure: Measure::Agree {
            runs,
            kind,
            near: at,
        },
        expect: Expect::AtMost { bound: AGREE_TOL },
    }
}

const fn count(text: &'static str, run: &'static Run, count: usize) -> Check {
    Check {
        text,
        measure: Measure::Count(run),
        expect: Expect::Exactly { count },
    }
}

const fn on_neutral(
    text: &'static str,
    run: &'static Run,
    kinds: &'static [Codim2KindOut],
    count: usize,
) -> Check {
    Check {
        text,
        measure: Measure::OnNeutral { run, kinds },
        expect: Expect::Exactly { count },
    }
}

/// `which` of the normal form of the point of `kind` at `at` on `run`.
const fn coefficient(
    text: &'static str,
    run: &'static Run,
    (kind, at): (Codim2KindOut, (f64, f64)),
    which: Coefficient,
    expect: Expect,
) -> Check {
    Check {
        text,
        measure: Measure::Coefficient {
            run,
            kind,
            near: at,
            which,
        },
        expect,
    }
}

/// `|a|`, `|b|` and the sign of `ab` at a Bogdanov–Takens point, as MatCont
/// prints them for CO oxidation.
const fn bt_rows(
    texts: [&'static str; 3],
    run: &'static Run,
    at: (f64, f64),
    (a, b): (f64, f64),
) -> [Check; 3] {
    [
        coefficient(
            texts[0],
            run,
            (BogdanovTakens, at),
            Coefficient::BtA,
            near(a, BT_A_TOL),
        ),
        coefficient(
            texts[1],
            run,
            (BogdanovTakens, at),
            Coefficient::BtB,
            near(b, BT_B_TOL),
        ),
        coefficient(
            texts[2],
            run,
            (BogdanovTakens, at),
            Coefficient::BtSignAb,
            near(1.0, 0.0),
        ),
    ]
}

/// On how many of the predator–prey map's curves of kind `curve` the
/// fold-flip lies.
const fn lppd_on(text: &'static str, curve: CurveKindOut, count: usize) -> Check {
    Check {
        text,
        measure: Measure::On {
            runs: PP_RUNS,
            curve,
            kind: FoldFlip,
            near: PP_LPPD,
        },
        expect: Expect::Exactly { count },
    }
}

use Codim2KindOut::{
    Bautin, BogdanovTakens, Chenciner, Cusp, FoldFlip, GeneralizedFlip, HopfHopf, Resonance1,
    Resonance2, Resonance3, Resonance4, ZeroHopf,
};

/// Every kind but those that begin a neutral stretch.
const NOT_ON_NEUTRAL: &[Codim2KindOut] = &[
    Cusp,
    Bautin,
    HopfHopf,
    Resonance3,
    Resonance4,
    Chenciner,
    FoldFlip,
    GeneralizedFlip,
];

const CO_BT1: (f64, f64) = (1.41762764, 0.97139757);
const CO_BT2: (f64, f64) = (1.16119871, 0.72233907);
const CO_CP: (f64, f64) = (1.00640833, 0.35599127);
const CO_GH1: (f64, f64) = (0.89131898, 0.23248735);
const CO_GH2: (f64, f64) = (0.92425502, 0.30587927);
/// `(|a|, |b|)` at the two BT points and the cusp's `c`, from MatCont
/// Tutorial IV: `(a, b) = (−0.08378442, −2.136280)`, `(−0.04822563, −1.937632)`.
const CO_BT1_AB: (f64, f64) = (0.08378442, 2.136280);
const CO_BT2_AB: (f64, f64) = (0.04822563, 1.937632);
const CO_CP_C: f64 = 0.3627844;

const CO_BT1_FOLD: [Check; 3] = bt_rows(
    [
        "CO fold: BT₁ |a|",
        "CO fold: BT₁ |b|",
        "CO fold: BT₁ sign(ab)",
    ],
    &CO_FOLD,
    CO_BT1,
    CO_BT1_AB,
);
const CO_BT2_FOLD: [Check; 3] = bt_rows(
    [
        "CO fold: BT₂ |a|",
        "CO fold: BT₂ |b|",
        "CO fold: BT₂ sign(ab)",
    ],
    &CO_FOLD,
    CO_BT2,
    CO_BT2_AB,
);
const CO_BT1_HOPF: [Check; 3] = bt_rows(
    [
        "CO Hopf: BT₁ |a|",
        "CO Hopf: BT₁ |b|",
        "CO Hopf: BT₁ sign(ab)",
    ],
    &CO_HOPF,
    CO_BT1,
    CO_BT1_AB,
);
const CO_BT2_HOPF: [Check; 3] = bt_rows(
    [
        "CO Hopf: BT₂ |a|",
        "CO Hopf: BT₂ |b|",
        "CO Hopf: BT₂ sign(ab)",
    ],
    &CO_HOPF,
    CO_BT2,
    CO_BT2_AB,
);
const CO_COEFFICIENTS: &[&[Check]] = &[
    &CO_BT1_FOLD,
    &CO_BT2_FOLD,
    &CO_BT1_HOPF,
    &CO_BT2_HOPF,
    &[coefficient(
        "CO fold: CP c",
        &CO_FOLD,
        (Cusp, CO_CP),
        Coefficient::CuspC,
        near(CO_CP_C, CUSP_C_TOL),
    )],
];

const LORENZ_BT: (f64, f64) = (1.4467167, 0.02094017);
const LORENZ_BT_MIRROR: (f64, f64) = (1.4467167, -0.02094017);
const LORENZ_ZH: (f64, f64) = (1.2834192961, 0.0001265408);
const LORENZ_ZH_MIRROR: (f64, f64) = (1.2834192961, -0.0001265408);
const LORENZ_GH: (f64, f64) = (2.3763600882, 0.0501974320);
const LORENZ_HH: (f64, f64) = (2.5332211070, 0.0262739430);

/// (a, ε) in the plane; the reference page lists (ε, a).
const PP_R4: (f64, f64) = (7.0, 7.0 / 6.0);
const PP_R3: (f64, f64) = (10.5, 49.0 / 38.0);
const PP_R2: (f64, f64) = (14.0, 35.0 / 26.0);
const PP_CH: (f64, f64) = (1.3257299842, -4.6400549446);
const PP_LPPD: (f64, f64) = (3.0, 2.0);
/// F2's generalized flip, derived on the reference page: `a = 1 − √2/4`,
/// `ε = 2√2 − 9/2`.
const PP_GPD: (f64, f64) = (
    1.0 - std::f64::consts::SQRT_2 / 4.0,
    2.0 * std::f64::consts::SQRT_2 - 4.5,
);
/// F2's 1:2 resonance, where its flip curve crosses `a = 3` (reference page).
const PP_R2_F2: (f64, f64) = (3.0, -5.0);

const CHECKS: &[Check] = &[
    at(
        "CO fold: BT₁ k₂",
        &CO_FOLD,
        BogdanovTakens,
        CO_BT1,
        0,
        REF_TOL,
    ),
    at(
        "CO fold: BT₁ K",
        &CO_FOLD,
        BogdanovTakens,
        CO_BT1,
        1,
        REF_TOL,
    ),
    at(
        "CO fold: BT₂ k₂",
        &CO_FOLD,
        BogdanovTakens,
        CO_BT2,
        0,
        REF_TOL,
    ),
    at(
        "CO fold: BT₂ K",
        &CO_FOLD,
        BogdanovTakens,
        CO_BT2,
        1,
        REF_TOL,
    ),
    at("CO fold: CP k₂", &CO_FOLD, Cusp, CO_CP, 0, CUSP_TOL),
    at("CO fold: CP K", &CO_FOLD, Cusp, CO_CP, 1, CUSP_TOL),
    at("CO Hopf: GH₁ k₂", &CO_HOPF, Bautin, CO_GH1, 0, REF_TOL),
    at("CO Hopf: GH₁ K", &CO_HOPF, Bautin, CO_GH1, 1, REF_TOL),
    at("CO Hopf: GH₂ k₂", &CO_HOPF, Bautin, CO_GH2, 0, REF_TOL),
    at("CO Hopf: GH₂ K", &CO_HOPF, Bautin, CO_GH2, 1, REF_TOL),
    agree(
        "CO: BT₁ on fold vs Hopf, max |Δp|",
        [&CO_FOLD, &CO_HOPF],
        BogdanovTakens,
        CO_BT1,
    ),
    agree(
        "CO: BT₂ on fold vs Hopf, max |Δp|",
        [&CO_FOLD, &CO_HOPF],
        BogdanovTakens,
        CO_BT2,
    ),
    count("CO fold: points (2 BT, CP)", &CO_FOLD, 3),
    count("CO Hopf: points (2 BT, 2 GH)", &CO_HOPF, 4),
    on_neutral(
        "CO Hopf: on a neutral stretch, not BT, ZH, R1, R2",
        &CO_HOPF,
        NOT_ON_NEUTRAL,
        0,
    ),
    on_neutral(
        "CO Hopf: BT on a neutral stretch",
        &CO_HOPF,
        &[BogdanovTakens],
        2,
    ),
    at(
        "Lorenz fold: BT F",
        &LORENZ_FOLD,
        BogdanovTakens,
        LORENZ_BT,
        0,
        REF_TOL,
    ),
    at(
        "Lorenz fold: BT T",
        &LORENZ_FOLD,
        BogdanovTakens,
        LORENZ_BT,
        1,
        REF_TOL,
    ),
    at(
        "Lorenz fold: mirror BT F",
        &LORENZ_FOLD,
        BogdanovTakens,
        LORENZ_BT_MIRROR,
        0,
        REF_TOL,
    ),
    at(
        "Lorenz fold: mirror BT T",
        &LORENZ_FOLD,
        BogdanovTakens,
        LORENZ_BT_MIRROR,
        1,
        REF_TOL,
    ),
    at(
        "Lorenz fold: ZH F",
        &LORENZ_FOLD,
        ZeroHopf,
        LORENZ_ZH,
        0,
        REF_TOL,
    ),
    at(
        "Lorenz fold: ZH T",
        &LORENZ_FOLD,
        ZeroHopf,
        LORENZ_ZH,
        1,
        REF_TOL,
    ),
    at(
        "Lorenz fold: mirror ZH F",
        &LORENZ_FOLD,
        ZeroHopf,
        LORENZ_ZH_MIRROR,
        0,
        REF_TOL,
    ),
    at(
        "Lorenz fold: mirror ZH T",
        &LORENZ_FOLD,
        ZeroHopf,
        LORENZ_ZH_MIRROR,
        1,
        REF_TOL,
    ),
    at(
        "Lorenz Hopf: GH F",
        &LORENZ_HOPF,
        Bautin,
        LORENZ_GH,
        0,
        REF_TOL,
    ),
    at(
        "Lorenz Hopf: GH T",
        &LORENZ_HOPF,
        Bautin,
        LORENZ_GH,
        1,
        REF_TOL,
    ),
    at(
        "Lorenz Hopf: HH F",
        &LORENZ_HOPF,
        HopfHopf,
        LORENZ_HH,
        0,
        REF_TOL,
    ),
    at(
        "Lorenz Hopf: HH T",
        &LORENZ_HOPF,
        HopfHopf,
        LORENZ_HH,
        1,
        REF_TOL,
    ),
    agree(
        "Lorenz: BT on fold vs Hopf, max |Δp|",
        [&LORENZ_FOLD, &LORENZ_HOPF],
        BogdanovTakens,
        LORENZ_BT,
    ),
    agree(
        "Lorenz: ZH on fold vs fast Hopf, max |Δp|",
        [&LORENZ_FOLD, &LORENZ_FAST],
        ZeroHopf,
        LORENZ_ZH,
    ),
    agree(
        "Lorenz: HH on both Hopf curves, max |Δp|",
        [&LORENZ_HOPF, &LORENZ_FAST],
        HopfHopf,
        LORENZ_HH,
    ),
    count("Lorenz fold: points (2 BT, 2 ZH, CP)", &LORENZ_FOLD, 5),
    count("Lorenz Hopf: points (2 BT, 2 GH, 2 HH)", &LORENZ_HOPF, 6),
    count("Lorenz fast Hopf: points (2 HH, 2 ZH)", &LORENZ_FAST, 4),
    on_neutral(
        "Lorenz Hopf: on a neutral stretch, not BT, ZH, R1, R2",
        &LORENZ_HOPF,
        NOT_ON_NEUTRAL,
        0,
    ),
    on_neutral(
        "Lorenz Hopf: BT on a neutral stretch",
        &LORENZ_HOPF,
        &[BogdanovTakens],
        2,
    ),
    at("PP NS: R4 a", &PP_NS, Resonance4, PP_R4, 0, REF_TOL),
    at("PP NS: R4 ε", &PP_NS, Resonance4, PP_R4, 1, REF_TOL),
    at("PP NS: R3 a", &PP_NS, Resonance3, PP_R3, 0, REF_TOL),
    at("PP NS: R3 ε", &PP_NS, Resonance3, PP_R3, 1, REF_TOL),
    at("PP NS: R2 a", &PP_NS, Resonance2, PP_R2, 0, REF_TOL),
    at("PP NS: R2 ε", &PP_NS, Resonance2, PP_R2, 1, REF_TOL),
    at("PP NS: CH a", &PP_NS, Chenciner, PP_CH, 0, REF_TOL),
    at("PP NS: CH ε", &PP_NS, Chenciner, PP_CH, 1, REF_TOL),
    agree(
        "PP: R2 on NS vs flip, max |Δp|",
        [&PP_NS, &PP_FLIP],
        Resonance2,
        PP_R2,
    ),
    at("PP BP: LPPD a", &PP_BP, FoldFlip, PP_LPPD, 0, REF_TOL),
    at("PP BP: LPPD ε", &PP_BP, FoldFlip, PP_LPPD, 1, REF_TOL),
    lppd_on("PP: curves of flips with LPPD there", CurveKindOut::Flip, 1),
    lppd_on(
        "PP: curves of branch points with LPPD there",
        CurveKindOut::BranchPoint,
        1,
    ),
    at(
        "PP flip of F2: GPD a (derived)",
        &PP_F2_FLIP,
        GeneralizedFlip,
        PP_GPD,
        0,
        REF_TOL,
    ),
    at(
        "PP flip of F2: GPD ε (derived)",
        &PP_F2_FLIP,
        GeneralizedFlip,
        PP_GPD,
        1,
        REF_TOL,
    ),
    at(
        "PP flip of F2, low: R2 a (derived)",
        &PP_F2_FLIP_LOW,
        Resonance2,
        PP_R2_F2,
        0,
        REF_TOL,
    ),
    at(
        "PP flip of F2, low: R2 ε (derived)",
        &PP_F2_FLIP_LOW,
        Resonance2,
        PP_R2_F2,
        1,
        REF_TOL,
    ),
    count("PP NS: points (R4, R3, R2, CH)", &PP_NS, 4),
    count("PP flip of F3: points (R2, LPPD)", &PP_FLIP, 2),
    count("PP branch point: points (LPPD)", &PP_BP, 1),
    count("PP flip of F2: points (GPD)", &PP_F2_FLIP, 1),
    count("PP flip of F2, low: points (R2)", &PP_F2_FLIP_LOW, 1),
    on_neutral(
        "PP NS: on a neutral stretch, not BT, ZH, R1, R2",
        &PP_NS,
        NOT_ON_NEUTRAL,
        0,
    ),
    on_neutral(
        "PP NS: R1, R2 on a neutral stretch",
        &PP_NS,
        &[Resonance1, Resonance2],
        1,
    ),
    Check {
        text: "every curve: points off their own vertex",
        measure: Measure::Misplaced,
        expect: Expect::Exactly { count: 0 },
    },
];

const WRONG_CHECKS: &[Check] = &[
    Check {
        text: "wrong: CO BT₁ k₂ = 1.41763764 (1e−5 off)",
        measure: Measure::P {
            run: &CO_FOLD,
            kind: BogdanovTakens,
            near: CO_BT1,
            axis: 0,
        },
        expect: near(CO_BT1.0 + 1e-5, REF_TOL),
    },
    Check {
        text: "wrong: CO cusp K = 0.35601127 (2e−5 off)",
        measure: Measure::P {
            run: &CO_FOLD,
            kind: Cusp,
            near: CO_CP,
            axis: 1,
        },
        expect: near(CO_CP.1 + 2e-5, CUSP_TOL),
    },
    Check {
        text: "wrong: PP R3 ε = 49/38 + 1e−5",
        measure: Measure::P {
            run: &PP_NS,
            kind: Resonance3,
            near: PP_R3,
            axis: 1,
        },
        expect: near(PP_R3.1 + 1e-5, REF_TOL),
    },
    Check {
        text: "wrong: Lorenz ZH T = 0.0001285408 (2e−6 off)",
        measure: Measure::P {
            run: &LORENZ_FOLD,
            kind: ZeroHopf,
            near: LORENZ_ZH,
            axis: 1,
        },
        expect: near(LORENZ_ZH.1 + 2e-6, REF_TOL),
    },
    lppd_on(
        "wrong: PP: a fold curve with LPPD there",
        CurveKindOut::Fold,
        1,
    ),
    on_neutral(
        "wrong: CO Hopf: a GH on a neutral stretch",
        &CO_HOPF,
        &[Bautin],
        1,
    ),
    count("wrong: CO fold has no cusp, 2 points", &CO_FOLD, 2),
    coefficient(
        "wrong: CO fold: BT₁ |b| = 2.137280 (1e−3 off)",
        &CO_FOLD,
        (BogdanovTakens, CO_BT1),
        Coefficient::BtB,
        near(CO_BT1_AB.1 + 1e-3, BT_B_TOL),
    ),
    coefficient(
        "wrong: CO Hopf: BT₂ sign(ab) = −1",
        &CO_HOPF,
        (BogdanovTakens, CO_BT2),
        Coefficient::BtSignAb,
        near(-1.0, 0.0),
    ),
    coefficient(
        "wrong: CO fold: CP c = 0.3628844 (1e−4 off)",
        &CO_FOLD,
        (Cusp, CO_CP),
        Coefficient::CuspC,
        near(CO_CP_C + 1e-4, CUSP_C_TOL),
    ),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Template {
    id: String,
    format: InputFormat,
    equations: Vec<String>,
}

fn store() -> &'static SystemStore {
    static STORE: OnceLock<SystemStore> = OnceLock::new();
    STORE.get_or_init(SystemStore::new)
}

struct System {
    id: u32,
    names: Vec<String>,
}

fn system(b: &'static Branch) -> Arc<System> {
    static SYSTEMS: OnceLock<Mutex<HashMap<&'static str, Arc<System>>>> = OnceLock::new();
    let systems = SYSTEMS.get_or_init(Default::default);
    if let Some(done) = systems.lock().unwrap().get(b.label) {
        return done.clone();
    }
    let (source, format) = match b.source {
        Source::Template(id) => {
            let templates: Vec<Template> =
                serde_json::from_str(TEMPLATES_JSON).expect("templates.json");
            let t = templates
                .into_iter()
                .find(|t| t.id == id)
                .unwrap_or_else(|| panic!("no template {id:?}"));
            (t.equations.join("\n"), t.format)
        }
        Source::Text(text) => (text.to_string(), InputFormat::Text),
    };
    let res = service::compile_system(store(), &CompileRequest { source, format });
    assert!(
        res.ok
            && !res
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error),
        "{} doesn't compile: {:?}",
        b.label,
        res.diagnostics
    );
    let sys = Arc::new(System {
        id: res.system_id.expect("compiled"),
        names: res.params.iter().map(|p| p.name.clone()).collect(),
    });
    systems.lock().unwrap().insert(b.label, sys.clone());
    sys
}

impl System {
    fn param(&self, name: &str) -> usize {
        self.names
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("no parameter {name} in {:?}", self.names))
    }

    fn values(&self, b: &Branch) -> Vec<f64> {
        self.names
            .iter()
            .map(|n| {
                b.params
                    .iter()
                    .find(|(m, _)| m == n)
                    .unwrap_or_else(|| panic!("{}: no value for {n}", b.label))
                    .1
            })
            .collect()
    }
}

/// `continuationStep`: a hundredth of the range, to one significant digit.
fn branch_step((lo, hi): (f64, f64)) -> f64 {
    format!("{:.0e}", (hi - lo) / 100.0)
        .parse()
        .expect("a number")
}

fn branch(b: &'static Branch) -> Arc<BranchOut> {
    static BRANCHES: OnceLock<Mutex<HashMap<&'static str, Arc<BranchOut>>>> = OnceLock::new();
    let branches = BRANCHES.get_or_init(Default::default);
    if let Some(done) = branches.lock().unwrap().get(b.label) {
        return done.clone();
    }
    let sys = system(b);
    let ds = branch_step(b.range);
    let req = ContinuationRequest {
        request_id: 1,
        system_id: sys.id,
        params: sys.values(b),
        param: sys.param(b.param),
        range: b.range,
        start: ContinuationStart::Point {
            x0: b.x0.to_vec(),
            period: 1,
        },
        direction: ContinuationDirection::Both,
        ds,
        ds_max: 2.0 * ds,
        max_points: BRANCH_MAX_POINTS,
    };
    let compiled = store().get(sys.id).expect("stored");
    let out = service::compute_continuation(&compiled, &req)
        .unwrap_or_else(|e| panic!("{}: {e}", b.label));
    let out = Arc::new(out.branch);
    branches.lock().unwrap().insert(b.label, out.clone());
    out
}

/// `curveKindOf` (app/src/state/curves.ts).
fn curve_kind(kind: SpecialKindOut) -> CurveKindOut {
    match kind {
        SpecialKindOut::Fold => CurveKindOut::Fold,
        SpecialKindOut::Hopf => CurveKindOut::Hopf,
        SpecialKindOut::BranchPoint => CurveKindOut::BranchPoint,
        SpecialKindOut::Flip => CurveKindOut::Flip,
        SpecialKindOut::NeimarkSacker => CurveKindOut::NeimarkSacker,
    }
}

/// A run's curve and where its seed was, across.
struct Followed {
    curve: CurveOut,
    seed: f64,
}

fn curve(run: &'static Run) -> Arc<Followed> {
    static CURVES: OnceLock<Mutex<HashMap<&'static str, Arc<Followed>>>> = OnceLock::new();
    let curves = CURVES.get_or_init(Default::default);
    if let Some(done) = curves.lock().unwrap().get(run.label) {
        return done.clone();
    }
    let (b, sys) = (branch(run.branch), system(run.branch));
    let at = b
        .special
        .iter()
        .filter(|sp| sp.kind == run.from)
        .min_by(|p, q| (p.p - run.near).abs().total_cmp(&(q.p - run.near).abs()))
        .unwrap_or_else(|| panic!("{}: no {:?} on {}", run.label, run.from, run.branch.label));
    let across = sys.param(run.branch.param);
    let mut params = sys.values(run.branch);
    params[across] = at.p;
    let req = CurveRequest {
        request_id: 1,
        system_id: sys.id,
        params,
        free: (across, sys.param(run.up)),
        ranges: run.ranges,
        seed: CurveSeed {
            kind: curve_kind(run.from),
            x: at.x.clone(),
            period: at.period,
        },
        direction: ContinuationDirection::Both,
        ds: UI_DS,
        ds_max: 2.0 * UI_DS,
        max_points: UI_MAX_POINTS,
    };
    let compiled = store().get(sys.id).expect("stored");
    let out =
        service::compute_curve(&compiled, &req).unwrap_or_else(|e| panic!("{}: {e}", run.label));
    let out = Arc::new(Followed {
        curve: out.curve,
        seed: at.p,
    });
    curves.lock().unwrap().insert(run.label, out.clone());
    out
}

/// The located point of `kind` nearest `near`, each parameter as a fraction
/// of its side of the box.
fn nearest(run: &'static Run, kind: Codim2KindOut, near: (f64, f64)) -> Option<(f64, f64)> {
    let c = curve(run);
    let ((lo1, hi1), (lo2, hi2)) = run.ranges;
    let d = |p: (f64, f64)| ((p.0 - near.0) / (hi1 - lo1)).hypot((p.1 - near.1) / (hi2 - lo2));
    c.curve
        .special
        .iter()
        .filter(|sp| sp.kind == kind)
        .map(|sp| sp.p)
        .min_by(|p, q| d(*p).total_cmp(&d(*q)))
}

/// `which` of the normal form of the located point of `kind` nearest `near`.
fn coefficient_of(
    run: &'static Run,
    kind: Codim2KindOut,
    near: (f64, f64),
    which: Coefficient,
) -> Option<f64> {
    let at = nearest(run, kind, near)?;
    let c = curve(run);
    let sp = c.curve.special.iter().find(|sp| sp.p == at)?;
    match (which, sp.normal_form?) {
        (Coefficient::BtA, NormalFormOut::BogdanovTakens { a, .. }) => Some(a.abs()),
        (Coefficient::BtB, NormalFormOut::BogdanovTakens { b, .. }) => Some(b.abs()),
        (Coefficient::BtSignAb, NormalFormOut::BogdanovTakens { a, b }) => Some((a * b).signum()),
        (Coefficient::CuspC, NormalFormOut::Cusp { c }) => Some(c),
        _ => None,
    }
}

/// The point or a point beside it is a neutral saddle.
fn on_a_neutral_stretch(c: &CurveOut, index: usize) -> bool {
    let to = (index + 1).min(c.neutral.len() - 1);
    c.neutral[index.saturating_sub(1)..=to].iter().any(|n| *n)
}

fn measure(check: &Check) -> f64 {
    let missing = f64::NAN;
    match check.measure {
        Measure::P {
            run,
            kind,
            near,
            axis,
        } => nearest(run, kind, near).map_or(missing, |p| [p.0, p.1][axis]),
        Measure::Agree { runs, kind, near } => {
            match (nearest(runs[0], kind, near), nearest(runs[1], kind, near)) {
                (Some(a), Some(b)) => (a.0 - b.0).abs().max((a.1 - b.1).abs()),
                _ => missing,
            }
        }
        Measure::Count(run) => curve(run).curve.special.len() as f64,
        Measure::OnNeutral { run, kinds } => {
            let c = &curve(run).curve;
            c.special
                .iter()
                .filter(|sp| kinds.contains(&sp.kind) && on_a_neutral_stretch(c, sp.index))
                .count() as f64
        }
        Measure::On {
            runs,
            curve: kind_of_curve,
            kind,
            near,
        } => runs
            .iter()
            .filter(|run| curve(run).curve.kind == kind_of_curve)
            .filter(|run| {
                nearest(run, kind, near).is_some_and(|p| {
                    (p.0 - near.0).abs() <= REF_TOL && (p.1 - near.1).abs() <= REF_TOL
                })
            })
            .count() as f64,
        Measure::Coefficient {
            run,
            kind,
            near,
            which,
        } => coefficient_of(run, kind, near, which).unwrap_or(missing),
        Measure::Misplaced => RUNS
            .iter()
            .map(|run| {
                let c = &curve(run).curve;
                c.special
                    .iter()
                    .filter(|sp| (c.p1[sp.index], c.p2[sp.index]) != sp.p)
                    .count()
            })
            .sum::<usize>() as f64,
    }
}

struct Row {
    text: &'static str,
    expect: &'static Expect,
    measured: f64,
}

impl Row {
    fn delta(&self) -> Option<f64> {
        match *self.expect {
            Expect::Near { value, .. } => Some((self.measured - value).abs()),
            Expect::Exactly { count } => Some((self.measured - count as f64).abs()),
            Expect::AtMost { .. } => None,
        }
        .filter(|d| d.is_finite())
    }

    fn passes(&self) -> bool {
        match *self.expect {
            Expect::Near { tol, .. } => self.delta().is_some_and(|d| d <= tol),
            Expect::Exactly { .. } => self.delta() == Some(0.0),
            Expect::AtMost { bound } => self.measured <= bound,
        }
    }
}

fn print_line([claim, reference, measured, delta, tol, verdict]: [&str; 6]) {
    let pad = " ".repeat(54usize.saturating_sub(claim.chars().count()));
    println!("{claim}{pad} {reference:>16} {measured:>16} {delta:>8} {tol:>8} {verdict}");
}

/// Twelve decimals, without the sign of a value that rounds to zero.
fn fixed(v: f64) -> String {
    let s = format!("{v:.12}");
    match s.strip_prefix('-') {
        Some(digits) if digits.bytes().all(|c| c == b'0' || c == b'.') => digits.to_string(),
        _ => s,
    }
}

fn print_table(rows: &[Row]) {
    println!();
    print_line(["claim", "reference", "measured", "Δ", "tol", "result"]);
    for r in rows {
        let (reference, measured, tol) = match *r.expect {
            Expect::Near { value, tol } => (fixed(value), fixed(r.measured), format!("{tol:.0e}")),
            Expect::AtMost { bound } => (
                "0".to_string(),
                format!("{:.3e}", r.measured),
                format!("≤ {bound:.0e}"),
            ),
            Expect::Exactly { count } => (
                count.to_string(),
                format!("{}", r.measured),
                "exact".to_string(),
            ),
        };
        print_line([
            r.text,
            &reference,
            &measured,
            &r.delta().map_or("—".to_string(), |d| format!("{d:.1e}")),
            &tol,
            if r.passes() { "PASS" } else { "FAIL" },
        ]);
    }
}

fn evaluate(check: &'static Check) -> Row {
    Row {
        text: check.text,
        expect: &check.expect,
        measured: measure(check),
    }
}

/// Each run's seed and box, as the table's claims use them.
fn print_runs() {
    println!(
        "\nEach curve: Δs = {UI_DS}, Δs max = {}, at most {UI_MAX_POINTS} points each way, both \
         directions, as the Parameter plane sends them.",
        2.0 * UI_DS
    );
    for run in RUNS {
        let c = curve(run);
        let ((lo1, hi1), (lo2, hi2)) = run.ranges;
        println!(
            "  {:<20} from {:?} at {} = {:.8} ({}); box {} ∈ [{lo1}, {hi1}], {} ∈ [{lo2}, {hi2}]; \
             {} points, ends {:?} and {:?}",
            run.label,
            run.from,
            run.branch.param,
            c.seed,
            run.branch.label,
            run.branch.param,
            run.up,
            c.curve.p1.len(),
            c.curve.end.first,
            c.curve.end.last
        );
    }
}

#[test]
fn codim2_claims_reproduce() {
    let started = Instant::now();
    let rows: Vec<Row> = CHECKS
        .iter()
        .chain(CO_COEFFICIENTS.iter().flat_map(|c| c.iter()))
        .map(evaluate)
        .collect();
    print_runs();
    print_table(&rows);
    let widest = RUNS
        .iter()
        .flat_map(|run| curve(run).curve.special.clone())
        .filter_map(|sp| sp.bracket)
        .fold(0.0, f64::max);
    print_line([
        "info: widest bisection bracket, scaled arclength",
        "—",
        &format!("{widest:.3e}"),
        "—",
        "—",
        "info",
    ]);
    println!(
        "{} rows, wall time {:.2} s",
        rows.len(),
        started.elapsed().as_secs_f64()
    );
    let failed: Vec<&str> = rows
        .iter()
        .filter(|r| !r.passes())
        .map(|r| r.text)
        .collect();
    assert!(failed.is_empty(), "failed claims: {failed:?}");
}

#[test]
fn wrong_claims_fail() {
    let rows: Vec<Row> = WRONG_CHECKS.iter().map(evaluate).collect();
    print_table(&rows);
    let passed: Vec<&str> = rows.iter().filter(|r| r.passes()).map(|r| r.text).collect();
    assert!(passed.is_empty(), "wrong claims not failed: {passed:?}");
}
