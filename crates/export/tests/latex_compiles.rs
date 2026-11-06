//! Every golden LaTeX output compiles with pdfLaTeX in a minimal document
//! that loads exactly the packages its `% Needs:` line lists.
//!
//! Skipped when `pdflatex` is not installed, unless `REQUIRE_PDFLATEX=1`
//! (CI's TeX job), which turns a missing `pdflatex` into a failure. On
//! Ubuntu: `apt-get install texlive-latex-base texlive-latex-recommended
//! texlive-latex-extra texlive-pictures texlive-science
//! texlive-fonts-recommended lmodern`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn have_pdflatex() -> bool {
    let found = Command::new("pdflatex")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !found && std::env::var_os("REQUIRE_PDFLATEX").is_some_and(|v| v == "1") {
        panic!("REQUIRE_PDFLATEX=1 but pdflatex is not installed");
    }
    if !found {
        eprintln!("pdflatex not found: skipping the LaTeX compile tests");
    }
    found
}

/// The preamble lines a snippet asks for in its `% Needs:` line.
fn needs(tex: &str) -> String {
    let line = tex
        .lines()
        .find_map(|l| l.strip_prefix("% Needs: "))
        .expect("every snippet lists its packages");
    // `\usepackage{a} \usepackage{b} \pgfplotsset{…}` → one per line.
    line.replace(" \\", "\n\\")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sepx-latex-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pdflatex(dir: &Path, main: &str) {
    let out = Command::new("pdflatex")
        .args(["-interaction=nonstopmode", "-halt-on-error", main])
        .current_dir(dir)
        .output()
        .expect("run pdflatex");
    if !out.status.success() {
        let log = String::from_utf8_lossy(&out.stdout);
        let tail: Vec<&str> = log.lines().rev().take(40).collect();
        panic!(
            "{} failed to compile in {}:\n{}",
            main,
            dir.display(),
            tail.into_iter().rev().collect::<Vec<_>>().join("\n")
        );
    }
    let log = std::fs::read_to_string(dir.join(main.replace(".tex", ".log"))).unwrap_or_default();
    assert!(!log.contains("! "), "{main} logged an error");
    assert!(
        !log.contains("Dimension too large"),
        "{main}: a coordinate is too large for TeX"
    );
}

fn golden_tex() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::fs::read_dir(golden_dir())
        .expect("tests/golden exists (run the golden test first)")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tex"))
        .map(|n| {
            let text = std::fs::read_to_string(golden_dir().join(&n)).unwrap();
            (n, text)
        })
        .collect();
    out.sort();
    out
}

/// The data files a figure reads through `\sepxdir`.
fn data_files(tex: &str) -> Vec<String> {
    tex.split("{\\sepxdir ")
        .skip(1)
        .filter_map(|s| s.split('}').next())
        .map(str::to_string)
        .collect()
}

