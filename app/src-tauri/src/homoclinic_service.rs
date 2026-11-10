//! What the `compute_homoclinic` command does, testable without a window:
//! from a Bogdanov–Takens point, the curve of homoclinic orbits as two
//! parameters vary (see [`dyn_analysis::continue_homoclinic`]), run as a job
//! the interface can cancel.

use std::time::Instant;

use dyn_analysis::{CurveOptions, Homoclinic, HomoclinicEnd, HomoclinicError};
use dyn_core::{SystemKind, VectorField};
use dyn_expr::CompiledSystem;

use crate::ipc::{
    ContinuationDirection, HomoclinicEndOut, HomoclinicOut, HomoclinicProgress, HomoclinicRequest,
    HomoclinicResponse, HomoclinicStop, MAX_HOMOCLINIC_POINTS,
};
use crate::jobs::Jobs;
use crate::service::{CONTINUATION_DS_MIN, CONTINUATION_MAX_ANGLE};

/// The homoclinic orbit beside the Bogdanov–Takens point of `req`, found
/// once, and the curve through it in `req.direction`. `progress` hears each
/// point of the curve as it is accepted, both directions counted together.
/// `None` once `cancelled` turns true, with nothing of the run kept.
pub fn compute_homoclinic(
    system: &CompiledSystem,
    req: &HomoclinicRequest,
    cancelled: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(HomoclinicProgress),
) -> Result<Option<HomoclinicResponse>, String> {
    let started = Instant::now();
    let f: &dyn VectorField = system;
    validate(f, req)?;
    let opts = |sign: f64| CurveOptions {
        free: req.free,
        ranges: req.ranges,
        ds: sign * req.ds,
        ds_min: CONTINUATION_DS_MIN,
        ds_max: req.ds_max,
        max_points: req.max_points,
        max_angle: CONTINUATION_MAX_ANGLE,
    };
    let mut halves = || {
        let start =
            dyn_analysis::homoclinic_from_bt(f, &req.params, &req.x, &opts(1.0), cancelled)?;
        let mut sent = 0;
        let mut follow = |sign: f64, again: bool| {
            // The second half starts at the first one's start.
            let mut skip = again;
            dyn_analysis::continue_homoclinic(f, &start, &opts(sign), cancelled, &mut |pt| {
                if std::mem::take(&mut skip) {
                    return;
                }
                sent += 1;
                progress(HomoclinicProgress {
                    request_id: req.request_id,
                    points: sent,
                    p: pt.p,
                });
            })
        };
        Ok(match req.direction {
            ContinuationDirection::Forward => (None, follow(1.0, false)?),
            ContinuationDirection::Backward => (None, follow(-1.0, false)?),
            ContinuationDirection::Both => {
                let forward = follow(1.0, false)?;
                (Some(follow(-1.0, true)?), forward)
            }
        })
    };
    match halves() {
        Err(HomoclinicError::Cancelled) => Ok(None),
        // A cancel after the engine's last look keeps nothing either.
        Ok(_) if cancelled() => Ok(None),
        Ok((backward, forward)) => Ok(Some(HomoclinicResponse {
            request_id: req.request_id,
            curve: join(backward, forward),
            elapsed_ms: started.elapsed().as_secs_f64() * 1e3,
        })),
        Err(e) => Err(refusal(system, req, e)),
    }
}

/// [`compute_homoclinic`] as job `req.request_id` of `jobs`, which
/// [`Jobs::cancel`] stops, before it starts or while it runs. The job is
/// unregistered when it returns.
pub fn run_job(
    jobs: &Jobs,
    system: &CompiledSystem,
    req: &HomoclinicRequest,
    progress: &mut dyn FnMut(HomoclinicProgress),
) -> Result<Option<HomoclinicResponse>, String> {
    let job = jobs.start(req.request_id);
    compute_homoclinic(system, req, &|| job.cancelled(), progress)
}

const UNSUPPORTED: &str = "Homoclinic curves are followed in flows with two or more states.";

const NOT_AUTONOMOUS: &str = "A homoclinic orbit returns to an equilibrium, which a system that \
     depends on t doesn't have.";

