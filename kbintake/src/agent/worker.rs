use std::path::PathBuf;

use anyhow::Result;
use tracing::{error, info, warn};

use crate::adapter::local_folder::LocalFolderAdapter;
use crate::app::App;
use crate::domain::{ItemJob, ManifestRecord};
use crate::processor::{deduper, hasher, validator};
use crate::queue::repository::Repository;

pub fn process_item(app: &App, item: ItemJob) -> Result<()> {
    let source = PathBuf::from(&item.source_path);
    let conn = app.open_conn()?;
    let repo = Repository::new(&conn);
    let target = match app.config.target_by_id(&item.target_id) {
        Ok(target) => target,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_TARGET_MISSING", &err.to_string())?;
            error!(item_id = %item.item_id, target_id = %item.target_id, error = %err, "target lookup failed");
            return Ok(());
        }
    };

    repo.update_item_running(&item.item_id, "validating")?;
    let size = match validator::validate_file(&source, app.config.import.max_file_size_mb) {
        Ok(size) => size,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_SOURCE_INVALID", &err.to_string())?;
            error!(item_id = %item.item_id, error = %err, "validation failed");
            return Ok(());
        }
    };

    repo.update_item_running(&item.item_id, "hashing")?;
    let hash = match hasher::sha256_file(&source) {
        Ok(hash) => hash,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_HASH_FAILED", &err.to_string())?;
            error!(item_id = %item.item_id, error = %err, "hash failed");
            return Ok(());
        }
    };
    repo.update_item_hash(&item.item_id, &hash, size as i64)?;

    if let Some(existing_record_id) = deduper::find_duplicate_record(&repo, &item.target_id, &hash)?
    {
        repo.mark_item_duplicate(&item.item_id, &existing_record_id)?;
        warn!(item_id = %item.item_id, duplicate_of = %existing_record_id, "duplicate skipped");
        return Ok(());
    }

    repo.update_item_running(&item.item_id, "copying")?;
    let adapter = LocalFolderAdapter::new(&target.root_path);
    let dest = match adapter.store_copy(&source, &item.source_name) {
        Ok(dest) => dest,
        Err(err) => {
            repo.mark_item_failed(&item.item_id, "E_COPY_FAILED", &err.to_string())?;
            error!(item_id = %item.item_id, error = %err, "copy failed");
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
        if let Some(existing_record_id) =
            deduper::find_duplicate_record(&repo, &item.target_id, &record.sha256)?
        {
            repo.mark_item_duplicate(&item.item_id, &existing_record_id)?;
            warn!(
                item_id = %item.item_id,
                duplicate_of = %existing_record_id,
                error = %err,
                "manifest insert raced with existing duplicate"
            );
            return Ok(());
        }

        repo.mark_item_failed(&item.item_id, "E_MANIFEST_FAILED", &err.to_string())?;
        error!(item_id = %item.item_id, error = %err, "manifest insert failed");
        return Ok(());
    }
    repo.mark_item_success(&item.item_id, &record.stored_path)?;

    info!(item_id = %item.item_id, stored_path = %record.stored_path, "item imported");
    Ok(())
}
