/// Search query representation
///
/// Parsed from a single query string using inline filter syntax.
/// Plain text becomes the filename pattern; prefixed tokens become filters/options.
///
/// # Query format
///
/// ```text
/// <search text> [ext:rs,py] [size:>1mb] [size:<500kb] [type:file|dir]
///               [after:2024-01-15] [before:2025-12-31] [name:substring]
///               [match:regex] [hidden:yes] [case:yes] [depth:3] [max:100]
/// ```
///
/// All prefixed tokens are order-independent. Unrecognised tokens are treated
/// as part of the search text.
///
/// # Examples
///
/// | Input | Meaning |
/// |---|---|
/// | `report` | Files containing "report" in name |
/// | `*.rs ext:rs size:>1kb` | `.rs` files larger than 1 KB |
/// | `type:dir hidden:yes` | All directories including hidden |
/// | `match:^test_.*\.rs$ depth:2` | Regex match, max 2 levels deep |
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub filters: Vec<QueryFilter>,
    pub options: SearchOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueryFilter {
    Extension(Vec<String>),
    SizeGreaterThan(u64),
    SizeLessThan(u64),
    ModifiedAfter(i64),
    ModifiedBefore(i64),
    IsDirectory,
    IsFile,
    IsHidden,
    NameContains(String),
    NameMatches(String), // Regex
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub include_hidden: bool,
    pub max_depth: Option<usize>,
    pub max_results: Option<usize>,
    pub batch_size: Option<usize>,
}

