use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn independent_processes_produce_byte_identical_release_trees() {
    let temporary = tempfile::tempdir().unwrap();
    let source = temporary.path().join("deterministic.rb");
    fs::write(
        &source,
        "def setup\n  size(8, 8)\n  background(0.25, 0.5, 0.75)\nend\n",
    )
    .unwrap();
    let first = temporary.path().join("first");
    let second = temporary.path().join("second");

    build_in_process(&source, &first, "1");
    build_in_process(&source, &second, "4102444800");

    let first_files = collect_files(&first);
    let second_files = collect_files(&second);
    assert_eq!(
        first_files.keys().collect::<Vec<_>>(),
        second_files.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in first_files {
        assert_eq!(
            bytes,
            second_files[&path],
            "artifact `{}` differed between independent compiler processes",
            path.display()
        );
    }
}

fn build_in_process(source: &Path, output: &Path, source_date_epoch: &str) {
    let result = Command::new(env!("CARGO_BIN_EXE_polygl"))
        .current_dir(source.parent().unwrap())
        .args(["build"])
        .arg(source)
        .args(["--release", "--source-map", "none", "-o"])
        .arg(output)
        .env("SOURCE_DATE_EPOCH", source_date_epoch)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "separate build process failed:\n{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn collect_files(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_directory(root, root, &mut files);
    files
}

fn collect_directory(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<std::path::PathBuf, Vec<u8>>,
) {
    let mut entries = fs::read_dir(directory)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_directory(root, &path, files);
        } else {
            files.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                fs::read(path).unwrap(),
            );
        }
    }
}
