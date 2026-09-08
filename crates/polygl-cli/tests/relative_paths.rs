use std::fs;
use std::process::Command;

#[test]
fn builds_with_bare_relative_source_and_output_paths() {
    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("sketch.rb"), "def setup\nend\n").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_polygl"))
        .current_dir(temporary.path())
        .args(["build", "sketch.rb", "-o", "dist"])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "build failed:\n{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(temporary.path().join("dist/index.html").is_file());
}
