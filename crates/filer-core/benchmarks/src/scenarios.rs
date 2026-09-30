//! # Version 1 scenario rules
//!
//! This module expands a validated request into the exact action sequence and
//! milestone set that a trace may claim. Keeping these rules separate makes
//! the trace state machine responsible for progress, not scenario knowledge.

use crate::manifests::ValidatedManifest;
use crate::schema::{Field, Filter, Phase, ProcessCache, Request, SemanticCache, Sort};
use crate::{ErrorCode, ProtocolError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ActionKind {
    Open {
        page_number: u64,
        row_first: bool,
        listing: ListingKind,
    },
    Page {
        page_number: u64,
    },
    SortName,
    FilterName,
    ClearFilter,
    Refresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ListingKind {
    None,
    Membership,
    Metadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionPlan {
    pub(crate) id: String,
    pub(crate) kind: ActionKind,
    pub(crate) required: Vec<Phase>,
    pub(crate) optional: Vec<Phase>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScenarioKind {
    FastFirst,
    FastScale,
    MetadataFirst,
    Continuation,
    NameSort,
    NameFilter,
    Refresh,
    ReferenceJourney,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScenarioPlan {
    pub(crate) kind: ScenarioKind,
    pub(crate) actions: Vec<ActionPlan>,
    pub(crate) first_page_gate: bool,
}

pub(crate) fn validate_request(
    request: &Request,
    manifest: &ValidatedManifest,
) -> Result<ScenarioPlan, ProtocolError> {
    if request.fixture.id != manifest.id() || request.fixture.digest != manifest.manifest_digest() {
        return Err(ProtocolError::new(
            ErrorCode::FixtureReferenceMismatch,
            "request fixture does not match the selected validated manifest",
        ));
    }
    if request.viewport_size != 40 || request.page_size != 256 {
        return Err(ProtocolError::new(
            ErrorCode::InvalidScenarioConfiguration,
            "version 1 scenarios require viewport_size 40 and page_size 256",
        ));
    }
    let plan = match request.scenario_id.as_str() {
        "browse.fast.first" => ScenarioPlan {
            kind: ScenarioKind::FastFirst,
            actions: vec![open_action(1, true, ListingKind::Membership)],
            first_page_gate: true,
        },
        "browse.fast.scale" => ScenarioPlan {
            kind: ScenarioKind::FastScale,
            actions: vec![open_action(1, false, ListingKind::Membership)],
            first_page_gate: true,
        },
        "browse.metadata.first" => ScenarioPlan {
            kind: ScenarioKind::MetadataFirst,
            actions: vec![open_action(1, false, ListingKind::Metadata)],
            first_page_gate: true,
        },
        "browse.next" => ScenarioPlan {
            kind: ScenarioKind::Continuation,
            actions: continuation_actions(false),
            first_page_gate: true,
        },
        "view.sort.name" => ScenarioPlan {
            kind: ScenarioKind::NameSort,
            actions: vec![transform_action("sort-name", Phase::TransformCompleted)],
            first_page_gate: false,
        },
        "view.filter.common" => ScenarioPlan {
            kind: ScenarioKind::NameFilter,
            actions: vec![transform_action("filter-name", Phase::TransformCompleted)],
            first_page_gate: false,
        },
        "browse.refresh" => ScenarioPlan {
            kind: ScenarioKind::Refresh,
            actions: vec![refresh_action("refresh")],
            first_page_gate: false,
        },
        "journey.browse-reference" => ScenarioPlan {
            kind: ScenarioKind::ReferenceJourney,
            actions: continuation_actions(true),
            first_page_gate: true,
        },
        _ => {
            return Err(ProtocolError::new(
                ErrorCode::InvalidScenarioConfiguration,
                "scenario_id is not a version 1 scenario",
            ));
        }
    };
    validate_request_shape(request, &plan.kind)?;
    Ok(plan)
}

fn validate_request_shape(request: &Request, kind: &ScenarioKind) -> Result<(), ProtocolError> {
    let expected_fixture = match kind {
        ScenarioKind::FastScale => "flat-100k-v1",
        _ => "flat-10k-v1",
    };
    if request.fixture.id != expected_fixture {
        return Err(ProtocolError::new(
            ErrorCode::InvalidScenarioConfiguration,
            "fixture does not match the scenario",
        ));
    }
    let expected_fields = match kind {
        ScenarioKind::MetadataFirst => vec![
            Field::Identity,
            Field::Kind,
            Field::SizeBytes,
            Field::ModifiedUnixNs,
        ],
        _ => vec![Field::Identity, Field::Kind],
    };
    if request.requested_fields != expected_fields {
        return Err(ProtocolError::new(
            ErrorCode::InvalidScenarioConfiguration,
            "requested_fields do not match the scenario",
        ));
    }
    let expected_sort = match kind {
        ScenarioKind::NameSort | ScenarioKind::NameFilter | ScenarioKind::Refresh => {
            Sort::NameAscending
        }
        _ => Sort::ProviderOrder,
    };
    if request.sort != expected_sort {
        return Err(ProtocolError::new(
            ErrorCode::InvalidScenarioConfiguration,
            "sort does not match the scenario",
        ));
    }
    let expected_filter = match kind {
        ScenarioKind::NameFilter => Filter::NameContains {
            value: "file-0001".to_string(),
            case_sensitive: true,
        },
        _ => Filter::None,
    };
    if request.filter != expected_filter {
        return Err(ProtocolError::new(
            ErrorCode::InvalidScenarioConfiguration,
            "filter does not match the scenario",
        ));
    }
    match kind {
        ScenarioKind::FastFirst
            if request.cache.process != ProcessCache::Cold
                || request.cache.semantic != SemanticCache::Empty =>
        {
            return Err(ProtocolError::new(
                ErrorCode::InvalidScenarioConfiguration,
                "browse.fast.first requires a cold process and empty semantic cache",
            ));
        }
        ScenarioKind::NameSort | ScenarioKind::NameFilter | ScenarioKind::Refresh
            if request.cache.process != ProcessCache::Warm
                || request.cache.semantic != SemanticCache::Reused =>
        {
            return Err(ProtocolError::new(
                ErrorCode::InvalidScenarioConfiguration,
                "snapshot scenarios require a warm process and reused semantic cache",
            ));
        }
        ScenarioKind::ReferenceJourney
            if request.cache.process != ProcessCache::Warm
                || request.cache.semantic != SemanticCache::Empty =>
        {
            return Err(ProtocolError::new(
                ErrorCode::InvalidScenarioConfiguration,
                "the reference journey requires a warm process and empty semantic cache",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn open_action(page_number: u64, row_first: bool, listing: ListingKind) -> ActionPlan {
    let mut required = vec![Phase::ViewportCommitted, Phase::PageCommitted];
    let mut optional = Vec::new();
    if row_first {
        required.insert(0, Phase::RowFirst);
    } else {
        optional.push(Phase::RowFirst);
    }
    if listing != ListingKind::None {
        required.push(Phase::ListingCompleted);
    }
    ActionPlan {
        id: "open".to_string(),
        kind: ActionKind::Open {
            page_number,
            row_first,
            listing,
        },
        required,
        optional,
    }
}

fn continuation_actions(journey: bool) -> Vec<ActionPlan> {
    let mut actions = vec![open_action(1, false, ListingKind::None)];
    for page_number in 2..=40 {
        actions.push(ActionPlan {
            id: format!("page-{page_number:04}"),
            kind: ActionKind::Page { page_number },
            required: if page_number == 40 {
                vec![Phase::PageCommitted, Phase::ListingCompleted]
            } else {
                vec![Phase::PageCommitted]
            },
            optional: Vec::new(),
        });
    }
    if journey {
        actions.extend([
            transform_action("sort-name", Phase::TransformCompleted),
            ActionPlan {
                id: "filter-name".to_string(),
                kind: ActionKind::FilterName,
                required: vec![Phase::TransformCompleted, Phase::ViewCommitted],
                optional: Vec::new(),
            },
            ActionPlan {
                id: "clear-filter".to_string(),
                kind: ActionKind::ClearFilter,
                required: vec![Phase::TransformCompleted, Phase::ViewCommitted],
                optional: Vec::new(),
            },
            refresh_action("refresh"),
        ]);
    }
    actions
}

fn transform_action(id: &str, _phase: Phase) -> ActionPlan {
    ActionPlan {
        id: id.to_string(),
        kind: if id == "filter-name" {
            ActionKind::FilterName
        } else if id == "clear-filter" {
            ActionKind::ClearFilter
        } else {
            ActionKind::SortName
        },
        required: vec![Phase::TransformCompleted, Phase::ViewCommitted],
        optional: Vec::new(),
    }
}

fn refresh_action(id: &str) -> ActionPlan {
    ActionPlan {
        id: id.to_string(),
        kind: ActionKind::Refresh,
        required: vec![
            Phase::ListingCompleted,
            Phase::TransformCompleted,
            Phase::ViewCommitted,
        ],
        optional: Vec::new(),
    }
}
