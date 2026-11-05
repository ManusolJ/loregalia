mod rules;

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

const USAGE: &str = "Usage: cargo xtask gate [--fast]

  gate         project rules, format, clippy, every test (run before finishing a task)
  gate --fast  project rules, format, loregalia-core and xtask tests (run while iterating)";

struct Step {
    name: &'static str,
    args: &'static [&'static str],
    hint: &'static str,
}

const FORMAT: Step = Step {
    name: "format",
    args: &["fmt", "--all", "--check"],
    hint: "Run `cargo fmt --all`, then rerun the gate.",
};

const CLIPPY: Step = Step {
    name: "clippy",
    args: &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ],
    hint: "Fix the code. Do not add `#[allow]` unless a human approved it.",
};

const FAST_TESTS: Step = Step {
    name: "fast tests",
    args: &[
        "test",
        "--locked",
        "--package",
        "loregalia-core",
        "--package",
        "xtask",
    ],
    hint: "Fix the code, not the test, unless the test itself is wrong.",
};

const ALL_TESTS: Step = Step {
    name: "tests",
    args: &["test", "--workspace", "--locked"],
    hint: "Fix the code, not the test, unless the test itself is wrong.",
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.as_slice() {
        ["gate"] => gate(&[&FORMAT, &CLIPPY, &ALL_TESTS]),
        ["gate", "--fast"] => gate(&[&FORMAT, &FAST_TESTS]),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn gate(steps: &[&Step]) -> ExitCode {
    let root = workspace_root();
    let started = Instant::now();

    let violations = rules::check(&root);
    if !violations.is_empty() {
        println!("[FAIL] project rules");
        for violation in &violations {
            println!("  {violation}");
        }

        return fail("project rules", "Fix each violation listed above.");
    }
    println!("[ ok ] project rules");

    for step in steps {
        let step_started = Instant::now();
        let succeeded = run_cargo(&root, step.args);
        let seconds = step_started.elapsed().as_secs();

        if !succeeded {
            println!("[FAIL] {} ({seconds}s)", step.name);

            return fail(step.name, step.hint);
        }

        println!("[ ok ] {} ({seconds}s)", step.name);
    }

    println!(
        "GATE PASSED in {}s. Manual check still needed for anything a test cannot see.",
        started.elapsed().as_secs()
    );

    ExitCode::SUCCESS
}

fn fail(step: &str, hint: &str) -> ExitCode {
    println!("GATE FAILED at: {step}");
    println!("Next: {hint}");

    ExitCode::FAILURE
}

fn run_cargo(root: &Path, args: &[&str]) -> bool {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());

    Command::new(cargo)
        .args(args)
        .current_dir(root)
        .status()
        .is_ok_and(|status| status.success())
}

fn workspace_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));

    manifest.parent().unwrap_or(manifest).to_path_buf()
}
