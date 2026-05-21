// XLSX + CSV export adapters — replace the JS `xlsx` library's writeFile path.
//
// Spec 001-leptos-migration §Phase 1D T097-T098; research.md §8 (`xlsx`
// replacement). The handler builds the workbook server-side from a typed
// `Vec<ManualSession>` using `rust_xlsxwriter` (write-only; sufficient
// because we never read .xlsx files) and writes it directly to the
// caller-provided path. Same user-visible behaviour as the JS-era flow;
// less data crossing the bridge (typed records instead of a pre-built
// base64 blob).
//
// The legacy `write_excel_file` cutover-parity command was removed in
// Phase 6 (T235); the JS-era export path is gone post-cutover.
//
// Lint allowance rationale — `clippy::redundant_pub_crate`: `pub(super)`
// items in a private module are callable by `lib.rs` without widening to
// plain `pub`, which would trip the workspace `unreachable_pub` lint.
#![allow(
    clippy::redundant_pub_crate,
    reason = "Private module exposes pub(super) export helpers to lib.rs while avoiding plain pub unreachable_pub."
)]

use std::path::Path;

use rust_xlsxwriter::Workbook;

use crate::{BridgeError, ManualSession};

const fn session_type_to_str(session_type: super::SessionType) -> &'static str {
    match session_type {
        super::SessionType::Focus => "focus",
        super::SessionType::Break => "break",
        super::SessionType::LongBreak => "longBreak",
        super::SessionType::Custom => "custom",
    }
}

