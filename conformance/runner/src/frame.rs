use std::fs;
use std::path::PathBuf;

use crate::ConformanceError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedFrame {
    pub renderer: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct L1BaselineStore {
    root: PathBuf,
}

impl L1BaselineStore {
    #[must_use]
    pub fn new(conformance_root: impl Into<PathBuf>) -> Self {
        Self {
            root: conformance_root.into(),
        }
    }

    pub fn verify(&self, case: &str, actual: &RenderedFrame) -> Result<(), ConformanceError> {
        validate_name(case)?;
        validate_name(&actual.renderer)?;
        let path = self
            .root
            .join("l1-render")
            .join(case)
            .join(format!("{}.rgba", actual.renderer));
        let expected = parse_baseline(&actual.renderer, &fs::read_to_string(path)?)?;
        compare_frames(&expected, actual)
    }
}

impl RenderedFrame {
    pub fn validate(&self) -> Result<(), ConformanceError> {
        let pixels = self
            .width
            .checked_mul(self.height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| ConformanceError::InvalidFrame("dimensions overflow".to_owned()))?;
        let expected_len = usize::try_from(pixels)
            .map_err(|_| ConformanceError::InvalidFrame("dimensions exceed usize".to_owned()))?;
        if self.rgba.len() != expected_len {
            return Err(ConformanceError::InvalidFrame(format!(
                "expected {expected_len} RGBA bytes, got {}",
                self.rgba.len()
            )));
        }
        Ok(())
    }
}

pub fn compare_frames(
    expected: &RenderedFrame,
    actual: &RenderedFrame,
) -> Result<(), ConformanceError> {
    expected.validate()?;
    actual.validate()?;
    if expected != actual {
        return Err(ConformanceError::FrameMismatch {
            expected_renderer: expected.renderer.clone(),
            actual_renderer: actual.renderer.clone(),
        });
    }
    Ok(())
}

fn parse_baseline(renderer: &str, contents: &str) -> Result<RenderedFrame, ConformanceError> {
    let mut lines = contents.lines();
    let dimensions = lines
        .next()
        .and_then(|line| line.split_once('x'))
        .ok_or_else(|| ConformanceError::InvalidFrame("missing WIDTHxHEIGHT header".to_owned()))?;
    let width = dimensions
        .0
        .parse()
        .map_err(|_| ConformanceError::InvalidFrame("invalid width".to_owned()))?;
    let height = dimensions
        .1
        .parse()
        .map_err(|_| ConformanceError::InvalidFrame("invalid height".to_owned()))?;
    let hex = lines
        .next()
        .ok_or_else(|| ConformanceError::InvalidFrame("missing RGBA hex data".to_owned()))?;
    if lines.next().is_some()
        || hex.len() % 2 != 0
        || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ConformanceError::InvalidFrame(
            "RGBA baseline has an invalid shape".to_owned(),
        ));
    }
    let rgba = (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .map_err(|_| ConformanceError::InvalidFrame("invalid RGBA hex byte".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let frame = RenderedFrame {
        renderer: renderer.to_owned(),
        width,
        height,
        rgba,
    };
    frame.validate()?;
    Ok(frame)
}

fn validate_name(value: &str) -> Result<(), ConformanceError> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || "-_".contains(char));
    if valid {
        Ok(())
    } else {
        Err(ConformanceError::InvalidName(value.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::{RenderedFrame, compare_frames, parse_baseline};

    #[test]
    fn l1_compares_renderer_keyed_rgba_frames() {
        let frame = RenderedFrame {
            renderer: "swiftshader".to_owned(),
            width: 1,
            height: 1,
            rgba: vec![12, 34, 56, 255],
        };
        compare_frames(&frame, &frame).unwrap();

        let mut changed = frame.clone();
        changed.rgba[0] = 13;
        assert!(compare_frames(&frame, &changed).is_err());
    }

    #[test]
    fn parses_renderer_keyed_text_baselines() {
        let frame = parse_baseline("swiftshader", "1x1\n0c2238ff\n").unwrap();
        assert_eq!(frame.rgba, [12, 34, 56, 255]);
        assert!(parse_baseline("swiftshader", "1x1\nnot-hex\n").is_err());
        assert!(parse_baseline("swiftshader", "1x1\na€\n").is_err());
    }
}
