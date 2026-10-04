//! Compares our mdast renderer against the CommonMark spec's own expected HTML.
//!
//! Usage: `SPEC=/path/to/spec.json cargo run --example spec_check`
//!
//! The crate's `to_html_with_options` output is also compared, so a mismatch
//! can be attributed to either us or a spec-version difference.

#![allow(dead_code)] // the #[path] include exposes fns this target doesn't call

#[path = "../src/render.rs"]
mod render;

use std::collections::BTreeMap;

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct Case {
    markdown: String,
    html: String,
    example: usize,
    section: String,
}

fn load(path: &str) -> Vec<Case> {
    let raw = std::fs::read_to_string(path).unwrap();
    let cases: Vec<Case> = serde_json::from_str(&raw).unwrap();
    cases
}

fn reference(md: &str) -> String {
    markdown::to_html_with_options(md, &markdown::Options {
        compile: markdown::CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..markdown::CompileOptions::default()
        },
        ..markdown::Options::default()
    })
    .unwrap()
}

fn compile(md: &str) -> String {
    let tree = markdown::to_mdast(md, &markdown::ParseOptions::default()).unwrap();
    render::compile_html_content_tree(&tree)
}

/// Ignore insignificant whitespace: we concatenate block elements with no
/// separator, the spec pretty-prints with newlines and indentation.
fn squash(html: &str) -> String {
    html.chars().filter(|c| !c.is_whitespace()).collect()
}

fn main() {
    let path = std::env::var("SPEC").expect("set SPEC to a CommonMark spec.json");
    let cases = load(&path);

    let mut vs_spec: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut vs_crate: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut shown = 0;

    for case in &cases {
        let ours = compile(&case.markdown);
        let theirs = reference(&case.markdown);
        if squash(&ours) != squash(&case.html) {
            vs_spec.entry(case.section.clone()).or_default().push(case.example);
        }
        if squash(&ours) != squash(&theirs) {
            vs_crate.entry(case.section.clone()).or_default().push(case.example);
        }
        if squash(&ours) != squash(&theirs)
            && shown < 8
            && std::env::var("SHOW").is_ok()
        {
            shown += 1;
            println!(
                "--- example {} [{}] ---\n  input:    {:?}\n  ours:     {}\n  crate:    {}\n  spec:     {}\n",
                case.example, case.section, case.markdown,
                ours, theirs, case.html
            );
        }
    }

    let total = |m: &BTreeMap<String, Vec<usize>>| m.values().map(Vec::len).sum::<usize>();
    println!("{} examples", cases.len());
    println!(
        "  ours vs crate: {} mismatches across {} sections",
        total(&vs_crate),
        vs_crate.len()
    );
    println!(
        "  ours vs spec:  {} mismatches across {} sections",
        total(&vs_spec),
        vs_spec.len()
    );
    for (section, examples) in &vs_crate {
        println!("    {section:<26} {} (e.g. {:?})", examples.len(), &examples[..examples.len().min(6)]);
    }
}
