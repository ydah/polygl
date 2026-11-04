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

    let assets = temporary.path().join("assets");
    fs::create_dir(&assets).unwrap();
    fs::write(assets.join("picture.png"), b"texture").unwrap();
    fs::write(assets.join("original.psd"), b"source material").unwrap();
    fs::write(
        temporary.path().join("sketch.rb"),
        "def setup\n  texture_load(\"assets/picture.png\")\nend\n",
    )
    .unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_polygl"))
        .current_dir(temporary.path())
        .args(["build", "sketch.rb", "-o", "assets"])
        .output()
        .unwrap();

    assert!(!result.status.success());
    assert_eq!(fs::read(assets.join("picture.png")).unwrap(), b"texture");
    assert_eq!(
        fs::read(assets.join("original.psd")).unwrap(),
        b"source material"
    );
}
