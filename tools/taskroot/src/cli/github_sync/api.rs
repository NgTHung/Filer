use std::{
    io::Write,
    process::{Command, Stdio},
};

use serde_json::{Value, json};

use crate::error::TaskError;

const ITEMS: &str = "query($project:ID!,$cursor:String){node(id:$project){... on ProjectV2{items(first:100,after:$cursor){nodes{id isArchived type content{__typename ... on DraftIssue{id title body} ... on Issue{id title body}}} pageInfo{hasNextPage endCursor}}}}}";

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

pub(super) fn items(project: &str) -> Result<Vec<Value>, TaskError> {
    let mut items = Vec::new();
    let mut cursor = Value::Null;
    loop {
        let data = request(ITEMS, json!({"project":project,"cursor":cursor}))?;
        let connection = &data["node"]["items"];
        let page = connection["nodes"]
            .as_array()
            .ok_or_else(|| invalid("missing project items; check project ID and project scope"))?;
        items.extend(page.iter().cloned());
        match connection["pageInfo"]["hasNextPage"].as_bool() {
            Some(false) => return Ok(items),
            Some(true)
                if connection["pageInfo"]["endCursor"].is_string()
                    && connection["pageInfo"]["endCursor"] != cursor =>
            {
                cursor = connection["pageInfo"]["endCursor"].clone()
            }
            _ => return Err(invalid("invalid project pagination")),
        }
    }
}
