/// Minimal i18n support.
///
/// Keys are dotted identifiers like "cli.no_input_paths".
/// `lang` should be "zh-CN" or "en" (default).
///
/// Returns a `String` so that unknown keys can be echoed back to the caller.
pub fn tr(key: &str, lang: &str) -> String {
    if lang != "zh-CN" {
        return en(key).to_string();
    }
    zh_cn(key).unwrap_or_else(|| en(key)).to_string()
}

fn en(key: &str) -> &'static str {
    match key {
        // -- CLI import --
        "cli.no_input_paths" => "no input paths provided",
        "cli.no_importable_files" => "no importable files found",
        "cli.queued_batch" => "Queued batch: {}",
        "cli.items_queued" => "Items queued: {}",
        "cli.target" => "Target: {}",
        "cli.routing_rule_single" => "Routing rule: {}",
        "cli.routing_rule_multiple" => "Routing rule: multiple",
        "cli.batch" => "Batch: {}",
        "cli.status" => "Status: {}",
        "cli.source" => "Source: {}",
        "cli.items" => "Items: {}",
        "cli.retried_items" => "Retried items: {retried}",
        "cli.batch_already_undone" => "Batch already undone: {}",
        "cli.config_dir" => "Config dir: {}",
        "cli.database" => "Database: {}",
        "cli.ok_check" => "[OK] {check}: {detail}",
        "cli.warn_check" => "[WARN] {check}: {detail}",
        "cli.warn_hint" => "  Hint: {hint}",
        "cli.fail_check" => "[FAIL] {check}: {detail}",
        "cli.fail_hint" => "  Hint: {hint}",
        "cli.config_valid" => "Config validation succeeded.",
        "cli.default_target" => "Default target: {}",
        "cli.target_path" => "Path: {}",
        "cli.target_list_header" => "default  target  name  status  path",
        "cli.item_target" => "Target: {}",
        "cli.item_name" => "Name: {}",
        "cli.item_status" => "Status: {}",
        "cli.item_path" => "Path: {}",
        "cli.added_target" => "Added target: {}",
        "cli.renamed_target" => "Renamed target: {}",
        "cli.archived_target" => "Archived target: {}",
        "cli.registered" => "Registered: HKCU\\{}",
        "cli.command" => "Command: {}",
        "cli.icon" => "Icon: {}",
        "cli.explorer_removed" => "Removed Explorer context-menu entries",
        "cli.com_feasibility" => "Windows 11 Explorer COM feasibility probe",
        "cli.explorer_run_import_hidden" => "explorer run-import is only intended for the hidden GUI launcher",
        "cli.no_active_targets" => "No active targets configured.",
        "cli.vault_target" => "Target: {0}{1}",
        "cli.vault_path" => "  Path:          {}",
        "cli.vault_files" => "  Files imported: {}",
        "cli.vault_storage" => "  Storage used:  {}",
        "cli.vault_failed" => "  Failed:        {}",
        "cli.service_status" => "Service status: {}",
        "cli.undone_prefix" => "Undone",
        "cli.undone_suffix_success" => "of {} items: {} succeeded, {} marked duplicate, {} failed",
        "cli.undone_suffix_partial" => "of {} items: {} succeeded, {} marked duplicate, {} failed (partially undone)",
        "cli.undone_suffix_failed" => "of {} items: {} failed (nothing to undo)",
        "cli.json_table_mutual" => "--json and --table cannot be used together",
        "cli.unsupported_status" => "unsupported status filter: {status}",
        "cli.dry_run_table_header" => "Source Path\tTarget\tRule\tDestination\tAction",

        // -- Vault audit --
        "cli.audit_target_header" => "Target: {0} — {1} issue(s)",
        "cli.audit_orphan" => "Orphan files (in vault, not in manifest): {}",
        "cli.audit_missing" => "Missing files (in manifest, file deleted): {}",
        "cli.audit_duplicate" => "Duplicate records (same SHA-256): {} extra",
        "cli.audit_malformed" => "Malformed frontmatter (missing kbintake_ fields): {}",
        "cli.audit_clean" => "No issues found.",
        "cli.audit_fix_summary" => "Fixed: {0} missing cleaned, {1} duplicates resolved",

        // -- Toast notifications --
        "toast.title" => "KBIntake",
        "toast.queued_single" => "Queued {count} item(s) for {target} using rule {rule}.",
        "toast.queued_multiple" => "Queued {count} item(s) for {target} using multiple rules.",
        "toast.queued_none" => "Queued {count} item(s) for {target}.",
        "toast.import_failed_before" => "Import failed before processing finished.",
        "toast.imported_single" => "Imported {count} file(s) into {target} using rule {rule}.",
        "toast.imported_multiple" => "Imported {count} file(s) into {target} using multiple rules.",
        "toast.imported_none" => "Imported {count} file(s) into {target}.",
        "toast.duplicate_skipped" => "{count} duplicate skipped.",
        "toast.no_duplicates" => "No duplicates skipped.",
        "toast.finish_failures_single" => "Import finished with {count} failure(s) after rule {rule}.",
        "toast.finish_failures_multiple" => "Import finished with {count} failure(s) after multiple rules.",
        "toast.finish_failures_none" => "Import finished with {count} failure(s).",
        "toast.retry_hint" => "Run: kbintake jobs retry {batch_id}",

        // -- Agent --
        "agent.processed" => "Processed items: {processed}",

        // -- TUI --
        "tui.config_saved" => "Configuration saved.",
        "tui.exiting" => "Exiting settings TUI.",
        "tui.footer" => "q: quit  s: save  a: add  r: remove  d: set default  e: edit  f: toggle frontmatter  l: language  +/-: adjust size",
        "tui.targets_title" => " Targets [a: add, r: remove, d: set default] ",
        "tui.import_title" => " Import Settings [f: frontmatter, l: language, +/-: size] ",
        "tui.watch_title" => " Watch Configs ",
        "tui.templates_title" => " Templates ",
        "tui.subfolder_col" => "Subfolder",
        "tui.default_col" => "Default",
        "tui.target_col" => "Target",
        "tui.name_col" => "Name",
        "tui.status_col" => "Status",
        "tui.path_col" => "Path",
        "tui.template_col" => "Template",
        "tui.extensions_col" => "Extensions",
        "tui.debounce_col" => "Debounce",
        "tui.base_col" => "Base",
        "tui.tags_col" => "Tags",
        "tui.max_file_size" => "Max file size:",
        "tui.frontmatter" => "Frontmatter injection:",
        "tui.language" => "Language:",
        "tui.size_up" => "Increase max size by 64MB",
        "tui.size_down" => "Decrease max size by 64MB",
        "tui.toggle_frontmatter" => "Toggle frontmatter injection",
        "tui.toggle_language" => "Toggle language (en / zh-CN)",
        "tui.no_watch_configs" => "No watch paths configured. Use `kbintake watch --add` to add one.",
        "tui.no_templates" => "No templates configured.",
        "tui.all_extensions" => "(all)",
        "tui.add_target_hint" => "Use `kbintake targets add <name> <path>` to add a target.",
        "tui.removed" => "Removed:",
        "tui.cannot_remove_last" => "Cannot remove the last active target.",
        "tui.default_changed" => "Default target updated.",
        "tui.vault_col" => "Vault",

        // -- Watcher --
        "watcher.duplicate" => "Another watcher instance is already running.",
        "toast.watch_import_ok_title" => "Watch Import OK",
        "toast.watch_import_ok" => "Imported: {file}",
        "toast.watch_import_warn_title" => "Watch Import Warning",
        "toast.watch_import_warn" => "Skipped or failed: {file}",
        "toast.watch_import_queued_title" => "Watch Import Queued",
        "toast.watch_import_queued" => "Queued: {file}",

        // -- Config --
        "config.target_already_configured" => "target already configured: {name}",
        "config.target_name_empty" => "target name cannot be empty",
        "config.target_name_invalid" => "target name may only contain letters, numbers, '-' and '_'",
        "config.target_archived" => "Target '{}' is archived and cannot be used.",

        // -- DB --
        "db.missing_table" => "missing database table: {table}",
        "db.missing_index" => "missing database index: {index}",
        "db.schema_outdated" => "schema version out of date: {version} != {LATEST_SCHEMA_VERSION}",

        // -- Explorer --
        "explorer.install_unsupported" => "Explorer context-menu installation is only supported on Windows",
        "explorer.uninstall_unsupported" => "Explorer context-menu uninstallation is only supported on Windows",
        "explorer.menu_file" => "Add to Knowledge Base",
        "explorer.menu_dir" => "Add Folder to Knowledge Base",

        // -- Service --
        "service.app_data_init" => "service app data directory already initialized",
        "service.stop_channel_init" => "service stop channel already initialized",
        "service.install_unsupported" => "service install is only supported on Windows",
        "service.start_unsupported" => "service start is only supported on Windows",
        "service.stop_unsupported" => "service stop is only supported on Windows",
        "service.uninstall_unsupported" => "service uninstall is only supported on Windows",
        "service.run_unsupported" => "service run is only supported on Windows",
        "service.started" => "Service '{}' started.",
        "service.already_stopped" => "Service '{}' is already stopped.",
        "service.stopped" => "Service '{}' stopped.",
        "service.removed" => "Service '{}' removed.",

        // -- Processor --
        "processor.no_input_paths" => "no input paths provided",
        "processor.no_importable_files" => "no importable files found",
        "processor.path_not_exist" => "input path does not exist: {}",
        "processor.file_not_exist" => "source file does not exist: {}",
        "processor.not_a_file" => "source path is not a file: {}",
        "processor.file_exceeds_size" => "source file exceeds max size of {} MB",
        "processor.template_self_inherit" => "template '{}' cannot inherit from itself",
        "processor.template_name_empty" => "template name cannot be empty",
        "processor.template_duplicate" => "duplicate template name '{}'",
        "processor.template_unterminated" => "unterminated string literal",
        "processor.file_locked" => "file locked after {} retries",

        // -- Queue --
        "queue.not_found" => "{entity} not found: {id}",

        // -- Error prefix --
        "error.prefix" => "ERROR [{code}]: {err:#}",

        _ => "<missing i18n key>",
    }
}

