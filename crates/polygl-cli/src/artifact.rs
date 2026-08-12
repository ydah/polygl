use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use polygl_lir::AssetReference;
use tempfile::TempDir;
use unicode_normalization::UnicodeNormalization;

use crate::CliError;

#[derive(Debug)]
pub(crate) struct ArtifactFile {
    pub(crate) relative_path: PathBuf,
    pub(crate) contents: Vec<u8>,
}

impl ArtifactFile {
    pub(crate) fn new(relative_path: impl Into<PathBuf>, contents: impl Into<Vec<u8>>) -> Self {
        Self {
            relative_path: relative_path.into(),
            contents: contents.into(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedAssets {
    pub(crate) files: Vec<ArtifactFile>,
    pub(crate) source_paths: Vec<PathBuf>,
}

pub(crate) fn prepare_assets(
    source_path: &Path,
    references: &[AssetReference],
) -> Result<PreparedAssets, CliError> {
    let source_directory = source_path.parent().unwrap_or_else(|| Path::new("."));
    let canonical_source_directory = source_directory.canonicalize().map_err(|error| {
        CliError::new(format!(
            "failed to resolve source directory {}: {error}",
            source_directory.display()
        ))
    })?;
    let mut files = Vec::with_capacity(references.len());
    let mut source_paths = Vec::with_capacity(references.len());

    for reference in references {
        let relative_path = reference.path.split('/').collect::<PathBuf>();
        let input_path = source_directory.join(&relative_path);
        let canonical_input = input_path.canonicalize().map_err(|error| {
            CliError::new(format!(
                "failed to read texture asset {} referenced as `{}`: {error}",
                input_path.display(),
                reference.path,
            ))
        })?;
        if !canonical_input.starts_with(&canonical_source_directory) {
            return Err(CliError::new(format!(
                "texture asset `{}` resolves outside source directory {}",
                reference.path,
                source_directory.display(),
            )));
        }
        let metadata = canonical_input.metadata().map_err(|error| {
            CliError::new(format!(
                "failed to inspect texture asset {}: {error}",
                input_path.display()
            ))
        })?;
        if !metadata.is_file() {
            return Err(CliError::new(format!(
                "texture asset {} referenced as `{}` is not a regular file",
                input_path.display(),
                reference.path,
            )));
        }
        let contents = fs::read(&canonical_input).map_err(|error| {
            CliError::new(format!(
                "failed to read texture asset {} referenced as `{}`: {error}",
                input_path.display(),
                reference.path,
            ))
        })?;
        source_paths.push(input_path);
        files.push(ArtifactFile::new(relative_path, contents));
    }

    Ok(PreparedAssets {
        files,
        source_paths,
    })
}

pub(crate) fn publish(destination: &Path, files: Vec<ArtifactFile>) -> Result<(), CliError> {
    validate_paths(&files)?;
    let (parent, target) = resolve_destination(destination)?;
    reject_unsafe_existing_destination(&target)?;

    let staging = tempfile::Builder::new()
        .prefix(".polygl-stage-")
        .tempdir_in(&parent)
        .map_err(|error| {
            CliError::new(format!(
                "failed to create build staging directory in {}: {error}",
                parent.display()
            ))
        })?;
    write_files(staging.path(), files)?;
    replace_destination(staging, &target, &parent)
}

fn resolve_destination(destination: &Path) -> Result<(PathBuf, PathBuf), CliError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        CliError::new(format!(
            "failed to create output parent directory {}: {error}",
            parent.display()
        ))
    })?;
    let parent = parent.canonicalize().map_err(|error| {
        CliError::new(format!(
            "failed to resolve output parent directory {}: {error}",
            parent.display()
        ))
    })?;
    let name = destination.file_name().ok_or_else(|| {
        CliError::new(format!(
            "output path {} must name a directory below its parent",
            destination.display()
        ))
    })?;
    Ok((parent.clone(), parent.join(name)))
}

fn reject_unsafe_existing_destination(target: &Path) -> Result<(), CliError> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(CliError::new(format!(
                "failed to inspect output path {}: {error}",
                target.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(CliError::new(format!(
            "refusing to replace symlink output directory {}",
            target.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(CliError::new(format!(
            "output path {} exists and is not a directory",
            target.display()
        )));
    }
    Ok(())
}

fn write_files(root: &Path, files: Vec<ArtifactFile>) -> Result<(), CliError> {
    for file in files {
        let output_path = root.join(&file.relative_path);
        let parent = output_path.parent().ok_or_else(|| {
            CliError::new(format!(
                "build artifact {} has no parent directory",
                file.relative_path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|error| {
            CliError::new(format!(
                "failed to create artifact directory {}: {error}",
                parent.display(),
            ))
        })?;
        fs::write(&output_path, &file.contents).map_err(|error| {
            CliError::new(format!(
                "failed to write build artifact {}: {error}",
                output_path.display()
            ))
        })?;
    }
    Ok(())
}

fn replace_destination(staging: TempDir, target: &Path, parent: &Path) -> Result<(), CliError> {
    if !target.exists() {
        return install_staging(staging, target);
    }

    let backup = tempfile::Builder::new()
        .prefix(".polygl-backup-")
        .tempdir_in(parent)
        .map_err(|error| {
            CliError::new(format!(
                "failed to create build backup directory in {}: {error}",
                parent.display()
            ))
        })?;
    let previous = backup.path().join("previous");
    fs::rename(target, &previous).map_err(|error| {
        CliError::new(format!(
            "failed to preserve previous build {}: {error}",
            target.display()
        ))
    })?;

    if let Err(install_error) = install_staging(staging, target) {
        return match fs::rename(&previous, target) {
            Ok(()) => Err(install_error),
            Err(restore_error) => Err(CliError::new(format!(
                "{install_error}; additionally failed to restore the previous build: {restore_error}"
            ))),
        };
    }
    Ok(())
}

fn install_staging(staging: TempDir, target: &Path) -> Result<(), CliError> {
    fs::rename(staging.path(), target).map_err(|error| {
        CliError::new(format!(
            "failed to activate staged build at {}: {error}",
            target.display()
        ))
    })?;
    let _ = staging.keep();
    Ok(())
}

#[derive(Default)]
struct PathNode {
    terminal: Option<String>,
    children: BTreeMap<String, PathNode>,
}

fn validate_paths(files: &[ArtifactFile]) -> Result<(), CliError> {
    let mut root = PathNode::default();
    for file in files {
        let display = portable_relative_path(&file.relative_path)?;
        let components = display
            .split('/')
            .map(portable_component_key)
            .collect::<Result<Vec<_>, CliError>>()?;
        insert_path(&mut root, &components, &display)?;
    }
    Ok(())
}

fn portable_relative_path(path: &Path) -> Result<String, CliError> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(CliError::new(format!(
                "build artifact path {} is not a portable relative path",
                path.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            CliError::new(format!(
                "build artifact path {} is not valid UTF-8",
                path.display()
            ))
        })?;
        components.push(component);
    }
    if components.is_empty() {
        return Err(CliError::new("build artifact path may not be empty"));
    }
    Ok(components.join("/"))
}

fn portable_component_key(component: &str) -> Result<String, CliError> {
    if component.ends_with(['.', ' ']) {
        return Err(CliError::new(format!(
            "build artifact component `{component}` may not end in a dot or space"
        )));
    }
    if component.chars().any(|character| {
        character.is_control()
            || matches!(character, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*')
    }) {
        return Err(CliError::new(format!(
            "build artifact component `{component}` contains a Windows-incompatible character"
        )));
    }
    let normalized = component.nfc().collect::<String>().to_lowercase();
    let device_name = normalized.split('.').next().unwrap_or_default();
    if is_windows_device_name(device_name) {
        return Err(CliError::new(format!(
            "build artifact component `{component}` is a reserved Windows device name"
        )));
    }
    Ok(normalized)
}

fn is_windows_device_name(name: &str) -> bool {
    matches!(name, "con" | "prn" | "aux" | "nul" | "conin$" | "conout$")
        || name
            .strip_prefix("com")
            .or_else(|| name.strip_prefix("lpt"))
            .is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
}

fn insert_path(node: &mut PathNode, components: &[String], display: &str) -> Result<(), CliError> {
    let mut current = node;
    for component in components {
        if let Some(existing) = &current.terminal {
            return Err(path_collision(existing, display));
        }
        current = current.children.entry(component.clone()).or_default();
    }
    if let Some(existing) = &current.terminal {
        return Err(path_collision(existing, display));
    }
    if let Some(existing) = first_terminal(current) {
        return Err(path_collision(existing, display));
    }
    current.terminal = Some(display.to_owned());
    Ok(())
}

fn first_terminal(node: &PathNode) -> Option<&str> {
    node.terminal
        .as_deref()
        .or_else(|| node.children.values().find_map(first_terminal))
}

fn path_collision(first: &str, second: &str) -> CliError {
    CliError::new(format!(
        "build artifact paths `{first}` and `{second}` collide on a supported platform"
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use polygl_lir::AssetReference;
    use polygl_span::{SourceFile, SourceId};

    use super::{ArtifactFile, prepare_assets, publish, validate_paths};

    #[test]
    fn rejects_platform_and_prefix_collisions() {
        for paths in [
            vec!["A.png", "a.png"],
            vec!["caf\u{e9}.png", "cafe\u{301}.png"],
            vec!["foo", "foo/bar.png"],
            vec!["app.js", "app.js/texture.png"],
        ] {
            let files = paths
                .into_iter()
                .map(|path| ArtifactFile::new(path, Vec::new()))
                .collect::<Vec<_>>();
            assert!(validate_paths(&files).is_err());
        }
        for path in [
            "CON",
            "nul.png",
            "COM¹.txt",
            "lpt²",
            "CONIN$",
            "conout$.log",
            "folder./image.png",
            "bad?.png",
        ] {
            assert!(validate_paths(&[ArtifactFile::new(path, Vec::new())]).is_err());
        }
    }

    #[test]
    fn replaces_complete_generations_and_removes_stale_files() {
        let temporary = tempfile::tempdir().unwrap();
        let output = temporary.path().join("dist");
        publish(
            &output,
            vec![
                ArtifactFile::new("app.js", b"old".to_vec()),
                ArtifactFile::new("stale.png", b"stale".to_vec()),
            ],
        )
        .unwrap();
        publish(&output, vec![ArtifactFile::new("app.js", b"new".to_vec())]).unwrap();
        assert_eq!(fs::read(output.join("app.js")).unwrap(), b"new");
        assert!(!output.join("stale.png").exists());
    }

    #[test]
    fn validation_failure_preserves_the_previous_generation() {
        let temporary = tempfile::tempdir().unwrap();
        let output = temporary.path().join("dist");
        publish(
            &output,
            vec![ArtifactFile::new("sentinel.txt", b"previous".to_vec())],
        )
        .unwrap();
        let error = publish(
            &output,
            vec![
                ArtifactFile::new("same", Vec::new()),
                ArtifactFile::new("same/file", Vec::new()),
            ],
        )
        .unwrap_err();
        assert!(error.to_string().contains("collide"));
        assert_eq!(fs::read(output.join("sentinel.txt")).unwrap(), b"previous");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlink_destination_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let target = temporary.path().join("outside");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("sentinel"), "safe").unwrap();
        let output = temporary.path().join("dist");
        symlink(&target, &output).unwrap();

        let error = publish(
            &output,
            vec![ArtifactFile::new("app.js", b"unsafe".to_vec())],
        )
        .unwrap_err();
        assert!(error.to_string().contains("symlink"));
        assert_eq!(fs::read_to_string(target.join("sentinel")).unwrap(), "safe");
        assert!(!target.join("app.js").exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_asset_symlink_that_escapes_the_source_directory() {
        use std::os::unix::fs::symlink;

        let temporary = tempfile::tempdir().unwrap();
        let source_directory = temporary.path().join("source");
        fs::create_dir(&source_directory).unwrap();
        let source_path = source_directory.join("main.rb");
        fs::write(&source_path, "").unwrap();
        let outside = temporary.path().join("secret.png");
        fs::write(&outside, "secret").unwrap();
        symlink(&outside, source_directory.join("escape.png")).unwrap();
        let source = SourceFile::new(SourceId::new(0), "main.rb", "");
        let reference = AssetReference {
            path: "escape.png".to_owned(),
            span: source.span(0, 0).unwrap(),
        };

        let error = prepare_assets(&source_path, &[reference]).unwrap_err();
        assert!(error.to_string().contains("outside source directory"));
    }
}