fn joined_tag_names(tags: Option<&Vec<serde_json::Value>>) -> String {
    tags.map(|values| {
        values
            .iter()
            .filter_map(|v| v.get("name").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join("; ")
    })
    .unwrap_or_default()
}

/// Build an XLSX workbook from `sessions` and write it to `path`.
///
/// Schema (one row per session, header in row 0):
/// `id` | `session_type` | `duration` | `start_time` | `end_time` | `date` | `created_at` | `title` | `tags` | `notes`
///
/// Schema mirrors `export_csv` so both formats expose identical column sets.
/// `session_type` is serialised via the closed-domain enum's `Display`
/// equivalent (camelCase string), matching what the JS-era export wrote.
/// `notes`, `title`, and `tags` fall back to empty strings when `None`.
pub(super) fn export(path: &Path, sessions: &[ManualSession]) -> Result<(), BridgeError> {
    let mut workbook = Workbook::new();
    let sheet = workbook
        .add_worksheet()
        .set_name("Session History")
        .map_err(|e| BridgeError::Internal {
            msg: format!("Failed to set worksheet name: {e}"),
        })?;

    let headers = [
        "id",
        "session_type",
        "duration",
        "start_time",
        "end_time",
        "date",
        "created_at",
        "title",
        "tags",
        "notes",
    ];
    for (col, header) in headers.iter().enumerate() {
        let col_index = u16::try_from(col).map_err(|e| BridgeError::Internal {
            msg: format!("xlsx column index overflow: {e}"),
        })?;
        sheet
            .write_string(0, col_index, *header)
            .map_err(|e| BridgeError::Internal {
                msg: format!("Failed to write header cell: {e}"),
            })?;
    }

    for (i, session) in sessions.iter().enumerate() {
        // Row 0 holds headers; sessions start at row 1. Bound-check the
        // index → u32 conversion explicitly (the lint denies `as` casts
        // on potentially-truncating conversions).
        let row = u32::try_from(i + 1).map_err(|e| BridgeError::Internal {
            msg: format!("xlsx row index overflow: {e}"),
        })?;
        let session_type_str = session_type_to_str(session.session_type);
        let tags_joined = joined_tag_names(session.tags.as_ref());
        sheet
            .write_string(row, 0, &session.id)
            .and_then(|s| s.write_string(row, 1, session_type_str))
            .and_then(|s| s.write_number(row, 2, f64::from(session.duration)))
            .and_then(|s| s.write_string(row, 3, &session.start_time))
            .and_then(|s| s.write_string(row, 4, &session.end_time))
            .and_then(|s| s.write_string(row, 5, &session.date))
            .and_then(|s| s.write_string(row, 6, &session.created_at))
            .and_then(|s| s.write_string(row, 7, session.title.as_deref().unwrap_or("")))
            .and_then(|s| s.write_string(row, 8, &tags_joined))
            .and_then(|s| s.write_string(row, 9, session.notes.as_deref().unwrap_or("")))
            .map_err(|e| BridgeError::Internal {
                msg: format!("Failed to write session row: {e}"),
            })?;
    }

    workbook.save(path).map_err(|e| BridgeError::Internal {
        msg: format!("Failed to save xlsx to {}: {e}", path.display()),
    })?;
    Ok(())
}

/// Build a CSV string from `sessions` and write it to `path`.
///
/// Schema mirrors the XLSX export so both formats round-trip the same
/// columns. Fields containing commas, quotes, or newlines are wrapped
/// in double quotes with embedded `"` doubled per RFC 4180.
pub(super) fn export_csv(path: &Path, sessions: &[ManualSession]) -> Result<(), BridgeError> {
    let mut out = String::new();
    out.push_str(
        "id,session_type,duration,start_time,end_time,date,created_at,title,tags,notes\r\n",
    );
    for session in sessions {
        let session_type_str = session_type_to_str(session.session_type);
        let tags_joined = joined_tag_names(session.tags.as_ref());
        let title = session.title.as_deref().unwrap_or("");
        let notes = session.notes.as_deref().unwrap_or("");
        let row = [
            csv_field(&session.id),
            csv_field(session_type_str),
            session.duration.to_string(),
            csv_field(&session.start_time),
            csv_field(&session.end_time),
            csv_field(&session.date),
            csv_field(&session.created_at),
            csv_field(title),
            csv_field(&tags_joined),
            csv_field(notes),
        ];
        out.push_str(&row.join(","));
        out.push_str("\r\n");
    }
    std::fs::write(path, out).map_err(|e| BridgeError::Internal {
        msg: format!("Failed to save csv to {}: {e}", path.display()),
    })?;
    Ok(())
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{csv_field, export, export_csv, ManualSession};
    use crate::SessionType;
    use tempfile::tempdir;

    fn sample_session(id: &str, session_type: SessionType) -> ManualSession {
        ManualSession {
            id: id.to_string(),
            session_type,
            duration: 25,
            start_time: "09:00".to_string(),
            end_time: "09:25".to_string(),
            notes: Some("deep work".to_string()),
            created_at: "2026-05-10T09:00:00Z".to_string(),
            date: "Sat May 10 2026".to_string(),
            tags: None,
            title: None,
        }
    }

    /// Pins T098's done-signal: writing a workbook to a tempdir produces
    /// a non-empty file. We don't validate the binary contents (xlsx
    /// is a zipped XML format — round-tripping would require pulling
    /// in a reader, defeating the write-only choice in research.md §8);
    /// existence + non-zero size is the available-evidence assertion.
    #[test]
    fn export_writes_non_empty_workbook_to_tempfile() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("export.xlsx");
        let sessions = vec![
            sample_session("ms-1", SessionType::Focus),
            sample_session("ms-2", SessionType::LongBreak),
        ];
        export(&out, &sessions).unwrap();
        let metadata = std::fs::metadata(&out).expect("xlsx file should exist");
        assert!(metadata.len() > 0, "xlsx file should be non-empty");
    }

    #[test]
    fn export_handles_empty_session_list() {
        // No data rows — just the header. The workbook should still be
        // a valid xlsx file with non-zero size (the rust_xlsxwriter
        // engine writes the workbook scaffolding even for an empty
        // worksheet).
        let dir = tempdir().unwrap();
        let out = dir.path().join("empty.xlsx");
        export(&out, &[]).unwrap();
        let metadata = std::fs::metadata(&out).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn export_handles_missing_notes_as_empty_string() {
        // The JS-era xlsx library wrote `null` notes as empty cells; we
        // do the same so the round-trip produces a byte-stable
        // user-visible spreadsheet.
        let dir = tempdir().unwrap();
        let out = dir.path().join("nonotes.xlsx");
        let mut session = sample_session("ms-3", SessionType::Custom);
        session.notes = None;
        export(&out, &[session]).unwrap();
        let metadata = std::fs::metadata(&out).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn csv_field_quotes_special_chars() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn export_csv_writes_header_and_rows() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("export.csv");
        let mut session = sample_session("ms-1", SessionType::Focus);
        session.title = Some("Spec, draft".to_string());
        session.notes = Some("includes \"quoted\" word".to_string());
        export_csv(&out, &[session]).unwrap();
        let contents = std::fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = contents.split_terminator("\r\n").collect();
        assert_eq!(lines.len(), 2, "header + 1 row");
        assert_eq!(
            lines[0],
            "id,session_type,duration,start_time,end_time,date,created_at,title,tags,notes"
        );
        assert_eq!(
            lines[1],
            r#"ms-1,focus,25,09:00,09:25,Sat May 10 2026,2026-05-10T09:00:00Z,"Spec, draft",,"includes ""quoted"" word""#
        );
    }

    #[test]
    fn export_csv_handles_empty_session_list() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("empty.csv");
        export_csv(&out, &[]).unwrap();
        let contents = std::fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = contents.split_terminator("\r\n").collect();
        assert_eq!(lines.len(), 1, "header only");
        assert_eq!(
            lines[0],
            "id,session_type,duration,start_time,end_time,date,created_at,title,tags,notes"
        );
    }
}
