use std::fs;
use std::path::{Path, PathBuf};

const AGENTS_MAX_LINES: usize = 120;
const THEME_FILE: &str = "crates/app/src/theme.rs";
const RULE_FILE: &str = "xtask/src/rules.rs";
const REQUIRED_DOCS: [&str; 6] = [
    "docs/ARCHITECTURE.md",
    "docs/DECISIONS.md",
    "docs/DESIGN.md",
    "docs/METADATA.md",
    "docs/PRODUCT.md",
    "docs/ROADMAP.md",
];
const RAW_COLOR_CALLS: [&str; 4] = ["rgb(0x", "rgba(0x", "hsla(", "hsl("];
const BANNED_IMPORTS: [&str; 2] = ["use gpui_kit::*", "use gpui::"];

pub fn check(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    check_core_is_pure(root, &mut violations);
    check_agents_file(root, &mut violations);

    for doc in REQUIRED_DOCS {
        if !root.join(doc).is_file() {
            violations.push(format!("{doc}: required document is missing"));
        }
    }

    for folder in ["crates", "xtask"] {
        for path in rust_files(&root.join(folder)) {
            let relative = relative_path(root, &path);
            if let Ok(source) = fs::read_to_string(&path) {
                violations.extend(check_source(&relative, &source));
            }
        }
    }

    violations
}

fn check_core_is_pure(root: &Path, violations: &mut Vec<String>) {
    let manifest = fs::read_to_string(root.join("crates/core/Cargo.toml")).unwrap_or_default();

    if manifest.contains("gpui") {
        violations.push(
            "crates/core/Cargo.toml: loregalia-core must not depend on gpui or gpui-kit".to_owned(),
        );
    }
}

fn check_agents_file(root: &Path, violations: &mut Vec<String>) {
    let Ok(agents) = fs::read_to_string(root.join("AGENTS.md")) else {
        violations.push("AGENTS.md: file is missing".to_owned());

        return;
    };

    let lines = agents.lines().count();
    if lines > AGENTS_MAX_LINES {
        violations.push(format!(
            "AGENTS.md: {lines} lines, limit is {AGENTS_MAX_LINES}; move detail into docs/"
        ));
    }
}

pub fn check_source(path: &str, source: &str) -> Vec<String> {
    let mut violations = Vec::new();

    if path == RULE_FILE {
        return violations;
    }

    for (index, line) in source.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim_start();

        if trimmed.starts_with("//") {
            violations.push(format!(
                "{path}:{number}: comment found; rename or restructure instead"
            ));
        }

        for banned in BANNED_IMPORTS {
            if trimmed.starts_with(banned) {
                violations.push(format!(
                    "{path}:{number}: `{banned}` is banned; import gpui-kit items by name"
                ));
            }
        }

        if path != THEME_FILE {
            for call in RAW_COLOR_CALLS {
                if line.contains(call) {
                    violations.push(format!(
                        "{path}:{number}: raw color `{call}`; use a Palette token from theme.rs"
                    ));
                }
            }
        }
    }

    violations
}

fn rust_files(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(folder) else {
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }

    files
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::check_source;

    const VIEW: &str = "crates/app/src/shell.rs";

    #[test]
    fn clean_source_passes() {
        let source = "use gpui_kit::div;\n\nfn view() {\n    div();\n}\n";

        assert!(check_source(VIEW, source).is_empty());
    }

    #[test]
    fn comments_are_reported_with_their_line() {
        let violations = check_source(VIEW, "fn a() {}\n// explain\n");

        assert_eq!(violations.len(), 1);
        assert!(violations[0].starts_with("crates/app/src/shell.rs:2:"));
    }

    #[test]
    fn wildcard_gpui_import_is_rejected() {
        assert_eq!(check_source(VIEW, "use gpui_kit::*;\n").len(), 1);
    }

    #[test]
    fn raw_colors_are_rejected_outside_the_theme() {
        assert_eq!(check_source(VIEW, "let c = rgb(0x111315);\n").len(), 1);
    }

    #[test]
    fn raw_colors_are_allowed_in_the_theme() {
        let source = "let c = rgb(0x111315);\n";

        assert!(check_source("crates/app/src/theme.rs", source).is_empty());
    }
}
