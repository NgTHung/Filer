//! # Markdown Sections
//!
//! This module extracts task body sections without parsing full markdown. Task files
//! only need stable level-two sections, so a small line scanner keeps validation and
//! lifecycle updates predictable.
//!
//! ```
//! use filer_task::markdown::section;
//!
//! let body = "## Acceptance Criteria\n\n- [x] Works\n";
//! assert!(section(body, "Acceptance Criteria").is_some());
//! ```

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChecklistItem {
    pub checked: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChecklistMatch {
    pub(crate) item: ChecklistItem,
    pub(crate) marker: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MarkdownSection {
    pub heading: String,
    pub content: String,
}

pub fn level_two_sections(content: &str) -> Vec<MarkdownSection> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let Some(heading) = level_two_heading(lines[index]) else {
            index += 1;
            continue;
        };
        let start = index + 1;
        let end = lines[start..]
            .iter()
            .position(|line| level_two_heading(line).is_some())
            .map(|offset| start + offset)
            .unwrap_or(lines.len());
        sections.push(MarkdownSection {
            heading: heading.to_string(),
            content: lines[start..end].join("\n").trim().to_string(),
        });
        index = end;
    }

    sections
}

pub fn section(content: &str, heading: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines
        .iter()
        .position(|line| is_heading(line, heading))
        .map(|index| index + 1)?;
    let end = lines[start..]
        .iter()
        .position(|line| is_level_two_heading(line))
        .map(|offset| start + offset)
        .unwrap_or(lines.len());
    Some(lines[start..end].join("\n").trim().to_string())
}

pub fn has_section(content: &str, heading: &str) -> bool {
    section(content, heading).is_some()
}

pub fn checklist_items(content: &str, heading: &str) -> Vec<ChecklistItem> {
    checklist_matches(content, heading)
        .into_iter()
        .map(|matched| matched.item)
        .collect()
}

pub(crate) fn checklist_matches(content: &str, heading: &str) -> Vec<ChecklistMatch> {
    let mut matches = Vec::new();
    let mut in_section = false;
    let mut offset = 0;

    for line in content.split_inclusive('\n') {
        if let Some(current_heading) = level_two_heading(line) {
            if in_section && current_heading != heading {
                break;
            }
            in_section = current_heading == heading;
        } else if in_section && let Some(matched) = parse_checklist_match(line, offset) {
            matches.push(matched);
        }
        offset += line.len();
    }
    matches
}

pub fn has_unchecked_checklist_item(content: &str, heading: &str) -> bool {
    checklist_items(content, heading)
        .iter()
        .any(|item| !item.checked)
}

pub fn replace_or_append_section(content: &str, heading: &str, replacement: &str) -> String {
    let normalized = format!("## {heading}\n\n{}\n", replacement.trim());
    let lines: Vec<&str> = content.lines().collect();
    let Some(start) = lines.iter().position(|line| is_heading(line, heading)) else {
        let mut output = content.trim_end().to_string();
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(&normalized);
        return output;
    };

    let end = lines[start + 1..]
        .iter()
        .position(|line| is_level_two_heading(line))
        .map(|offset| start + 1 + offset)
        .unwrap_or(lines.len());

    let mut output = String::new();
    if start > 0 {
        output.push_str(&lines[..start].join("\n"));
        output.push_str("\n\n");
    }
    output.push_str(&normalized);
    if end < lines.len() {
        output.push('\n');
        output.push_str(&lines[end..].join("\n"));
        output.push('\n');
    }
    output
}

fn parse_checklist_match(line: &str, offset: usize) -> Option<ChecklistMatch> {
    let trimmed = line.trim_start();
    let indentation = line.len() - trimmed.len();
    let rest = trimmed.strip_prefix("- [")?;
    let (marker, text) = rest.split_once(']')?;
    let checked = match marker {
        "x" | "X" => true,
        " " => false,
        _ => return None,
    };
    let marker_start = offset + indentation + 3;
    Some(ChecklistMatch {
        item: ChecklistItem {
            checked,
            text: text.trim().to_string(),
        },
        marker: marker_start..marker_start + 1,
    })
}

fn is_heading(line: &str, heading: &str) -> bool {
    line.trim_end_matches('\r').trim() == format!("## {heading}")
}

fn is_level_two_heading(line: &str) -> bool {
    level_two_heading(line).is_some()
}

fn level_two_heading(line: &str) -> Option<&str> {
    line.trim_end_matches('\r')
        .trim()
        .strip_prefix("## ")
        .filter(|heading| !heading.is_empty() && !heading.starts_with('#'))
}

#[cfg(test)]
mod tests {
    use super::{
        checklist_items, has_unchecked_checklist_item, level_two_sections,
        replace_or_append_section, section,
    };

    #[test]
    fn extracts_section_until_next_level_two_heading() {
        let content =
            "## Summary\n\nText\n\n## Acceptance Criteria\n\n- [ ] Works\n\n## Notes\n\nLater\n";

        assert_eq!(
            section(content, "Acceptance Criteria").as_deref(),
            Some("- [ ] Works")
        );
    }

    #[test]
    fn extracts_checked_state() {
        let content = "## Exit Criteria\n\n- [x] Done\n- [ ] Open\n";

        let items = checklist_items(content, "Exit Criteria");

        assert_eq!(items.len(), 2);
        assert!(items[0].checked);
        assert!(!items[1].checked);
        assert!(has_unchecked_checklist_item(content, "Exit Criteria"));
    }

    #[test]
    fn replaces_existing_section() {
        let content = "## Summary\n\nText\n\n## Blocked Reason\n\nOld\n\n## Acceptance Criteria\n\n- [ ] Works\n";

        let updated = replace_or_append_section(content, "Blocked Reason", "New reason.");

        assert!(updated.contains("## Blocked Reason\n\nNew reason."));
        assert!(!updated.contains("Old"));
        assert!(updated.contains("## Acceptance Criteria"));
    }

    #[test]
    fn appends_missing_section() {
        let content = "## Summary\n\nText\n";

        let updated = replace_or_append_section(content, "Rationale", "Deferred.");

        assert!(updated.ends_with("## Rationale\n\nDeferred.\n"));
    }

    #[test]
    fn extracts_all_level_two_sections_in_order() {
        let content = "---\nid: CORE-001\n---\n\n## Summary\n\nWhy.\n\n### Detail\n\nMore.\n\n## Acceptance Criteria\n\n- [ ] Works\n";

        let sections = level_two_sections(content);

        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading, "Summary");
        assert_eq!(sections[0].content, "Why.\n\n### Detail\n\nMore.");
        assert_eq!(sections[1].heading, "Acceptance Criteria");
    }
}
