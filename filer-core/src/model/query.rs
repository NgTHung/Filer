/// Search query representation
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub filters: Vec<QueryFilter>,
    pub options: SearchOptions,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub include_hidden: bool,
    pub max_depth: Option<usize>,
    pub max_results: Option<usize>,
}

impl SearchQuery {
    /// Parse query string into structured query
    pub fn parse(input: &str) -> Result<Self, QueryParseError> {
        todo!()
    }
}

#[derive(Debug)]
pub struct QueryParseError {
    pub message: String,
    pub position: usize,
}