/// The engine's refusal as a sentence, naming the free parameters by their
/// LaTeX as `service::curve_refusal` does.
fn refusal(system: &CompiledSystem, req: &HomoclinicRequest, e: HomoclinicError) -> String {
    let params = &system.info.params;
    let (across, up) = (&params[req.free.0].latex, &params[req.free.1].latex);
    match e {
        HomoclinicError::NotAutonomous => NOT_AUTONOMOUS.into(),
        HomoclinicError::Unsupported => UNSUPPORTED.into(),
        HomoclinicError::NoSaddle => "No equilibrium next to this point is a saddle with a \
             one-dimensional unstable or stable manifold; start from a Bogdanov–Takens point (BT) \
             the Parameter plane found."
            .into(),
        HomoclinicError::NotFound => format!(
            "No homoclinic orbit was found next to this point; narrower ranges of {across} and \
             {up} search closer to it."
        ),
        HomoclinicError::Cancelled => unreachable!("a cancelled job has no refusal"),
    }
}

/// One polyline: the backward half reversed and without its start, then the
/// forward half.
fn join(backward: Option<Homoclinic>, forward: Homoclinic) -> HomoclinicOut {
    let stop = |end| match end {
        HomoclinicEnd::Range => HomoclinicStop::Range,
        HomoclinicEnd::MaxPoints => HomoclinicStop::MaxPoints,
        HomoclinicEnd::StepTooSmall => HomoclinicStop::StepTooSmall,
        HomoclinicEnd::Shrunk => HomoclinicStop::Shrunk,
    };
    let end = HomoclinicEndOut {
        first: backward.as_ref().map(|b| stop(b.end)),
        last: stop(forward.end),
    };
    let reversed = forward.reversed;
    let points: Vec<_> = backward
        .into_iter()
        .flat_map(|b| b.points.into_iter().skip(1).rev())
        .chain(forward.points)
        .collect();
    HomoclinicOut {
        p1: points.iter().map(|pt| pt.p.0).collect(),
        p2: points.iter().map(|pt| pt.p.1).collect(),
        x: points
            .iter()
            .flat_map(|pt| pt.saddle.x.iter().copied())
            .collect(),
        sigma: points.iter().map(|pt| pt.saddle.sigma).collect(),
        focus: points.iter().map(|pt| pt.saddle.focus).collect(),
        chaotic: points.iter().map(|pt| pt.saddle.chaotic).collect(),
        return_time: points.iter().map(|pt| pt.return_time).collect(),
        reversed,
        end,
    }
}

