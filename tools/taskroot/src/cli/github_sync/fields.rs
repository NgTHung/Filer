use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{error::TaskError, model::Task};

use super::api::invalid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Field {
    id: String,
    name: String,
    data_type: String,
    #[serde(default)]
    options: Vec<SelectOption>,
}

#[derive(Deserialize)]
struct SelectOption {
    id: String,
    name: String,
}

#[derive(Serialize)]
pub(super) struct FieldChange {
    name: &'static str,
    target: String,
    #[serde(skip)]
    id: String,
    #[serde(skip)]
    value: Option<Value>,
}

pub(super) fn changes(
    fields: &[Field],
    task: &Task,
    remote: Option<&Value>,
) -> Result<Vec<FieldChange>, TaskError> {
    let values = remote
        .map(|item| {
            if item["fieldValues"]["pageInfo"]["hasNextPage"] != false {
                return Err(invalid("incomplete project item fields"));
            }
            item["fieldValues"]["nodes"]
                .as_array()
                .ok_or_else(|| invalid("missing item fields"))
        })
        .transpose()?;
    let mut changes = Vec::new();
    for (name, target, kind) in [
        ("Status", task.metadata.status.to_string(), "SINGLE_SELECT"),
        (
            "Priority",
            task.metadata.priority.to_string(),
            "SINGLE_SELECT",
        ),
        ("Taskroot ID", task.qualified_id(), "TEXT"),
        ("Domain", task.domain.clone(), "TEXT"),
        (
            "Taskroot Milestone",
            task.metadata.milestone.clone().unwrap_or_default(),
            "TEXT",
        ),
    ] {
        let matching: Vec<_> = fields.iter().filter(|field| field.name == name).collect();
        let [field] = matching.as_slice() else {
            return Err(invalid(format!(
                "expected exactly one {name} field; configure project fields before applying"
            )));
        };
        if field.data_type != kind {
            return Err(invalid(format!("{name} must be {kind}")));
        }
        let value = match kind {
            "SINGLE_SELECT" => {
                let options: Vec<_> = field
                    .options
                    .iter()
                    .filter(|option| option.name == target)
                    .collect();
                let [option] = options.as_slice() else {
                    return Err(invalid(format!(
                        "{name} needs exactly one option named {target}"
                    )));
                };
                Some(json!({"singleSelectOptionId":option.id}))
            }
            _ if target.is_empty() => None,
            _ => Some(json!({"text":target})),
        };
        let current =
            values.and_then(|values| values.iter().find(|value| value["field"]["id"] == field.id));
        let same = match (current, &value) {
            (Some(current), Some(value)) if kind == "SINGLE_SELECT" => {
                current["optionId"] == value["singleSelectOptionId"]
            }
            (Some(current), Some(value)) => current["text"] == value["text"],
            (None, None) => true,
            _ => false,
        };
        if !same {
            changes.push(FieldChange {
                name,
                target,
                id: field.id.clone(),
                value,
            });
        }
    }
    Ok(changes)
}

pub(super) fn update(
    project: &str,
    item: &str,
    changes: &[FieldChange],
    draft: Option<Value>,
    writer: &mut super::api::Writer,
) -> Result<(), TaskError> {
    if changes.is_empty() && draft.is_none() {
        return Ok(());
    }
    let mut declarations = Vec::new();
    let mut mutations = Vec::new();
    let mut variables = serde_json::Map::new();
    if let Some(draft) = &draft {
        declarations.push("$draft:UpdateProjectV2DraftIssueInput!".to_string());
        mutations.push("draft:updateProjectV2DraftIssue(input:$draft){draftIssue{id}}".to_string());
        variables.insert("draft".to_string(), draft.clone());
    }
    for (index, change) in changes.iter().enumerate() {
        let key = format!("field{index}");
        let mut input = json!({"projectId":project,"itemId":item,"fieldId":change.id});
        let (kind, mutation) = match &change.value {
            Some(value) => {
                input["value"] = value.clone();
                (
                    "UpdateProjectV2ItemFieldValueInput",
                    "updateProjectV2ItemFieldValue",
                )
            }
            None => (
                "ClearProjectV2ItemFieldValueInput",
                "clearProjectV2ItemFieldValue",
            ),
        };
        declarations.push(format!("${key}:{kind}!"));
        mutations.push(format!(
            "{key}:{mutation}(input:${key}){{projectV2Item{{id}}}}"
        ));
        variables.insert(key, input);
    }
    let query = format!(
        "mutation({}){{{}}}",
        declarations.join(","),
        mutations.join(" ")
    );
    let result = writer.write(&query, Value::Object(variables))?;
    if let Some(draft) = draft
        && result["draft"]["draftIssue"]["id"] != draft["draftIssueId"]
    {
        return Err(invalid(
            "incomplete draft update response; rerun to reconcile",
        ));
    }
    for index in 0..changes.len() {
        if result[format!("field{index}")]["projectV2Item"]["id"] != item {
            return Err(invalid(
                "incomplete field update response; rerun to reconcile",
            ));
        }
    }
    Ok(())
}
