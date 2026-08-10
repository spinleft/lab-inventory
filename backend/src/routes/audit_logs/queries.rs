//! Every SQL statement the audit log route issues lives here.
//!
//! Only the projection is shared: the filters are built with a `QueryBuilder`
//! in the handler, which owns the query type they come from.
//!
//! Audit rows are never written from here — `audit::record_audit` does that,
//! inside the transaction of whatever change is being recorded.

/// The projection shared by the listing and its count, for `QueryBuilder`
/// callers that append their own filters.
pub(super) fn audit_log_select() -> &'static str {
    r#"
    SELECT
        audit_logs.audit_log_id,
        audit_logs.actor_user_id,
        actor_user.username AS actor_username,
        audit_logs.action,
        audit_logs.resource_type,
        audit_logs.resource_id,
        audit_logs.details,
        audit_logs.created_at
    FROM audit_logs
    LEFT JOIN users AS actor_user ON actor_user.user_id = audit_logs.actor_user_id
    "#
}

/// The same source as [`audit_log_select`] with no columns, so a count and a
/// page are always filtered over exactly the same rows.
pub(super) fn audit_log_count_select() -> &'static str {
    r#"
    SELECT COUNT(*)
    FROM audit_logs
    LEFT JOIN users AS actor_user ON actor_user.user_id = audit_logs.actor_user_id
    "#
}
