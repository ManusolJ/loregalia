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
const RELAY_MANIFEST: &str = "crates/relay/Cargo.toml";
const APP_PACKAGE: &str = "loregalia";
const RAW_COLOR_CALLS: [&str; 4] = ["rgb(0x", "rgba(0x", "hsla(", "hsl("];
const BANNED_IMPORTS: [&str; 2] = ["use gpui_kit::*", "use gpui::"];

pub fn check(root: &Path) -> Vec<String> {
    let mut violations = Vec::new();

    check_core_is_pure(root, &mut violations);
    check_relay(root, &mut violations);
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

fn check_relay(root: &Path, violations: &mut Vec<String>) {
    let Ok(manifest) = fs::read_to_string(root.join(RELAY_MANIFEST)) else {
        return;
    };

    violations.extend(check_relay_manifest(&manifest));
}

pub fn check_relay_manifest(manifest: &str) -> Vec<String> {
    let mut violations = Vec::new();

    if manifest.contains("gpui") {
        violations.push(format!(
            "{RELAY_MANIFEST}: the relay must not depend on gpui or gpui-kit"
        ));
    }

    if manifest.lines().any(depends_on_app) {
        violations.push(format!(
            "{RELAY_MANIFEST}: the relay must not depend on the app crate (crates/app); use loregalia-core"
        ));
    }

    violations
}

fn depends_on_app(line: &str) -> bool {
    let line = line.trim();
    let key = line
        .split(['=', '.', ' ', '['])
        .find(|part| !part.is_empty())
        .unwrap_or_default();
    let table = line
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|header| header.rsplit('.').next());
    let renamed_package = format!("package = \"{APP_PACKAGE}\"");

    key == APP_PACKAGE
        || table == Some(APP_PACKAGE)
        || line.contains(&renamed_package)
        || line.contains("crates/app\"")
        || line.contains("\"../app\"")
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
    use super::{check_relay_manifest, check_source};
    use std::path::Path;

    const VIEW: &str = "crates/app/src/shell.rs";

    const CLEAN_RELAY: &str = "[package]\nname = \"loregalia-relay\"\n\n[dependencies]\nloregalia-core.workspace = true\naxum = \"0.8\"\n";

    #[test]
    fn a_relay_using_only_core_passes() {
        assert!(check_relay_manifest(CLEAN_RELAY).is_empty());
    }

    #[test]
    fn a_relay_depending_on_gpui_is_rejected() {
        let manifest = format!("{CLEAN_RELAY}gpui-kit.workspace = true\n");

        let violations = check_relay_manifest(&manifest);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("gpui"));
    }

    #[test]
    fn a_relay_depending_on_the_app_crate_is_rejected() {
        let manifests = [
            format!("{CLEAN_RELAY}loregalia.workspace = true\n"),
            format!("{CLEAN_RELAY}loregalia = {{ path = \"../app\" }}\n"),
            format!("{CLEAN_RELAY}\n[dependencies.loregalia]\nworkspace = true\n"),
            format!("{CLEAN_RELAY}views = {{ package = \"loregalia\", version = \"0.1\" }}\n"),
            format!("{CLEAN_RELAY}views = {{ path = \"../../crates/app\" }}\n"),
        ];

        for manifest in manifests {
            let violations = check_relay_manifest(&manifest);

            assert_eq!(violations.len(), 1, "{manifest}");
            assert!(violations[0].contains("crates/app"), "{manifest}");
        }
    }

    #[test]
    fn a_missing_relay_crate_is_not_a_violation() {
        let mut violations = Vec::new();

        super::check_relay(Path::new("no-such-root"), &mut violations);

        assert!(violations.is_empty());
    }

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
