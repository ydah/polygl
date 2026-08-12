use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn every_example_builds_in_debug_and_release_modes() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("CLI crate lives under the workspace crates directory")
        .to_owned();
    let examples = workspace.join("examples");
    let mut sources = fs::read_dir(&examples)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    sources.sort_by_key(std::fs::DirEntry::file_name);
    assert!(
        !sources.is_empty(),
        "the example inventory must not be empty"
    );

    let temporary = tempfile::tempdir().unwrap();
    for source in sources {
        let source = source.path();
        if !source.is_file() {
            continue;
        }
        let extension = source.extension().and_then(|value| value.to_str());
        assert!(
            matches!(extension, Some("rb" | "php" | "pl")),
            "unrecognized example file must be explicitly handled: {}",
            source.display()
        );
        for mode in ["debug", "release"] {
            let output = temporary.path().join(format!(
                "{}-{mode}",
                source.file_name().unwrap().to_string_lossy()
            ));
            let result = Command::new(env!("CARGO_BIN_EXE_polygl"))
                .args(["build"])
                .arg(&source)
                .arg(format!("--{mode}"))
                .args(["-o"])
                .arg(&output)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "example {} failed in {mode} mode:\n{}{}",
                source.display(),
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            for artifact in [
                "app.js",
                "index.html",
                "polygl-manifest.json",
                "runtime.js",
                "shaders.js",
            ] {
                assert!(
                    output.join(artifact).is_file(),
                    "example {} omitted {artifact} in {mode} mode",
                    source.display()
                );
            }
            assert_eq!(
                output.join("app.js.map").is_file(),
                mode == "debug",
                "example {} has the wrong source-map policy in {mode} mode",
                source.display()
            );
            let manifest: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(output.join("polygl-manifest.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(manifest["options"]["mode"], mode);
        }
    }
}
