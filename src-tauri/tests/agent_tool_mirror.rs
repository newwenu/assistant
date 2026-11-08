// 参考: docs/store/I38b-agent-loop-spec.md §4.5 / A18 — 端口镜像一一对应契约

use assistant_agent::agent::round_executor::{ToolErrorClass, ToolStage};
use assistant_tools::pipeline::PipelineStage;
use assistant_tools::result::ToolErrorKind;

fn to_tools_stage(stage: ToolStage) -> PipelineStage {
    match stage {
        ToolStage::Find => PipelineStage::Find,
        ToolStage::Validate => PipelineStage::Validate,
        ToolStage::Gateway => PipelineStage::Gateway,
        ToolStage::Execute => PipelineStage::Execute,
        ToolStage::Persist => PipelineStage::Persist,
    }
}

fn to_agent_stage(stage: PipelineStage) -> ToolStage {
    match stage {
        PipelineStage::Find => ToolStage::Find,
        PipelineStage::Validate => ToolStage::Validate,
        PipelineStage::Gateway => ToolStage::Gateway,
        PipelineStage::Execute => ToolStage::Execute,
        PipelineStage::Persist => ToolStage::Persist,
    }
}

fn to_tools_error(kind: ToolErrorClass) -> ToolErrorKind {
    match kind {
        ToolErrorClass::Business => ToolErrorKind::Business,
        ToolErrorClass::Timeout => ToolErrorKind::Timeout,
        ToolErrorClass::Cancelled => ToolErrorKind::Cancelled,
        ToolErrorClass::Internal => ToolErrorKind::Internal,
        ToolErrorClass::Denied => ToolErrorKind::Denied,
    }
}

fn to_agent_error(kind: ToolErrorKind) -> ToolErrorClass {
    match kind {
        ToolErrorKind::Business => ToolErrorClass::Business,
        ToolErrorKind::Timeout => ToolErrorClass::Timeout,
        ToolErrorKind::Cancelled => ToolErrorClass::Cancelled,
        ToolErrorKind::Internal => ToolErrorClass::Internal,
        ToolErrorKind::Denied => ToolErrorClass::Denied,
    }
}

const AGENT_STAGES: [ToolStage; 5] = [
    ToolStage::Find,
    ToolStage::Validate,
    ToolStage::Gateway,
    ToolStage::Execute,
    ToolStage::Persist,
];

const TOOLS_STAGES: [PipelineStage; 5] = [
    PipelineStage::Find,
    PipelineStage::Validate,
    PipelineStage::Gateway,
    PipelineStage::Execute,
    PipelineStage::Persist,
];

const AGENT_ERRORS: [ToolErrorClass; 5] = [
    ToolErrorClass::Business,
    ToolErrorClass::Timeout,
    ToolErrorClass::Cancelled,
    ToolErrorClass::Internal,
    ToolErrorClass::Denied,
];

const TOOLS_ERRORS: [ToolErrorKind; 5] = [
    ToolErrorKind::Business,
    ToolErrorKind::Timeout,
    ToolErrorKind::Cancelled,
    ToolErrorKind::Internal,
    ToolErrorKind::Denied,
];

#[test]
fn stage_mirror_roundtrips_every_variant() {
    for stage in AGENT_STAGES {
        assert_eq!(
            to_agent_stage(to_tools_stage(stage)),
            stage,
            "agent -> tools -> agent must be identity for {stage:?}"
        );
    }
}

#[test]
fn stage_mirror_covers_the_same_variant_set() {
    let mapped: Vec<PipelineStage> = AGENT_STAGES.iter().copied().map(to_tools_stage).collect();
    for stage in TOOLS_STAGES {
        assert!(
            mapped.contains(&stage),
            "{stage:?} has no agent-side mirror; the two enums drifted"
        );
    }
    assert_eq!(
        mapped.len(),
        TOOLS_STAGES.len(),
        "mirror must stay one-to-one, not many-to-one"
    );
}

#[test]
fn error_class_mirror_roundtrips_every_variant() {
    for kind in AGENT_ERRORS {
        assert_eq!(
            to_agent_error(to_tools_error(kind)),
            kind,
            "agent -> tools -> agent must be identity for {kind:?}"
        );
    }
}

#[test]
fn error_class_mirror_covers_the_same_variant_set() {
    let mapped: Vec<ToolErrorKind> = AGENT_ERRORS.iter().copied().map(to_tools_error).collect();
    for kind in TOOLS_ERRORS {
        assert!(
            mapped.contains(&kind),
            "{kind:?} has no agent-side mirror; the two enums drifted"
        );
    }
    assert_eq!(
        mapped.len(),
        TOOLS_ERRORS.len(),
        "mirror must stay one-to-one, not many-to-one"
    );
}