#[test]
fn every_golden_snippet_compiles_in_a_minimal_document() {
    if !have_pdflatex() {
        return;
    }
    let all = golden_tex();
    assert!(all.len() >= 10, "{}", all.len());
    for (name, tex) in all {
        let dir = scratch(name.trim_end_matches(".tex"));
        for f in data_files(&tex) {
            std::fs::copy(golden_dir().join(&f), dir.join(&f)).unwrap();
        }
        let doc = if tex.contains("\\documentclass") {
            tex.clone()
        } else {
            format!(
                "\\documentclass{{article}}\n{}\n\\begin{{document}}\n{}\\end{{document}}\n",
                needs(&tex),
                tex
            )
        };
        std::fs::write(dir.join("main.tex"), doc).unwrap();
        pdflatex(&dir, "main.tex");
        assert!(dir.join("main.pdf").exists(), "{name}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn an_external_figure_can_be_input_from_a_subdirectory() {
    if !have_pdflatex() {
        return;
    }
    for name in [
        "rabbits-vs-sheep-figure-external.tex",
        "henon-figure-external.tex",
    ] {
        let tex = std::fs::read_to_string(golden_dir().join(name)).unwrap();
        let dir = scratch(&format!("sub-{name}"));
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(name), &tex).unwrap();
        for f in data_files(&tex) {
            std::fs::copy(golden_dir().join(&f), sub.join(&f)).unwrap();
        }
        // Exactly the line the figure's own comment asks for, with the
        // folder changed.
        let asked = tex
            .lines()
            .find_map(|l| l.split(", put ").nth(1)?.strip_suffix(" before it."))
            .expect("the figure says how to \\input it from another folder");
        assert_eq!(asked, "\\def\\sepxdir{figures/}");
        let doc = format!(
            "\\documentclass{{article}}\n{}\n\\begin{{document}}\n{}\n\\input{{sub/{name}}}\n\\end{{document}}\n",
            needs(&tex),
            asked.replace("figures/", "sub/"),
        );
        std::fs::write(dir.join("main.tex"), doc).unwrap();
        pdflatex(&dir, "main.tex");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The page size of a PDF in bp, from `pdfinfo`.
fn page_size(pdf: &Path) -> Option<(f64, f64)> {
    let out = Command::new("pdfinfo").arg(pdf).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.starts_with("Page size:"))?;
    let mut nums = line
        .split_whitespace()
        .filter_map(|w| w.parse::<f64>().ok());
    Some((nums.next()?, nums.next()?))
}

#[test]
fn a_standalone_figure_is_its_axis_plus_the_border() {
    if !have_pdflatex() {
        return;
    }
    // Its tick labels and names all sit inside the axis.
    {
        let name = "rabbits-vs-sheep-figure-dark-standalone.tex";
        let tex = std::fs::read_to_string(golden_dir().join(name)).unwrap();
        let dir = scratch(&format!("width-{name}"));
        std::fs::write(dir.join("main.tex"), &tex).unwrap();
        pdflatex(&dir, "main.tex");
        let Some((w, _)) = page_size(&dir.join("main.pdf")) else {
            eprintln!("pdfinfo not found: skipping the page size check");
            return;
        };
        // 8 cm of axis and a 2 mm border each side; the labels' halos may
        // reach a little past the axis. Stray spaces before the picture
        // used to add some 50 pt.
        let want = 80.0 / 25.4 * 72.0 + 2.0 * 2.0 / 25.4 * 72.0;
        assert!(
            (w - want).abs() < 4.0,
            "{name}: the page is {w} bp wide, not {want}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn compile_snippet(name: &str, tex: &str, files: &[(String, Vec<u8>)]) -> String {
    let dir = scratch(name);
    for (f, bytes) in files {
        std::fs::write(dir.join(f), bytes).unwrap();
    }
    let doc = format!(
        "\\documentclass{{article}}\n{}\n\\begin{{document}}\n{tex}\\end{{document}}\n",
        needs(tex)
    );
    std::fs::write(dir.join("main.tex"), doc).unwrap();
    pdflatex(&dir, "main.tex");
    let text = Command::new("pdftotext")
        .args(["-layout", "main.pdf", "-"])
        .current_dir(&dir)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    text
}

#[test]
fn the_lyapunov_table_prints_each_exponent_with_its_error() {
    if !have_pdflatex() {
        return;
    }
    let name = "lorenz-lyapunov-table.tex";
    let tex = std::fs::read_to_string(golden_dir().join(name)).unwrap();
    let text = compile_snippet("lyapunov", &tex, &[]);
    if text.is_empty() {
        eprintln!("pdftotext not found: skipping the text check");
        return;
    }
    // uncertainty-mode = separate: "0.906 ± 0.004", never "0.906(4)".
    for want in ["0.906 ± 0.004", "−14.5720 ± 0.0010", "Runge–Kutta 4"] {
        assert!(text.contains(want), "{want:?} is not in\n{text}");
    }
    assert!(!text.contains("0.906(4)"), "{text}");
}

#[test]
fn a_large_orbit_diagram_compiles_with_its_density_layer() {
    use separatrix_export::{
        orbit_figure, Labels, OrbitDiagram, OrbitFigureInput, OrbitRun, ParamValue, Placement,
        StateName, SystemKind, SystemMeta, Theme,
    };
    if !have_pdflatex() {
        return;
    }
    let (columns, kept) = (945, 200);
    let step = (4.0 - 2.8) / (columns - 1) as f64;
    let r: Vec<f64> = (0..columns).map(|j| 2.8 + j as f64 * step).collect();
    let mut x = Vec::with_capacity(columns * kept);
    for &rj in &r {
        let mut v: f64 = 0.5;
        for n in 0..1000 + kept {
            if n >= 1000 {
                x.push(v);
            }
            v = rj * v * (1.0 - v);
        }
    }
    let labels = Labels {
        states: vec![StateName {
            name: "x".into(),
            latex: "x".into(),
        }],
        params: vec![ParamValue {
            name: "r".into(),
            latex: "r".into(),
            value: 3.2,
        }],
    };
    let lines = vec!["x_{n+1}=rx_n\\left(1-x_n\\right)".to_string()];
    let sys = SystemMeta {
        kind: SystemKind::Map,
        labels: &labels,
        title: "Logistic map",
        source_ref: None,
        lines: &lines,
        format: dyn_expr::InputFormat::Latex,
    };
    let run = OrbitRun {
        param: 0,
        state: 0,
        x0: vec![0.5],
        transient: 1000,
        diagram: OrbitDiagram { r, x, kept },
    };
    let tokens = include_str!("../../../app/src/styles/ds/tokens.json");
    let pal = separatrix_export::Palette::from_tokens(tokens, Theme::Light).unwrap();
    let out = orbit_figure(&OrbitFigureInput {
        sys: &sys,
        run: &run,
        x_range: (2.8, 4.0),
        y_range: (0.0, 1.0),
        width_cm: 8.0,
        height_cm: 6.4,
        theme: Theme::Light,
        palette: &pal,
        placement: Placement::External {
            base: "logistic-orbit".into(),
        },
        standalone: false,
    });
    assert!(out.tex.contains("\\addplot graphics"), "{}", out.tex);
    let files: Vec<(String, Vec<u8>)> = out
        .files
        .iter()
        .map(|f| (f.name.clone(), f.bytes.clone()))
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].0, "logistic-orbit-points.png");
    compile_snippet("orbit", &out.tex, &files);
}
