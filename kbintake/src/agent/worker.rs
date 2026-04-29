use std::path::PathBuf;

use anyhow::Result;
use tracing::{error, info, warn};

use crate::adapter::local_folder::LocalFolderAdapter;
use crate::app::App;
use crate::domain::{DomainEvent, ItemJob, ManifestRecord};
use crate::processor::{deduper, frontmatter, hasher, template, validator};
use crate::queue::repository::Repository;

pub fn process_item(app: &App, item: ItemJob) -> Result<()> {
    let source = PathBuf::from(&item.source_path);
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let target = match app.config.target_by_id(&item.target_id) {
        Ok(target) => target,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_TARGET_MISSING", &err.to_string())?;
            record_item_event(
                &repo,
                &item,
                "item.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_code": "E_TARGET_MISSING",
                    "error_message": err.to_string()
                }),
            )?;
            error!(item_id = %item.item_id, target_id = %item.target_id, error = %err, "target lookup failed");
            return Ok(());
        }
    };

    repo.update_item_running(&item.item_id, "validating")?;
    let size = match validator::validate_file(&source, app.config.import.max_file_size_mb) {
        Ok(size) => size,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_SOURCE_INVALID", &err.to_string())?;
            record_item_event(
                &repo,
                &item,
                "item.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_code": "E_SOURCE_INVALID",
                    "error_message": err.to_string()
                }),
            )?;
            error!(item_id = %item.item_id, error = %err, "validation failed");
            return Ok(());
        }
    };

    repo.update_item_running(&item.item_id, "hashing")?;
    let hash = match hasher::sha256_file(&source) {
        Ok(hash) => hash,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_HASH_FAILED", &err.to_string())?;
            record_item_event(
                &repo,
                &item,
                "item.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_code": "E_HASH_FAILED",
                    "error_message": err.to_string()
                }),
            )?;
            error!(item_id = %item.item_id, error = %err, "hash failed");
            return Ok(());
        }
    };
    repo.update_item_hash(&item.item_id, &hash, size as i64)?;

    let cli_tags: Vec<String> = item
        .cli_tags
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let rendered_template: Result<Option<template::RenderedTemplate>> =
        if let Some(template_config) = app.config.template_for_path(&source, size) {
            let resolved =
                template::resolve_template(&app.config.templates, &template_config.name)?;
            Ok(Some(template::render_template(
                &resolved,
                &template::TemplateRenderContext {
                    source_path: item.source_path.clone(),
                    source_name: item.source_name.clone(),
                    file_ext: item.file_ext.clone(),
                    file_size_bytes: size,
                    imported_at: chrono::Utc::now(),
                    sha256: hash.clone(),
                    target_name: target.name.clone(),
                    batch_id: item.batch_id.clone(),
                },
                &cli_tags,
            )))
        } else {
            Ok(None)
        };
    let rendered_template = match rendered_template {
        Ok(rendered_template) => rendered_template,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_TEMPLATE_FAILED", &err.to_string())?;
            record_item_event(
                &repo,
                &item,
                "item.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_code": "E_TEMPLATE_FAILED",
                    "error_message": err.to_string()
                }),
            )?;
            error!(item_id = %item.item_id, error = %err, "template render failed");
            return Ok(());
        }
    };

    if let Some((existing_record_id, stored_path)) = deduper::find_duplicate_record(&repo, &item.target_id, &hash)?
    {
        // If the stored file still exists, it's a genuine duplicate.
        if std::path::Path::new(&stored_path).exists() {
            repo.mark_item_duplicate(&item.item_id, &existing_record_id)?;
            record_item_event(
                &repo,
                &item,
                "item.duplicate",
                serde_json::json!({
                    "status": "duplicate",
                    "duplicate_of": existing_record_id
                }),
            )?;
            warn!(item_id = %item.item_id, duplicate_of = %existing_record_id, "duplicate skipped");
            return Ok(());
        }
        // File was deleted — remove stale manifest record and re-import.
        info!(item_id = %item.item_id, stored_path = %stored_path, "manifest record exists but file missing, re-importing");
        let _ = repo.delete_manifest_by_item(&existing_record_id);
    }

    repo.update_item_running(&item.item_id, "copying")?;

    let (dest, adapter) = if let Some(ref subfolder) = item.import_subfolder {
        // Watch mode with preserved directory structure — use exact path, no rename.
        let dest_root = target.root_path.join(subfolder);
        let adapter = LocalFolderAdapter::new(&dest_root);
        let dest = dest_root.join(&item.source_name);
        (dest, adapter)
    } else {
        let destination_root = rendered_template
            .as_ref()
            .and_then(|template| template.subfolder.as_deref())
            .filter(|subfolder| !subfolder.trim().is_empty())
            .map(|subfolder| target.root_path.join(subfolder))
            .or_else(|| {
                target
                    .default_subfolder
                    .as_deref()
                    .filter(|subfolder| !subfolder.trim().is_empty())
                    .map(|subfolder| target.root_path.join(subfolder))
            })
            .unwrap_or_else(|| target.root_path.clone());
        let adapter = LocalFolderAdapter::new(&destination_root);
        let dest = adapter.available_destination(&item.source_name);
        (dest, adapter)
    };

    let dest = match adapter.store_copy_to(&source, &dest) {
        Ok(dest) => dest,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_COPY_FAILED", &err.to_string())?;
            record_item_event(
                &repo,
                &item,
                "item.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_code": "E_COPY_FAILED",
                    "error_message": err.to_string()
                }),
            )?;
            error!(item_id = %item.item_id, error = %err, "copy failed");
            return Ok(());
        }
    };

    if app.config.import.inject_frontmatter
        && frontmatter::is_markdown_extension(item.file_ext.as_deref())
    {
        repo.update_item_running(&item.item_id, "frontmatter")?;
        if let Err(err) = frontmatter::inject_file(
            &dest,
            &frontmatter::FrontmatterFields {
                source_path: item.source_path.clone(),
                imported_at: chrono::Utc::now(),
                sha256: hash.clone(),
                target: target.name.clone(),
            },
            rendered_template
                .as_ref()
                .map(|template| &template.frontmatter),
        ) {
            repo.mark_item_failed(&item.item_id, "E_FRONTMATTER_FAILED", &err.to_string())?;
            record_item_event(
                &repo,
                &item,
                "item.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_code": "E_FRONTMATTER_FAILED",
                    "error_message": err.to_string()
                }),
            )?;
            error!(item_id = %item.item_id, error = %err, "frontmatter injection failed");
            return Ok(());
        }
    }

    let stored_sha256 = match hasher::sha256_file(&dest) {
        Ok(stored_sha256) => stored_sha256,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_HASH_FAILED", &err.to_string())?;
            record_item_event(
                &repo,
                &item,
                "item.failed",
                serde_json::json!({
                    "status": "failed",
                    "error_code": "E_HASH_FAILED",
                    "error_message": err.to_string()
                }),
            )?;
            error!(item_id = %item.item_id, error = %err, "stored file hash failed");
            return Ok(());
        }
    };

    let record = ManifestRecord::new(
        item.item_id.clone(),
        item.target_id.clone(),
        item.source_path.clone(),
        dest.to_string_lossy().to_string(),
        item.source_name.clone(),
        item.file_ext.clone(),
        Some(size as i64),
        hash,
    );
    if let Err(err) = repo.insert_manifest(&record) {
        if let Some((existing_record_id, stored_path)) =
            deduper::find_duplicate_record(&repo, &item.target_id, &record.sha256)?
        {
            if std::path::Path::new(&stored_path).exists() {
                repo.mark_item_duplicate(&item.item_id, &existing_record_id)?;
                record_item_event(
                    &repo,
                    &item,
                    "item.duplicate",
                    serde_json::json!({
                        "status": "duplicate",
                        "duplicate_of": existing_record_id
                    }),
                )?;
                warn!(
                    item_id = %item.item_id,
                    duplicate_of = %existing_record_id,
                    error = %err,
                    "manifest insert raced with existing duplicate"
                );
                return Ok(());
            }
            // File was deleted — remove stale record and retry.
            info!(item_id = %item.item_id, stored_path = %stored_path, "stale manifest, deleting and retrying");
            let _ = repo.delete_manifest_by_item(&existing_record_id);
        }

        repo.mark_item_failed(&item.item_id, "E_MANIFEST_FAILED", &err.to_string())?;
        record_item_event(
            &repo,
            &item,
            "item.failed",
            serde_json::json!({
                "status": "failed",
                "error_code": "E_MANIFEST_FAILED",
                "error_message": err.to_string()
            }),
        )?;
        error!(item_id = %item.item_id, error = %err, "manifest insert failed");
        return Ok(());
    }
    repo.mark_item_success(&item.item_id, &record.stored_path, &stored_sha256)?;
    record_item_event(
        &repo,
        &item,
        "item.success",
        serde_json::json!({
            "status": "success",
            "stored_path": record.stored_path,
            "sha256": record.sha256,
            "source_size": record.source_size
        }),
    )?;

    info!(item_id = %item.item_id, stored_path = %record.stored_path, "item imported");

    // Auto-open in Obsidian if configured and target has a vault name.
    if app.config.import.auto_open_obsidian
        && frontmatter::is_markdown_extension(item.file_ext.as_deref())
    {
        if let (Some(vault), Ok(rel)) = (
            target.obsidian_vault.as_ref(),
            dest.strip_prefix(&target.root_path),
        ) {
            let obsidian_path = rel.to_string_lossy().replace('\\', "/");
            if let Err(e) = crate::obsidian::open_note(vault, &obsidian_path) {
                warn!(item_id = %item.item_id, error = %e, "failed to auto-open note in Obsidian");
            }
        }
    }

    Ok(())
}

fn record_item_event(
    repo: &Repository<'_>,
    item: &ItemJob,
    event_type: &str,
    payload_json: serde_json::Value,
) -> Result<()> {
    repo.insert_event(&DomainEvent::new(
        "item",
        item.item_id.clone(),
        event_type,
        payload_json,
    ))
}
