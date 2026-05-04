use crate::utils::ApiError;
use serde::{Deserialize, Serialize};
use serde_aux::field_attributes::deserialize_option_number_from_string;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 200;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Pagination {
    #[serde(default, deserialize_with = "deserialize_option_number_from_string")]
    pub limit: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_option_number_from_string")]
    pub offset: Option<i64>,
}

impl Pagination {
    pub fn limit(&self) -> Result<i64, ApiError> {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if limit <= 0 {
            return Err(ApiError::BadRequest("limit must be positive".into()));
        }
        Ok(limit.min(MAX_LIMIT))
    }

    pub fn offset(&self) -> Result<i64, ApiError> {
        let offset = self.offset.unwrap_or(0);
        if offset < 0 {
            return Err(ApiError::BadRequest("offset must be non-negative".into()));
        }
        Ok(offset)
    }
}

#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(items: Vec<T>, pagination: &Pagination, total: i64) -> Result<Self, ApiError> {
        Ok(Self {
            items,
            limit: pagination.limit()?,
            offset: pagination.offset()?,
            total,
        })
    }
}

pub fn normalized_search_pattern(q: &Option<String>) -> Option<String> {
    q.as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{q}%"))
}
