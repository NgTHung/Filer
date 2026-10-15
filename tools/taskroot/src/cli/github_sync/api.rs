use std::{
    collections::HashSet,
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::error::TaskError;

const ITEMS: &str = "query($project:ID!,$cursor:String,$archived:[ProjectV2ItemArchivedState!]!){node(id:$project){... on ProjectV2{fields(first:100){nodes{... on ProjectV2FieldCommon{id name dataType} ... on ProjectV2SingleSelectField{options{id name}}} pageInfo{hasNextPage}} items(first:100,after:$cursor,archivedStates:$archived){nodes{id isArchived type content{__typename ... on DraftIssue{id title body} ... on Issue{id title body}} fieldValues(first:100){nodes{... on ProjectV2ItemFieldTextValue{text field{... on ProjectV2FieldCommon{id}}} ... on ProjectV2ItemFieldSingleSelectValue{optionId field{... on ProjectV2FieldCommon{id}}}} pageInfo{hasNextPage}}} pageInfo{hasNextPage endCursor}}}}}";

#[derive(Default)]
pub(super) struct Writer {
    last_write: Option<Instant>,
}

impl Writer {
    pub(super) fn write(&mut self, query: &str, variables: Value) -> Result<Value, TaskError> {
        if let Some(last) = self.last_write {
            std::thread::sleep(Duration::from_secs(1).saturating_sub(last.elapsed()));
        }
        let result = request(query, variables);
        self.last_write = Some(Instant::now());
        result
    }
}

pub(super) fn invalid(message: impl Into<String>) -> TaskError {
    TaskError::Message(format!("GitHub sync: {}", message.into()))
}

pub(super) fn request(query: &str, variables: Value) -> Result<Value, TaskError> {
    let mut child = Command::new("gh")
        .args(["api", "graphql", "--hostname", "github.com", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| invalid(format!("cannot run gh: {error}")))?;
    let input = json!({"query":query,"variables":variables});
    let write = match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(input.to_string().as_bytes()),
        None => return Err(invalid("cannot open gh input")),
    };
    let output = child
        .wait_with_output()
        .map_err(|error| invalid(format!("cannot wait for gh: {error}")))?;
    if !output.status.success() {
        return Err(invalid(String::from_utf8_lossy(&output.stderr)));
    }
    write.map_err(|error| invalid(format!("cannot write gh input: {error}")))?;
    let mut response: Value = serde_json::from_slice(&output.stdout)?;
    if response
        .get("errors")
        .is_some_and(|errors| errors.as_array().is_none_or(|items| !items.is_empty()))
    {
        return Err(invalid(format!("GraphQL errors: {}", response["errors"])));
    }
    if !response["data"].is_object() {
        return Err(invalid("missing GraphQL data"));
    }
    Ok(response["data"].take())
}

pub(super) fn items(project: &str) -> Result<(Vec<Value>, Vec<super::fields::Field>), TaskError> {
    let mut items = Vec::new();
    let mut cursor = Value::Null;
    let mut fields = None;
    let mut cursors = HashSet::new();
    loop {
        let data = request(
            ITEMS,
            json!({"project":project,"cursor":cursor,"archived":["ARCHIVED","NOT_ARCHIVED"]}),
        )?;
        if fields.is_none() {
            if data["node"]["fields"]["pageInfo"]["hasNextPage"] != false {
                return Err(invalid("missing or incomplete project fields"));
            }
            fields = Some(serde_json::from_value::<Vec<super::fields::Field>>(
                data["node"]["fields"]["nodes"].clone(),
            )?);
        }
        let connection = &data["node"]["items"];
        let page = connection["nodes"]
            .as_array()
            .ok_or_else(|| invalid("missing project items; check project ID and project scope"))?;
        items.extend(page.iter().cloned());
        match connection["pageInfo"]["hasNextPage"].as_bool() {
            Some(false) => return Ok((items, fields.unwrap_or_default())),
            Some(true)
                if connection["pageInfo"]["endCursor"]
                    .as_str()
                    .is_some_and(|value| cursors.insert(value.to_string())) =>
            {
                cursor = connection["pageInfo"]["endCursor"].clone()
            }
            _ => return Err(invalid("invalid project pagination")),
        }
    }
}