impl SearchQuery {
    /// Parse a query string into a structured `SearchQuery`.
    ///
    /// Tokens with recognised prefixes (`ext:`, `size:>`, `type:`, etc.)
    /// are extracted as filters or options. Everything else is joined into
    /// the `text` field as the filename search pattern.
    ///
    /// An empty/whitespace-only input is an error.
    pub fn parse(input: &str) -> Result<Self, QueryParseError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(QueryParseError {
                message: "Empty query".to_string(),
                position: 0,
            });
        }

        let mut text_parts: Vec<&str> = Vec::new();
        let mut filters: Vec<QueryFilter> = Vec::new();
        let mut options = SearchOptions::default();
        let mut position: usize = 0;

        for token in input.split_whitespace() {
            let parsed = if let Some(value) = token.strip_prefix("ext:") {
                Self::parse_ext(value, position)
            } else if let Some(value) = token.strip_prefix("size:") {
                Self::parse_size(value, position)
            } else if let Some(value) = token.strip_prefix("type:") {
                Self::parse_type(value, position)
            } else if let Some(value) = token.strip_prefix("after:") {
                Self::parse_date(value, position, true)
            } else if let Some(value) = token.strip_prefix("before:") {
                Self::parse_date(value, position, false)
            } else if let Some(value) = token.strip_prefix("name:") {
                if value.is_empty() {
                    Err(QueryParseError {
                        message: "Empty name: value".to_string(),
                        position,
                    })
                } else {
                    Ok(ParsedToken::Filter(QueryFilter::NameContains(
                        value.to_string(),
                    )))
                }
            } else if let Some(value) = token.strip_prefix("match:") {
                if value.is_empty() {
                    Err(QueryParseError {
                        message: "Empty match: pattern".to_string(),
                        position,
                    })
                } else {
                    // Validate regex compiles at parse time
                    regex::Regex::new(value).map_err(|e| QueryParseError {
                        message: format!("Invalid regex pattern: {}", e),
                        position,
                    })?;
                    Ok(ParsedToken::Filter(QueryFilter::NameMatches(
                        value.to_string(),
                    )))
                }
            } else if let Some(value) = token.strip_prefix("hidden:") {
                Self::parse_bool(value, position)
                    .map(|yes| ParsedToken::Option(OptionSet::Hidden(yes)))
            } else if let Some(value) = token.strip_prefix("case:") {
                Self::parse_bool(value, position)
                    .map(|yes| ParsedToken::Option(OptionSet::Case(yes)))
            } else if let Some(value) = token.strip_prefix("depth:") {
                value
                    .parse::<usize>()
                    .map(|d| ParsedToken::Option(OptionSet::Depth(d)))
                    .map_err(|_| QueryParseError {
                        message: format!("Invalid depth value: '{}'", value),
                        position,
                    })
            } else if let Some(value) = token.strip_prefix("max:") {
                value
                    .parse::<usize>()
                    .map(|m| ParsedToken::Option(OptionSet::Max(m)))
                    .map_err(|_| QueryParseError {
                        message: format!("Invalid max value: '{}'", value),
                        position,
                    })
            } else {
                // Not a prefixed token — it's search text
                text_parts.push(token);
                position += token.len() + 1;
                continue;
            };

            match parsed? {
                ParsedToken::Filter(f) => filters.push(f),
                ParsedToken::Option(opt) => match opt {
                    OptionSet::Hidden(yes) => {
                        options.include_hidden = yes;
                        if yes {
                            filters.push(QueryFilter::IsHidden);
                        }
                    }
                    OptionSet::Case(yes) => options.case_sensitive = yes,
                    OptionSet::Depth(d) => options.max_depth = Some(d),
                    OptionSet::Max(m) => options.max_results = Some(m),
                },
            }

            position += token.len() + 1;
        }

        Ok(SearchQuery {
            text: text_parts.join(" "),
            filters,
            options,
        })
    }

    fn parse_ext(value: &str, position: usize) -> Result<ParsedToken, QueryParseError> {
        if value.is_empty() {
            return Err(QueryParseError {
                message: "Empty ext: value".to_string(),
                position,
            });
        }
        let exts: Vec<String> = value
            .split(',')
            .map(|e| e.trim_start_matches('.').to_lowercase())
            .filter(|e| !e.is_empty())
            .collect();
        if exts.is_empty() {
            return Err(QueryParseError {
                message: format!("No valid extensions in '{}'", value),
                position,
            });
        }
        Ok(ParsedToken::Filter(QueryFilter::Extension(exts)))
    }

    fn parse_size(value: &str, position: usize) -> Result<ParsedToken, QueryParseError> {
        if value.len() < 2 {
            return Err(QueryParseError {
                message: format!(
                    "Invalid size filter: '{}' (use size:>1kb or size:<1mb)",
                    value
                ),
                position,
            });
        }

        let (comparator, rest) = value.split_at(1);
        let bytes = Self::parse_size_value(rest, position)?;

        match comparator {
            ">" => Ok(ParsedToken::Filter(QueryFilter::SizeGreaterThan(bytes))),
            "<" => Ok(ParsedToken::Filter(QueryFilter::SizeLessThan(bytes))),
            _ => Err(QueryParseError {
                message: format!("Invalid size comparator '{}' (use > or <)", comparator),
                position,
            }),
        }
    }

    /// Parse a human-readable size like "1kb", "500mb", "2gb", or plain bytes "4096".
    fn parse_size_value(input: &str, position: usize) -> Result<u64, QueryParseError> {
        let input_lower = input.to_lowercase();
        let (num_str, multiplier) = if let Some(n) = input_lower.strip_suffix("gb") {
            (n, 1024u64 * 1024 * 1024)
        } else if let Some(n) = input_lower.strip_suffix("mb") {
            (n, 1024u64 * 1024)
        } else if let Some(n) = input_lower.strip_suffix("kb") {
            (n, 1024u64)
        } else if let Some(n) = input_lower.strip_suffix('b') {
            (n, 1u64)
        } else {
            (input_lower.as_str(), 1u64)
        };

        num_str
            .parse::<u64>()
            .map(|n| n * multiplier)
            .map_err(|_| QueryParseError {
                message: format!("Invalid size value: '{}'", input),
                position,
            })
    }

    fn parse_type(value: &str, position: usize) -> Result<ParsedToken, QueryParseError> {
        match value.to_lowercase().as_str() {
            "file" | "f" => Ok(ParsedToken::Filter(QueryFilter::IsFile)),
            "dir" | "directory" | "d" | "folder" => {
                Ok(ParsedToken::Filter(QueryFilter::IsDirectory))
            }
            _ => Err(QueryParseError {
                message: format!("Unknown type '{}' (use file, dir, folder, f, or d)", value),
                position,
            }),
        }
    }

    /// Parse a date value. Supports:
    /// - Unix timestamp (seconds): `1700000000`
    /// - ISO date: `2024-01-15`
    fn parse_date(
        value: &str,
        position: usize,
        is_after: bool,
    ) -> Result<ParsedToken, QueryParseError> {
        let timestamp = if let Ok(ts) = value.parse::<i64>() {
            // Unix timestamp
            ts
        } else if value.len() == 10 && value.chars().filter(|c| *c == '-').count() == 2 {
            // YYYY-MM-DD → naive conversion to unix timestamp
            Self::date_to_timestamp(value, position)?
        } else {
            return Err(QueryParseError {
                message: format!(
                    "Invalid date '{}' (use YYYY-MM-DD or unix timestamp)",
                    value
                ),
                position,
            });
        };

        if is_after {
            Ok(ParsedToken::Filter(QueryFilter::ModifiedAfter(timestamp)))
        } else {
            Ok(ParsedToken::Filter(QueryFilter::ModifiedBefore(timestamp)))
        }
    }

    /// Convert YYYY-MM-DD to a unix timestamp (midnight UTC).
    fn date_to_timestamp(date: &str, position: usize) -> Result<i64, QueryParseError> {
        let parts: Vec<&str> = date.split('-').collect();
        if parts.len() != 3 {
            return Err(QueryParseError {
                message: format!("Invalid date format: '{}'", date),
                position,
            });
        }

        let year: i64 = parts[0].parse().map_err(|_| QueryParseError {
            message: format!("Invalid year in '{}'", date),
            position,
        })?;
        let month: i64 = parts[1].parse().map_err(|_| QueryParseError {
            message: format!("Invalid month in '{}'", date),
            position,
        })?;
        let day: i64 = parts[2].parse().map_err(|_| QueryParseError {
            message: format!("Invalid day in '{}'", date),
            position,
        })?;

        if !(1..=12).contains(&month) || !(1..=31).contains(&day) || year < 1970 {
            return Err(QueryParseError {
                message: format!("Date out of range: '{}'", date),
                position,
            });
        }

        // Days from 1970-01-01 using a simplified calculation.
        // Accurate enough for file search filtering.
        let mut days: i64 = 0;
        for y in 1970..year {
            days += if is_leap_year(y) { 366 } else { 365 };
        }
        let month_days = [
            31,
            28 + if is_leap_year(year) { 1 } else { 0 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        for m in month_days.iter().take((month - 1) as usize) {
            days += m;
        }
        days += day - 1;

        Ok(days * 86400)
    }

    fn parse_bool(value: &str, position: usize) -> Result<bool, QueryParseError> {
        match value.to_lowercase().as_str() {
            "yes" | "true" | "1" | "on" => Ok(true),
            "no" | "false" | "0" | "off" => Ok(false),
            _ => Err(QueryParseError {
                message: format!("Invalid boolean '{}' (use yes/no)", value),
                position,
            }),
        }
    }
}

/// Internal parsed token — either a filter or an option setter.

impl SearchQuery {
    /// Returns `true` if `node` satisfies all conditions in this query.
    ///
    /// AND semantics: every filter must pass, and the text pattern (if any)
    /// must match the file name. Case sensitivity is controlled by
    /// `options.case_sensitive`.
    pub fn matches(&self, node: &crate::model::node::FileNode) -> bool {
        if !self.text.is_empty() {
            let matched = if self.options.case_sensitive {
                node.name.contains(&self.text)
            } else {
                node.name.to_lowercase().contains(&self.text.to_lowercase())
            };
            if !matched {
                return false;
            }
        }

        for filter in &self.filters {
            if !filter.matches(node) {
                return false;
            }
        }

        true
    }
}

impl QueryFilter {
    /// Returns `true` if `node` satisfies this individual filter.
    pub fn matches(&self, node: &crate::model::node::FileNode) -> bool {
        match self {
            QueryFilter::Extension(exts) => {
                let ext = node.extension().unwrap_or("").to_lowercase();
                exts.iter().any(|e| e == &ext)
            }
            QueryFilter::SizeGreaterThan(n) => node.size > *n,
            QueryFilter::SizeLessThan(n) => node.size < *n,
            QueryFilter::ModifiedAfter(ts) => node
                .modified
                .map(|t| systemtime_to_i64(t) > *ts)
                .unwrap_or(false),
            QueryFilter::ModifiedBefore(ts) => node
                .modified
                .map(|t| systemtime_to_i64(t) < *ts)
                .unwrap_or(false),
            QueryFilter::IsDirectory => node.is_dir(),
            QueryFilter::IsFile => node.is_file(),
            QueryFilter::IsHidden => node.meta.hidden,
            QueryFilter::NameContains(s) => node.name.contains(s.as_str()),
            QueryFilter::NameMatches(pattern) => regex::Regex::new(pattern)
                .map(|re| re.is_match(&node.name))
                .unwrap_or(false),
        }
    }
}

fn systemtime_to_i64(t: std::time::SystemTime) -> i64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

enum ParsedToken {
    Filter(QueryFilter),
    Option(OptionSet),
}

/// Internal enum for option mutations.
enum OptionSet {
    Hidden(bool),
    Case(bool),
    Depth(usize),
    Max(usize),
}

fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

#[derive(Debug)]
pub struct QueryParseError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for QueryParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "query parse error at {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for QueryParseError {}
