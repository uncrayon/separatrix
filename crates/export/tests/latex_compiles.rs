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
