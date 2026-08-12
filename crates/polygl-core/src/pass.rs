use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileStage {
    Source,
    LoweredHir,
    TypedHir,
    DomainResolvedLir,
    SplitProgram,
    JavaScript,
    Glsl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassTrace {
    pub name: &'static str,
    pub input: CompileStage,
    pub output: CompileStage,
    pub elapsed: Duration,
    pub changed: bool,
}

#[derive(Default)]
pub(crate) struct PassManager {
    trace: Vec<PassTrace>,
}

impl PassManager {
    pub(crate) fn run<T, E>(
        &mut self,
        name: &'static str,
        input: CompileStage,
        output: CompileStage,
        changed: bool,
        pass: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let start = Instant::now();
        let result = pass();
        self.trace.push(PassTrace {
            name,
            input,
            output,
            elapsed: start.elapsed(),
            changed,
        });
        result
    }

    pub(crate) fn finish(self) -> Vec<PassTrace> {
        self.trace
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileStage, PassManager};

    #[test]
    fn records_failed_passes_in_deterministic_execution_order() {
        let mut manager = PassManager::default();
        let _: Result<(), &str> = manager.run(
            "adapter.lower",
            CompileStage::Source,
            CompileStage::LoweredHir,
            true,
            || Err("failed"),
        );
        let trace = manager.finish();
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].name, "adapter.lower");
        assert_eq!(trace[0].input, CompileStage::Source);
        assert_eq!(trace[0].output, CompileStage::LoweredHir);
    }
}
