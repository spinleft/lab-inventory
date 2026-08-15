//! Every SQL statement the label printer routes issue lives here.
//!
//! Rules for this module:
//! - one function issues at most one statement, and performs no orchestration
//! - functions never return a handler error type, only
//!   [`LabelPrinterDatabaseError`], so any handler can reuse them
use super::model::LabelPrinterRow;
use crate::domain::{LaboratoryId, NewLabelPrinter, UpdateLabelPrinter};
use crate::utils::error_chain_fmt;
use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(thiserror::Error)]
pub(super) enum LabelPrinterDatabaseError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl std::fmt::Debug for LabelPrinterDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub(super) fn map_database_error(error: sqlx::Error) -> LabelPrinterDatabaseError {
    if let sqlx::Error::Database(database_error) = &error {
        match (
            database_error.code().as_deref(),
            database_error.constraint(),
        ) {
            (Some("23505"), Some("uq_label_printers_laboratory_name")) => {
                return LabelPrinterDatabaseError::Conflict(
                    "A label printer with this name already exists".into(),
                );
            }
            (Some("23505"), _) => {
                return LabelPrinterDatabaseError::Conflict("Label printer already exists".into());
            }
            (Some("23514"), _) => {
                return LabelPrinterDatabaseError::Validation("Invalid label printer".into());
            }
            (Some("23503"), _) => {
                return LabelPrinterDatabaseError::Validation("Invalid referenced record".into());
            }
            _ => {}
        }
    }

    LabelPrinterDatabaseError::Unexpected(error.into())
}

pub(super) async fn insert_label_printer(
    transaction: &mut Transaction<'_, Postgres>,
    laboratory_id: LaboratoryId,
    printer: &NewLabelPrinter,
) -> Result<LabelPrinterRow, LabelPrinterDatabaseError> {
    sqlx::query_as!(
        LabelPrinterRow,
        r#"
        INSERT INTO label_printers (
            printer_id, laboratory_id, name, host, port, model,
            media_kind, media_width_mm, media_length_mm, auto_cut
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING printer_id, laboratory_id, name, host, port, model,
                  media_kind, media_width_mm, media_length_mm, auto_cut,
                  created_at, updated_at
        "#,
        Uuid::new_v4(),
        Uuid::from(laboratory_id),
        printer.name.as_ref(),
        printer.host.as_ref(),
        i32::from(printer.port),
        printer.model,
        printer.media.kind_str(),
        i32::from(printer.media.width_mm()),
        printer.media.length_mm().map(i32::from),
        printer.auto_cut,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn fetch_label_printer(
    pool: &PgPool,
    printer_id: Uuid,
) -> Result<Option<LabelPrinterRow>, anyhow::Error> {
    sqlx::query_as!(
        LabelPrinterRow,
        r#"
        SELECT printer_id, laboratory_id, name, host, port, model,
               media_kind, media_width_mm, media_length_mm, auto_cut,
               created_at, updated_at
        FROM label_printers
        WHERE printer_id = $1
        "#,
        printer_id,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to fetch label printer")
}

/// Same projection as [`fetch_label_printer`], but takes the row lock the write
/// paths need. `query_as!` requires a literal, so the column list cannot be
/// shared.
pub(super) async fn fetch_label_printer_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    printer_id: Uuid,
) -> Result<Option<LabelPrinterRow>, anyhow::Error> {
    sqlx::query_as!(
        LabelPrinterRow,
        r#"
        SELECT printer_id, laboratory_id, name, host, port, model,
               media_kind, media_width_mm, media_length_mm, auto_cut,
               created_at, updated_at
        FROM label_printers
        WHERE printer_id = $1
        FOR UPDATE
        "#,
        printer_id,
    )
    .fetch_optional(transaction.as_mut())
    .await
    .context("Failed to fetch label printer for update")
}

pub(super) async fn fetch_label_printers(
    pool: &PgPool,
    laboratory_id: LaboratoryId,
) -> Result<Vec<LabelPrinterRow>, anyhow::Error> {
    sqlx::query_as!(
        LabelPrinterRow,
        r#"
        SELECT printer_id, laboratory_id, name, host, port, model,
               media_kind, media_width_mm, media_length_mm, auto_cut,
               created_at, updated_at
        FROM label_printers
        WHERE laboratory_id = $1
        ORDER BY name
        "#,
        Uuid::from(laboratory_id),
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch label printers")
}

/// Applies a partial update.
///
/// Media is atomic: the three media columns move together, and a non-null
/// `media_kind` is what signals that they are being replaced. That is why
/// `media_length_mm` keys off `$6` rather than off itself — it has to be able
/// to become NULL when switching to continuous stock.
pub(super) async fn update_label_printer_in_database(
    transaction: &mut Transaction<'_, Postgres>,
    printer_id: Uuid,
    update: &UpdateLabelPrinter,
) -> Result<LabelPrinterRow, LabelPrinterDatabaseError> {
    sqlx::query_as!(
        LabelPrinterRow,
        r#"
        UPDATE label_printers
        SET name = COALESCE($2, name),
            host = COALESCE($3, host),
            port = COALESCE($4, port),
            model = COALESCE($5, model),
            media_kind = COALESCE($6, media_kind),
            media_width_mm = COALESCE($7, media_width_mm),
            media_length_mm = CASE WHEN $6::text IS NULL THEN media_length_mm ELSE $8 END,
            auto_cut = COALESCE($9, auto_cut),
            updated_at = now()
        WHERE printer_id = $1
        RETURNING printer_id, laboratory_id, name, host, port, model,
                  media_kind, media_width_mm, media_length_mm, auto_cut,
                  created_at, updated_at
        "#,
        printer_id,
        update.name.as_ref().map(|name| name.as_ref()),
        update.host.as_ref().map(|host| host.as_ref()),
        update.port.map(i32::from),
        update.model.as_deref(),
        update.media.map(|media| media.kind_str()),
        update.media.map(|media| i32::from(media.width_mm())),
        update
            .media
            .and_then(|media| media.length_mm())
            .map(i32::from),
        update.auto_cut,
    )
    .fetch_one(transaction.as_mut())
    .await
    .map_err(map_database_error)
}

pub(super) async fn delete_label_printer_from_database(
    transaction: &mut Transaction<'_, Postgres>,
    printer_id: Uuid,
) -> Result<(), LabelPrinterDatabaseError> {
    sqlx::query!(
        r#"
        DELETE FROM label_printers
        WHERE printer_id = $1
        "#,
        printer_id,
    )
    .execute(transaction.as_mut())
    .await
    .map_err(map_database_error)?;

    Ok(())
}
