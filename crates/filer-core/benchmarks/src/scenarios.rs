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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

/// The request fields a version 1 scenario fixes, independent of cache state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequestSettings {
    pub(crate) fixture_id: &'static str,
    pub(crate) requested_fields: Vec<Field>,
    pub(crate) sort: Sort,
    pub(crate) filter: Filter,
}

impl ScenarioKind {
    pub(crate) fn from_id(scenario_id: &str) -> Option<Self> {
        Some(match scenario_id {
            "browse.fast.first" => Self::FastFirst,
            "browse.fast.scale" => Self::FastScale,
            "browse.metadata.first" => Self::MetadataFirst,
            "browse.next" => Self::Continuation,
            "view.sort.name" => Self::NameSort,
            "view.filter.common" => Self::NameFilter,
            "browse.refresh" => Self::Refresh,
            "journey.browse-reference" => Self::ReferenceJourney,
            _ => return None,
        })
    }

    fn plan(self) -> ScenarioPlan {
        let (actions, first_page_gate) = match self {
            Self::FastFirst => (vec![open_action(1, true, ListingKind::Membership)], true),
            Self::FastScale => (vec![open_action(1, false, ListingKind::Membership)], true),
            Self::MetadataFirst => (vec![open_action(1, false, ListingKind::Metadata)], true),
            Self::Continuation => (continuation_actions(false), true),
            Self::NameSort => (vec![transform_action("sort-name")], false),
            Self::NameFilter => (vec![transform_action("filter-name")], false),
            Self::Refresh => (vec![refresh_action("refresh")], false),
            Self::ReferenceJourney => (continuation_actions(true), true),
        };
        ScenarioPlan {
            kind: self,
            actions,
            first_page_gate,
        }
    }

    pub(crate) fn request_settings(self) -> RequestSettings {
        RequestSettings {
            fixture_id: match self {
                Self::FastScale => "flat-100k-v1",
                _ => "flat-10k-v1",
            },
            requested_fields: match self {
                Self::MetadataFirst => Field::ALL.to_vec(),
                _ => vec![Field::Identity, Field::Kind],
            },
            sort: match self {
                Self::NameSort | Self::NameFilter | Self::Refresh => Sort::NameAscending,
                _ => Sort::ProviderOrder,
            },
            filter: match self {
                Self::NameFilter => Filter::NameContains {
                    value: "file-0001".to_string(),
                    case_sensitive: true,
                },
                _ => Filter::None,
            },
        }
    }
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
    let kind = ScenarioKind::from_id(&request.scenario_id).ok_or_else(|| {
        ProtocolError::new(
            ErrorCode::InvalidScenarioConfiguration,
            "scenario_id is not a version 1 scenario",
        )
    })?;
    let plan = kind.plan();
    validate_request_shape(request, &plan.kind)?;
    Ok(plan)
}

fn validate_request_shape(request: &Request, kind: &ScenarioKind) -> Result<(), ProtocolError> {
    let settings = kind.request_settings();
    let mismatch = if request.fixture.id != settings.fixture_id {
        Some("fixture")
    } else if request.requested_fields != settings.requested_fields {
        Some("requested_fields")
    } else if request.sort != settings.sort {
        Some("sort")
    } else if request.filter != settings.filter {
        Some("filter")
    } else {
        None
    };
    if let Some(field) = mismatch {
        return Err(ProtocolError::new(
            ErrorCode::InvalidScenarioConfiguration,
            format!("{field} does not match the scenario"),
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
            transform_action("sort-name"),
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

fn transform_action(id: &str) -> ActionPlan {
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