fn validate(f: &dyn VectorField, req: &HomoclinicRequest) -> Result<(), String> {
    if f.kind() != SystemKind::Ode || f.dim() < 2 {
        return Err(UNSUPPORTED.into());
    }
    if !f.is_autonomous() {
        return Err(NOT_AUTONOMOUS.into());
    }
    let n = f.n_params();
    if req.params.len() != n {
        return Err(format!(
            "Expected {n} parameter value(s) but got {}.",
            req.params.len()
        ));
    }
    if n < 2 {
        return Err(format!(
            "A curve needs two parameters to follow; this system has {n}."
        ));
    }
    if let Some(i) = [req.free.0, req.free.1].into_iter().find(|&i| i >= n) {
        return Err(format!("Parameter {i} doesn't exist; the system has {n}."));
    }
    if req.free.0 == req.free.1 {
        return Err("The two free parameters must be different.".into());
    }
    let (across, up) = req.ranges;
    if ![across, up]
        .iter()
        .all(|&(lo, hi)| lo.is_finite() && hi.is_finite() && lo < hi)
    {
        return Err(
            "Both parameter ranges must be finite, each with its start below its end.".into(),
        );
    }
    if req.x.len() != f.dim() {
        return Err(format!(
            "The Bogdanov–Takens point has {} coordinate(s) but the system has {}.",
            req.x.len(),
            f.dim()
        ));
    }
    if !req.x.iter().chain(&req.params).all(|v| v.is_finite()) {
        return Err("The Bogdanov–Takens point and the parameters must be finite.".into());
    }
    if !(req.ds > 0.0 && req.ds.is_finite()) {
        return Err("The step ds must be a positive number.".into());
    }
    if !(req.ds_max >= req.ds && req.ds_max.is_finite()) {
        return Err("The largest step must be a number at least ds.".into());
    }
    if !(2..=MAX_HOMOCLINIC_POINTS).contains(&req.max_points) {
        return Err(format!(
            "A homoclinic curve needs 2 to {MAX_HOMOCLINIC_POINTS} points in each direction."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::{CompileRequest, InputFormat};
    use crate::service::{compile_system, SystemStore};
    use std::sync::Arc;

    const FREIRE: &str = "x' = (-(beta + nu)*x + beta*y - 0.328578*x^3 + 0.933578*(y - x)^3)/0.6\n\
                          y' = beta*x - beta*y - z - 0.933578*(y - x)^3\n\
                          z' = y";

    /// `ν = −β = −√0.6`.
    const BT: (f64, f64) = (-0.774_596_669_241_483_4, 0.774_596_669_241_483_4);

    fn compiled(source: &str, format: InputFormat) -> Arc<CompiledSystem> {
        let store = SystemStore::new();
        let res = compile_system(
            &store,
            &CompileRequest {
                source: source.into(),
                format,
            },
        );
        assert!(res.ok, "{:?}", res.diagnostics);
        store.get(res.system_id.expect("compiled")).expect("stored")
    }

    /// The request the Parameter plane sends from the Freire circuit's BT
    /// row, `nu` across and `beta` up.
    fn freire(max_points: usize) -> (Arc<CompiledSystem>, HomoclinicRequest) {
        let sys = compiled(FREIRE, InputFormat::Text);
        let index = |name: &str| {
            sys.info
                .params
                .iter()
                .position(|p| p.name == name)
                .expect("a parameter")
        };
        let free = (index("nu"), index("beta"));
        let mut params = vec![0.0; 2];
        params[free.0] = BT.0;
        params[free.1] = BT.1;
        let req = HomoclinicRequest {
            request_id: 41,
            system_id: 0,
            params,
            free,
            ranges: ((-1.2, -0.4), (0.1, 1.0)),
            x: vec![0.0; 3],
            direction: ContinuationDirection::Both,
            ds: 0.01,
            ds_max: 0.02,
            max_points,
        };
        (sys, req)
    }

    fn run(
        jobs: &Jobs,
        sys: &CompiledSystem,
        req: &HomoclinicRequest,
        mut on: impl FnMut(&HomoclinicProgress),
    ) -> (
        Result<Option<HomoclinicResponse>, String>,
        Vec<HomoclinicProgress>,
    ) {
        let mut heard = Vec::new();
        let res = run_job(jobs, sys, req, &mut |p| {
            on(&p);
            heard.push(p);
        });
        (res, heard)
    }

    #[test]
    fn freire_from_its_bt_point_is_one_curve_that_shrinks_onto_it() {
        let (sys, req) = freire(500);
        let jobs = Jobs::new();
        let (res, heard) = run(&jobs, &sys, &req, |_| {});
        let res = res.expect("a curve").expect("not cancelled");
        let c = &res.curve;
        let n = c.p1.len();
        assert_eq!(res.request_id, 41);
        for len in [
            c.p2.len(),
            c.sigma.len(),
            c.focus.len(),
            c.chaotic.len(),
            c.return_time.len(),
        ] {
            assert_eq!(len, n);
        }
        assert_eq!(c.x.len(), 3 * n);
        assert!(!c.reversed);
        let p: Vec<(f64, f64)> = c.p1.iter().copied().zip(c.p2.iter().copied()).collect();
        assert!(
            p.windows(2).all(|w| w[0] != w[1]),
            "the start is in the curve once"
        );

        let ends = [c.end.first.expect("direction both"), c.end.last];
        assert_eq!(
            ends.iter()
                .filter(|e| **e == HomoclinicStop::Shrunk)
                .count(),
            1,
            "{ends:?}"
        );
        let shrunk = if ends[0] == HomoclinicStop::Shrunk {
            p[0]
        } else {
            p[n - 1]
        };
        let d = (shrunk.0 - BT.0).hypot(shrunk.1 - BT.1);
        assert!(d <= 2.2e-6, "ends {d} from the BT point");

        let counts: Vec<usize> = heard.iter().map(|m| m.points).collect();
        assert_eq!(counts, (1..=n).collect::<Vec<_>>());
        assert!(heard.iter().all(|m| m.request_id == 41 && p.contains(&m.p)));
        assert!(jobs.is_empty());
    }

    #[test]
    fn a_cancelled_job_returns_none_and_keeps_nothing() {
        let (sys, req) = freire(12);
        let jobs = Jobs::new();
        let (reference, all) = run(&jobs, &sys, &req, |_| {});
        let reference = reference.expect("a curve").expect("not cancelled").curve;
        let n = reference.p1.len();
        // In the first half, in the second, and after the last point.
        for k in [3, n - 1, n] {
            let (res, heard) = run(&jobs, &sys, &req, |m| {
                if m.points == k {
                    jobs.cancel(req.request_id);
                }
            });
            assert_eq!(res, Ok(None), "cancelled after {k}");
            assert_eq!(heard.len(), k);
            assert!(jobs.is_empty());

            let (again, heard) = run(&jobs, &sys, &req, |_| {});
            let again = again.expect("a curve").expect("not cancelled").curve;
            assert_eq!(again, reference);
            assert_eq!(heard, all);
        }
    }

    #[test]
    fn a_cancel_before_the_job_starts_stops_it_before_any_point() {
        let (sys, req) = freire(12);
        let jobs = Jobs::new();
        jobs.cancel(req.request_id);
        jobs.cancel(req.request_id);
        let (res, heard) = run(&jobs, &sys, &req, |_| {});
        assert_eq!(res, Ok(None));
        assert!(heard.is_empty());
        assert!(jobs.is_empty());
    }

    fn refusal_of(sys: &CompiledSystem, req: &HomoclinicRequest) -> String {
        compute_homoclinic(sys, req, &|| false, &mut |_| {}).expect_err("a refusal")
    }

    #[test]
    fn requests_out_of_range_are_refused_with_these_sentences() {
        let (sys, req) = freire(500);
        type Change = fn(&mut HomoclinicRequest);
        let refused = |change: Change| {
            let mut r = req.clone();
            change(&mut r);
            refusal_of(&sys, &r)
        };
        let cases: [(Change, &str); 12] = [
            (
                |r| r.params.push(0.0),
                "Expected 2 parameter value(s) but got 3.",
            ),
            (
                |r| r.free.1 = 2,
                "Parameter 2 doesn't exist; the system has 2.",
            ),
            (
                |r| r.free.1 = r.free.0,
                "The two free parameters must be different.",
            ),
            (
                |r| r.ranges.1 = (1.0, 1.0),
                "Both parameter ranges must be finite, each with its start below its end.",
            ),
            (
                |r| {
                    r.x.pop();
                },
                "The Bogdanov–Takens point has 2 coordinate(s) but the system has 3.",
            ),
            (
                |r| r.x[0] = f64::NAN,
                "The Bogdanov–Takens point and the parameters must be finite.",
            ),
            (|r| r.ds = 0.0, "The step ds must be a positive number."),
            (
                |r| r.ds_max = 0.005,
                "The largest step must be a number at least ds.",
            ),
            (
                |r| r.max_points = 1,
                "A homoclinic curve needs 2 to 2000 points in each direction.",
            ),
            (
                |r| r.max_points = 2001,
                "A homoclinic curve needs 2 to 2000 points in each direction.",
            ),
            (
                |r| r.ranges.0 = (f64::NEG_INFINITY, 0.0),
                "Both parameter ranges must be finite, each with its start below its end.",
            ),
            (
                |r| r.params = vec![0.0],
                "Expected 2 parameter value(s) but got 1.",
            ),
        ];
        for (change, sentence) in cases {
            assert_eq!(refused(change), sentence);
        }
    }

    fn planar(source: &str, format: InputFormat, params: Vec<f64>) -> String {
        let sys = compiled(source, format);
        let dim = sys.info.states.len();
        let req = HomoclinicRequest {
            request_id: 1,
            system_id: 0,
            params,
            free: (0, 1),
            ranges: ((-2.0, 2.0), (-2.0, 2.0)),
            x: vec![0.0; dim],
            direction: ContinuationDirection::Both,
            ds: 0.01,
            ds_max: 0.02,
            max_points: 500,
        };
        refusal_of(&sys, &req)
    }

    #[test]
    fn systems_without_the_orbit_are_refused_with_these_sentences() {
        let linear = "x' = a*x\ny' = b*y";
        assert_eq!(
            planar(linear, InputFormat::Text, vec![-1.0, -1.0]),
            "No equilibrium next to this point is a saddle with a one-dimensional unstable or \
             stable manifold; start from a Bogdanov–Takens point (BT) the Parameter plane found."
        );
        assert_eq!(
            planar(linear, InputFormat::Text, vec![1.0, -1.0]),
            "No homoclinic orbit was found next to this point; narrower ranges of a and b search \
             closer to it."
        );
        assert_eq!(
            planar(
                "x' = a*x + sin(t)\ny' = b*y",
                InputFormat::Text,
                vec![1.0, -1.0]
            ),
            "A homoclinic orbit returns to an equilibrium, which a system that depends on t \
             doesn't have."
        );
        let unsupported = "Homoclinic curves are followed in flows with two or more states.";
        assert_eq!(
            planar(
                "x_{n+1}=ax_n\ny_{n+1}=by_n",
                InputFormat::Latex,
                vec![0.5, 2.0]
            ),
            unsupported
        );
        assert_eq!(
            planar("x' = a*x - b", InputFormat::Text, vec![1.0, 1.0]),
            unsupported
        );
    }
}