fn zh_cn(key: &str) -> Option<&'static str> {
    match key {
        // -- CLI import --
        "cli.no_input_paths" => Some("未提供输入路径"),
        "cli.no_importable_files" => Some("未找到可导入的文件"),
        "cli.queued_batch" => Some("已排队批次: {}"),
        "cli.items_queued" => Some("已排队项目: {}"),
        "cli.target" => Some("目标: {}"),
        "cli.routing_rule_single" => Some("路由规则: {}"),
        "cli.routing_rule_multiple" => Some("路由规则: 多个"),
        "cli.batch" => Some("批次: {}"),
        "cli.status" => Some("状态: {}"),
        "cli.source" => Some("来源: {}"),
        "cli.items" => Some("项目: {}"),
        "cli.retried_items" => Some("已重试项目: {retried}"),
        "cli.batch_already_undone" => Some("批次已撤销: {}"),
        "cli.config_dir" => Some("配置目录: {}"),
        "cli.database" => Some("数据库: {}"),
        "cli.ok_check" => Some("[成功] {check}: {detail}"),
        "cli.warn_check" => Some("[警告] {check}: {detail}"),
        "cli.warn_hint" => Some("  提示: {hint}"),
        "cli.fail_check" => Some("[失败] {check}: {detail}"),
        "cli.fail_hint" => Some("  提示: {hint}"),
        "cli.config_valid" => Some("配置验证通过。"),
        "cli.default_target" => Some("默认目标: {}"),
        "cli.target_path" => Some("路径: {}"),
        "cli.target_list_header" => Some("默认  目标  名称  状态  路径"),
        "cli.item_target" => Some("目标: {}"),
        "cli.item_name" => Some("名称: {}"),
        "cli.item_status" => Some("状态: {}"),
        "cli.item_path" => Some("路径: {}"),
        "cli.added_target" => Some("已添加目标: {}"),
        "cli.renamed_target" => Some("已重命名目标: {}"),
        "cli.archived_target" => Some("已归档目标: {}"),
        "cli.registered" => Some("已注册: HKCU\\{}"),
        "cli.command" => Some("命令: {}"),
        "cli.icon" => Some("图标: {}"),
        "cli.explorer_removed" => Some("已移除 Explorer 右键菜单项"),
        "cli.com_feasibility" => Some("Windows 11 Explorer COM 可行性探测"),
        "cli.explorer_run_import_hidden" => Some("explorer run-import 仅供隐藏式 GUI 启动器使用"),
        "cli.no_active_targets" => Some("未配置活跃目标。"),
        "cli.vault_target" => Some("目标: {0}{1}"),
        "cli.vault_path" => Some("  路径:          {}"),
        "cli.vault_files" => Some("  已导入文件: {}"),
        "cli.vault_storage" => Some("  已用存储:    {}"),
        "cli.vault_failed" => Some("  失败:        {}"),
        "cli.service_status" => Some("服务状态: {}"),
        "cli.undone_prefix" => Some("已撤销"),
        "cli.undone_suffix_success" => Some("共 {} 个项目: {} 成功, {} 标记为重复, {} 失败"),
        "cli.undone_suffix_partial" => Some("共 {} 个项目: {} 成功, {} 标记为重复, {} 失败 (部分撤销)"),
        "cli.undone_suffix_failed" => Some("共 {} 个项目: {} 失败 (无可撤销内容)"),
        "cli.json_table_mutual" => Some("--json 和 --table 不能同时使用"),
        "cli.unsupported_status" => Some("不支持的状态过滤: {status}"),
        "cli.dry_run_table_header" => Some("源路径\t目标\t规则\t目标位置\t操作"),

        // -- Vault audit --
        "cli.audit_target_header" => Some("目标: {0} — {1} 个问题"),
        "cli.audit_orphan" => Some("孤立文件 (在 vault 中但不在 manifest 中): {}"),
        "cli.audit_missing" => Some("缺失文件 (在 manifest 中但文件已删除): {}"),
        "cli.audit_duplicate" => Some("重复记录 (相同 SHA-256): {} 条多余"),
        "cli.audit_malformed" => Some("格式异常的 frontmatter (缺少 kbintake_ 字段): {}"),
        "cli.audit_clean" => Some("未发现问题。"),
        "cli.audit_fix_summary" => Some("已修复: {0} 条缺失记录已清理, {1} 条重复已解决"),

        // -- Toast notifications --
        "toast.title" => Some("KBIntake"),
        "toast.queued_single" => Some("已排队 {count} 个项目到 {target}，使用规则 {rule}。"),
        "toast.queued_multiple" => Some("已排队 {count} 个项目到 {target}，使用多个规则。"),
        "toast.queued_none" => Some("已排队 {count} 个项目到 {target}。"),
        "toast.import_failed_before" => Some("导入在处理完成前失败。"),
        "toast.imported_single" => Some("已导入 {count} 个文件到 {target}，使用规则 {rule}。"),
        "toast.imported_multiple" => Some("已导入 {count} 个文件到 {target}，使用多个规则。"),
        "toast.imported_none" => Some("已导入 {count} 个文件到 {target}。"),
        "toast.duplicate_skipped" => Some("跳过 {count} 个重复项。"),
        "toast.no_duplicates" => Some("无重复项跳过。"),
        "toast.finish_failures_single" => Some("导入完成，规则 {rule} 后有 {count} 个失败。"),
        "toast.finish_failures_multiple" => Some("导入完成，多个规则后有 {count} 个失败。"),
        "toast.finish_failures_none" => Some("导入完成，{count} 个失败。"),
        "toast.retry_hint" => Some("运行: kbintake jobs retry {batch_id}"),

        // -- Agent --
        "agent.processed" => Some("已处理项目: {processed}"),

        // -- TUI --
        "tui.config_saved" => Some("配置已保存。"),
        "tui.exiting" => Some("退出设置界面。"),
        "tui.footer" => Some("q: 退出  s: 保存  a: 添加  r: 移除  d: 设默认  e: 编辑  f: 切换 frontmatter  l: 语言  +/-: 调整大小"),
        "tui.targets_title" => Some(" 目标 [a: 添加, r: 移除, d: 设默认] "),
        "tui.import_title" => Some(" 导入设置 [f: frontmatter, l: 语言, +/-: 大小] "),
        "tui.watch_title" => Some(" 监控配置 "),
        "tui.templates_title" => Some(" 模板 "),
        "tui.subfolder_col" => Some("子文件夹"),
        "tui.default_col" => Some("默认"),
        "tui.target_col" => Some("目标"),
        "tui.name_col" => Some("名称"),
        "tui.status_col" => Some("状态"),
        "tui.path_col" => Some("路径"),
        "tui.template_col" => Some("模板"),
        "tui.extensions_col" => Some("扩展名"),
        "tui.debounce_col" => Some("防抖"),
        "tui.base_col" => Some("基础"),
        "tui.tags_col" => Some("标签"),
        "tui.max_file_size" => Some("最大文件大小:"),
        "tui.frontmatter" => Some("Frontmatter 注入:"),
        "tui.language" => Some("语言:"),
        "tui.size_up" => Some("增加 64MB"),
        "tui.size_down" => Some("减少 64MB"),
        "tui.toggle_frontmatter" => Some("切换 frontmatter 注入"),
        "tui.toggle_language" => Some("切换语言 (en / zh-CN)"),
        "tui.no_watch_configs" => Some("未配置监控路径。使用 `kbintake watch --add` 添加。"),
        "tui.no_templates" => Some("未配置模板。"),
        "tui.all_extensions" => Some("(全部)"),
        "tui.add_target_hint" => Some("使用 `kbintake targets add <名称> <路径>` 添加目标。"),
        "tui.removed" => Some("已移除:"),
        "tui.cannot_remove_last" => Some("不能移除最后一个活跃目标。"),
        "tui.default_changed" => Some("默认目标已更新。"),
        "tui.vault_col" => Some("Vault"),

        // -- Watcher --
        "watcher.duplicate" => Some("已有另一个监控实例在运行。"),
        "toast.watch_import_ok_title" => Some("监控导入成功"),
        "toast.watch_import_ok" => Some("已导入: {file}"),
        "toast.watch_import_warn_title" => Some("监控导入警告"),
        "toast.watch_import_warn" => Some("已跳过或失败: {file}"),
        "toast.watch_import_queued_title" => Some("监控导入已排队"),
        "toast.watch_import_queued" => Some("已排队: {file}"),

        // -- Config --
        "config.target_already_configured" => Some("目标已配置: {name}"),
        "config.target_name_empty" => Some("目标名称不能为空"),
        "config.target_name_invalid" => Some("目标名称只能包含字母、数字、'-' 和 '_'"),
        "config.target_archived" => Some("目标 '{}' 已归档，无法使用。"),

        // -- DB --
        "db.missing_table" => Some("缺少数据库表: {table}"),
        "db.missing_index" => Some("缺少数据库索引: {index}"),
        "db.schema_outdated" => Some("数据库版本过旧: {version} != {LATEST_SCHEMA_VERSION}"),

        // -- Explorer --
        "explorer.install_unsupported" => Some("Explorer 右键菜单安装仅在 Windows 上支持"),
        "explorer.uninstall_unsupported" => Some("Explorer 右键菜单卸载仅在 Windows 上支持"),
        "explorer.menu_file" => Some("添加到知识库"),
        "explorer.menu_dir" => Some("添加文件夹到知识库"),

        // -- Service --
        "service.app_data_init" => Some("服务数据目录已初始化"),
        "service.stop_channel_init" => Some("服务停止通道已初始化"),
        "service.install_unsupported" => Some("服务安装仅在 Windows 上支持"),
        "service.start_unsupported" => Some("服务启动仅在 Windows 上支持"),
        "service.stop_unsupported" => Some("服务停止仅在 Windows 上支持"),
        "service.uninstall_unsupported" => Some("服务卸载仅在 Windows 上支持"),
        "service.run_unsupported" => Some("服务运行仅在 Windows 上支持"),
        "service.started" => Some("服务 '{}' 已启动。"),
        "service.already_stopped" => Some("服务 '{}' 已处于停止状态。"),
        "service.stopped" => Some("服务 '{}' 已停止。"),
        "service.removed" => Some("服务 '{}' 已移除。"),

        // -- Processor --
        "processor.no_input_paths" => Some("未提供输入路径"),
        "processor.no_importable_files" => Some("未找到可导入的文件"),
        "processor.path_not_exist" => Some("输入路径不存在: {}"),
        "processor.file_not_exist" => Some("源文件不存在: {}"),
        "processor.not_a_file" => Some("源路径不是文件: {}"),
        "processor.file_exceeds_size" => Some("源文件超过最大大小 {} MB"),
        "processor.template_self_inherit" => Some("模板 '{}' 不能继承自身"),
        "processor.template_name_empty" => Some("模板名称不能为空"),
        "processor.template_duplicate" => Some("重复的模板名称 '{}'"),
        "processor.template_unterminated" => Some("未终止的字符串字面量"),
        "processor.file_locked" => Some("文件在 {} 次重试后仍被锁定"),

        // -- Queue --
        "queue.not_found" => Some("{entity} 未找到: {id}"),

        // -- Error prefix --
        "error.prefix" => Some("错误 [{code}]: {err:#}"),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{en, tr, zh_cn};

    #[test]
    fn english_returns_english_for_unknown_lang() {
        assert_eq!(tr("cli.no_input_paths", "en"), "no input paths provided");
        assert_eq!(tr("cli.no_input_paths", "fr"), "no input paths provided");
    }

    #[test]
    fn zh_cn_returns_chinese_for_known_key() {
        assert_eq!(tr("cli.no_input_paths", "zh-CN"), "未提供输入路径");
        assert_eq!(tr("cli.no_importable_files", "zh-CN"), "未找到可导入的文件");
        assert_eq!(tr("toast.title", "zh-CN"), "KBIntake");
    }

    #[test]
    fn zh_cn_falls_back_to_english_for_missing_key() {
        assert_eq!(tr("nonexistent.key", "zh-CN"), "<missing i18n key>");
    }

    #[test]
    fn english_cover_all_keys() {
        // Every key that zh_cn has must also exist in en.
        let test_keys = [
            "cli.no_input_paths",
            "cli.no_importable_files",
            "toast.title",
            "toast.queued_single",
            "config.target_already_configured",
            "db.missing_table",
            "explorer.install_unsupported",
            "service.started",
            "processor.file_not_exist",
            "queue.not_found",
            "error.prefix",
        ];
        for key in test_keys {
            let result = en(key);
            assert!(result != "<missing i18n key>", "en() missing key: {key}");
            assert!(zh_cn(key).is_some(), "zh_cn() missing key: {key}");
        }
    }
}
