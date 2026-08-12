use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::{collections::BTreeMap, fs};

mod generated;

use generated::generate_runtime;
use polygl_adapter_api::FeatureTag;
use polygl_core::Compiler;

const CONFORMANCE_LAYERS: [&str; 3] = ["l1-render", "l2-hir-snapshots", "l3-neutral-hir"];

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let Some(command) = args.next() else {
        return Err(usage());
    };

    match command.as_str() {
        "gen-runtime" => {
            let check = match args.next().as_deref() {
                None => false,
                Some("--check") => true,
                Some(_) => return Err(usage()),
            };
            ensure_no_more_args(args)?;
            generate_runtime(check).map_err(|error| error.to_string())
        }
        "conformance" => {
            ensure_no_more_args(args)?;
            check_conformance_layout().map_err(|error| error.to_string())
        }
        "capabilities" => {
            let check = match args.next().as_deref() {
                None => false,
                Some("--check") => true,
                Some(_) => return Err(usage()),
            };
            ensure_no_more_args(args)?;
            generate_capability_matrix(check)
        }
        "release-stages" => {
            ensure_no_more_args(args)?;
            check_release_stages()
        }
        _ => Err(usage()),
    }
}

fn check_release_stages() -> Result<(), String> {
    let root = workspace_root();
    let contents = fs::read_to_string(root.join("scripts/release-crate-stages.txt"))
        .map_err(|error| format!("failed to read release stages: {error}"))?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(&root)
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid cargo metadata: {error}"))?;
    validate_release_stages(&contents, &metadata)
}

fn validate_release_stages(contents: &str, metadata: &serde_json::Value) -> Result<(), String> {
    let mut stages = BTreeMap::new();
    for (stage, line) in contents.lines().enumerate() {
        for package in line.split_whitespace() {
            if stages.insert(package.to_owned(), stage).is_some() {
                return Err(format!("release crate `{package}` appears more than once"));
            }
        }
    }
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| "cargo metadata did not contain packages".to_owned())?;
    for staged in stages.keys() {
        if !packages.iter().any(|package| package["name"] == *staged) {
            return Err(format!("release stages contain unknown crate `{staged}`"));
        }
    }

    for package in packages {
        let name = package["name"]
            .as_str()
            .ok_or_else(|| "cargo metadata package is missing a name".to_owned())?;
        let publish = &package["publish"];
        let publishable = name.starts_with("polygl-")
            && (publish.is_null()
                || publish
                    .as_array()
                    .is_some_and(|registries| !registries.is_empty()));
        if publishable && !stages.contains_key(name) {
            return Err(format!(
                "publishable crate `{name}` is missing from release stages"
            ));
        }
        let Some(&package_stage) = stages.get(name) else {
            continue;
        };
        let dependencies = package["dependencies"]
            .as_array()
            .ok_or_else(|| format!("cargo metadata for `{name}` is missing dependencies"))?;
        for dependency in dependencies {
            if !dependency["kind"].is_null() || dependency["path"].is_null() {
                continue;
            }
            let dependency_name = dependency["name"]
                .as_str()
                .ok_or_else(|| format!("dependency of `{name}` is missing a name"))?;
            let Some(&dependency_stage) = stages.get(dependency_name) else {
                continue;
            };
            if dependency_stage >= package_stage {
                return Err(format!(
                    "release crate `{name}` in stage {} depends on `{dependency_name}` in non-earlier stage {}",
                    package_stage + 1,
                    dependency_stage + 1,
                ));
            }
        }
    }
    Ok(())
}

fn ensure_no_more_args(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    if args.next().is_some() {
        return Err(usage());
    }
    Ok(())
}

fn check_conformance_layout() -> io::Result<()> {
    let root = workspace_root().join("conformance");
    for layer in CONFORMANCE_LAYERS {
        let path = root.join(layer);
        if !path.is_dir() {
            return Err(io::Error::other(format!(
                "missing conformance layer: {}",
                path.display()
            )));
        }
    }
    polygl_conformance::verify_smoke(&root).map_err(io::Error::other)?;
    Ok(())
}

fn generate_capability_matrix(check: bool) -> Result<(), String> {
    let path = workspace_root().join("docs/capabilities.md");
    let generated = capability_matrix()?;
    if check {
        let committed = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if committed != generated {
            return Err(format!(
                "{} is stale; run `cargo xtask capabilities`",
                path.display()
            ));
        }
    } else {
        write_if_changed(&path, &generated)?;
    }
    Ok(())
}

fn capability_matrix() -> Result<String, String> {
    let compiler = Compiler::standard();
    let mut output = String::from(
        "# Adapter capability matrix\n\n\
         This file is generated by `cargo xtask capabilities`. Do not edit it by hand.\n\n\
         Capability names are versioned contracts; `yes` means that the adapter has a\n\
         conformance case for that contract.\n\n| adapter |",
    );
    for feature in FeatureTag::ALL {
        output.push(' ');
        output.push_str(feature.as_str());
        output.push_str(" |");
    }
    output.push_str("\n| --- |");
    for _ in FeatureTag::ALL {
        output.push_str(" --- |");
    }
    output.push('\n');

    for adapter in compiler.adapters().iter() {
        let mut seen = std::collections::BTreeSet::new();
        for feature in adapter.capabilities() {
            if !seen.insert(*feature) {
                return Err(format!(
                    "adapter `{}` advertises `{}` more than once",
                    adapter.id(),
                    feature.as_str()
                ));
            }
        }
        output.push_str("| ");
        output.push_str(adapter.id());
        output.push_str(" |");
        for feature in FeatureTag::ALL {
            output.push_str(if seen.contains(&feature) {
                " yes |"
            } else {
                " - |"
            });
        }
        output.push('\n');
    }
    Ok(output)
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    if fs::read_to_string(path).is_ok_and(|current| current == contents) {
        return Ok(());
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must live directly below the workspace root")
        .to_path_buf()
}

fn usage() -> String {
    "usage: cargo xtask <gen-runtime [--check] | capabilities [--check] | conformance | release-stages>".to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        capability_matrix, check_conformance_layout, check_release_stages,
        generate_capability_matrix, generate_runtime, validate_release_stages,
    };

    #[test]
    fn generated_runtime_is_current() {
        generate_runtime(true).expect("committed runtime operations must be current");
    }

    #[test]
    fn generated_capability_matrix_is_current() {
        generate_capability_matrix(true).expect("committed capability matrix must be current");
    }

    #[test]
    fn capability_matrix_follows_the_standard_registry() {
        let generated = capability_matrix().unwrap();
        assert!(generated.contains("| ruby |"));
        assert!(generated.contains("| php |"));
        assert!(generated.contains("| perl |"));
        assert!(generated.contains("textures-v1"));
    }

    #[test]
    fn conformance_layers_are_present() {
        check_conformance_layout().expect("all conformance layers must be present");
    }

    #[test]
    fn release_stages_follow_normal_dependency_order() {
        check_release_stages().expect("release stages must follow the crate dependency DAG");
    }

    #[test]
    fn release_stage_validation_rejects_a_reversed_dependency() {
        let metadata = json!({
            "packages": [
                {"name": "polygl-a", "publish": null, "dependencies": []},
                {"name": "polygl-b", "publish": null, "dependencies": [
                    {"name": "polygl-a", "kind": null, "path": "/workspace/a"}
                ]}
            ]
        });
        let error = validate_release_stages("polygl-b\npolygl-a\n", &metadata).unwrap_err();
        assert!(error.contains("polygl-b"));
        assert!(error.contains("polygl-a"));
        assert!(error.contains("non-earlier stage"));
    }
}
