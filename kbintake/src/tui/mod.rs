use std::io;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Terminal,
};

use crate::config::{AppConfig, StringList, WatchConfig};
use crate::domain::{BatchJob, ItemJob};
use crate::i18n::tr;

/// Active tab in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabId {
    Targets = 0,
    Import = 1,
    Watch = 2,
    Templates = 3,
    Jobs = 4,
    Service = 5,
}

/// Text input mode for the overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InputMode {
    Normal,
    AddingTargetName,
    AddingTargetPath,
    AddingWatchPath,
    EditingTargetVault(usize),
    EditingWatchPath(usize),
    EditingWatchTarget(usize),
    EditingWatchExtensions(usize),
    EditingWatchDebounce(usize),
    EditingWatchTemplate(usize),
}

/// Top-level TUI state.
struct SettingsUi {
    config: AppConfig,
    active_tab: TabId,
    selected_index: usize,
    message: String,
    pending_save: bool,
    input_mode: InputMode,
    input_buffer: String,
    pending_target_name: Option<String>,
    // Jobs tab state
    app_data_dir: PathBuf,
    jobs_cache: Vec<BatchJob>,
    items_cache: Vec<ItemJob>,
    showing_items_for: Option<String>,
    // Service tab state
    service_status: String,
    explorer_installed: bool,
}

impl SettingsUi {
    fn new(config: AppConfig) -> Self {
        let app_data_dir = config.app_data_dir.clone();
        Self {
            config,
            active_tab: TabId::Targets,
            selected_index: 0,
            message: String::new(),
            pending_save: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            pending_target_name: None,
            app_data_dir,
            jobs_cache: Vec::new(),
            items_cache: Vec::new(),
            showing_items_for: None,
            service_status: String::new(),
            explorer_installed: false,
        }
    }

    fn save_config(&mut self) -> Result<()> {
        self.config.save()?;
        self.message = tr("tui.config_saved", self.config.language());
        Ok(())
    }

    fn lang(&self) -> &str {
        self.config.language()
    }

    fn tab_titles(&self) -> Vec<Span<'static>> {
        let keys = [
            "tui.tab_targets",
            "tui.tab_import",
            "tui.tab_watch",
            "tui.tab_templates",
            "tui.tab_jobs",
            "tui.tab_service",
        ];
        keys.iter()
            .map(|k| Span::raw(tr(k, self.lang()).to_string()))
            .collect()
    }

    fn item_count(&self) -> usize {
        match self.active_tab {
            TabId::Targets => self.config.targets.len(),
            TabId::Watch => self.config.watch.len(),
            TabId::Templates => self.config.templates.len(),
            TabId::Jobs => {
                if self.showing_items_for.is_some() {
                    self.items_cache.len()
                } else {
                    self.jobs_cache.len()
                }
            }
            TabId::Import | TabId::Service => 0,
        }
    }

    fn clamp_selected(&mut self) {
        let count = self.item_count();
        if count > 0 && self.selected_index >= count {
            self.selected_index = count - 1;
        }
    }

    fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn move_down(&mut self) {
        let count = self.item_count();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
        }
    }

    fn open_conn(&self) -> Result<rusqlite::Connection> {
        let db_path = self.app_data_dir.join("data").join("kbintake.db");
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.execute_batch("PRAGMA journal_mode=WAL")?;
        Ok(conn)
    }
}

/// Run the interactive settings TUI.
pub fn run_settings_tui(config: AppConfig) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut ui = SettingsUi::new(config);

    // Pre-load data for Jobs and Service tabs
    refresh_jobs(&mut ui);
    refresh_service_status(&mut ui);

    let result = run_loop(&mut terminal, &mut ui);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    if let Ok(()) = result {
        println!("{}", tr("tui.exiting", ui.lang()));
    } else {
        eprintln!("{}", tr("tui.exiting", ui.lang()));
    }

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ui: &mut SettingsUi,
) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            render(frame, ui);
            if ui.input_mode != InputMode::Normal {
                render_input_overlay(frame, ui);
            }
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Handle text input mode first.
            if ui.input_mode != InputMode::Normal && handle_text_input(ui, key.code) {
                continue;
            }

            // Jobs detail view has limited keys
            if ui.showing_items_for.is_some() && ui.active_tab == TabId::Jobs {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Backspace => {
                        ui.showing_items_for = None;
                        ui.selected_index = 0;
                    }
                    KeyCode::Up => ui.move_up(),
                    KeyCode::Down => ui.move_down(),
                    _ => {}
                }
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('1') => {
                    ui.active_tab = TabId::Targets;
                    ui.selected_index = 0;
                }
                KeyCode::Char('2') => {
                    ui.active_tab = TabId::Import;
                }
                KeyCode::Char('3') => {
                    ui.active_tab = TabId::Watch;
                    ui.selected_index = 0;
                }
                KeyCode::Char('4') => {
                    ui.active_tab = TabId::Templates;
                    ui.selected_index = 0;
                }
                KeyCode::Char('5') => {
                    ui.active_tab = TabId::Jobs;
                    ui.selected_index = 0;
                    refresh_jobs(ui);
                }
                KeyCode::Char('6') => {
                    ui.active_tab = TabId::Service;
                    refresh_service_status(ui);
                }
                KeyCode::Up => ui.move_up(),
                KeyCode::Down => ui.move_down(),
                KeyCode::Char('s') => {
                    if ui.active_tab == TabId::Service {
                        start_service(ui);
                    } else if let Err(e) = ui.save_config() {
                        ui.message = format!("ERROR: {e:#}");
                    } else {
                        ui.pending_save = false;
                    }
                }
                KeyCode::Char('S') if ui.active_tab == TabId::Service => {
                    stop_service(ui);
                }
                KeyCode::Char('a') => handle_add(ui),
                KeyCode::Char('r') if ui.active_tab == TabId::Jobs => {
                    retry_selected_job(ui);
                }
                KeyCode::Char('r') => handle_remove(ui),
                KeyCode::Char('u') if ui.active_tab == TabId::Jobs => {
                    undo_selected_job(ui);
                }
                KeyCode::Char('u') if ui.active_tab == TabId::Service => {
                    uninstall_service(ui);
                }
                KeyCode::Char('d') => handle_default(ui),
                KeyCode::Char('f') => handle_toggle_frontmatter(ui),
                KeyCode::Char('l') => handle_toggle_language(ui),
                KeyCode::Char('e') => handle_edit(ui),
                KeyCode::Char('t') => handle_edit_watch_field(ui, InputField::Target),
                KeyCode::Char('x') => handle_edit_watch_field(ui, InputField::Extensions),
                KeyCode::Char('b') => handle_edit_watch_field(ui, InputField::Debounce),
                KeyCode::Char('p') => handle_edit_watch_field(ui, InputField::Template),
                KeyCode::Char('+') | KeyCode::Char('-') => handle_size_adjust(ui, key.code),
                KeyCode::Char('i') if ui.active_tab == TabId::Service => {
                    install_service(ui);
                }
                KeyCode::Char('m') if ui.active_tab == TabId::Service => {
                    install_explorer(ui);
                }
                KeyCode::Char('M') if ui.active_tab == TabId::Service => {
                    uninstall_explorer(ui);
                }
                KeyCode::Char('w') if ui.active_tab == TabId::Service => {
                    toggle_watch_in_service(ui);
                }
                KeyCode::Char('D') if ui.active_tab == TabId::Service => {
                    run_doctor(ui);
                }
                KeyCode::Enter if ui.active_tab == TabId::Jobs => {
                    show_job_detail(ui);
                }
                KeyCode::F(5) if ui.active_tab == TabId::Jobs => {
                    refresh_jobs(ui);
                }
                KeyCode::F(5) if ui.active_tab == TabId::Service => {
                    refresh_service_status(ui);
                }
                _ => {}
            }
        }
    }
}

enum InputField {
    Target,
    Extensions,
    Debounce,
    Template,
}

fn handle_edit_watch_field(ui: &mut SettingsUi, field: InputField) {
    if ui.active_tab != TabId::Watch || ui.config.watch.is_empty() {
        return;
    }
    let idx = ui.selected_index;
    if idx >= ui.config.watch.len() {
        return;
    }
    ui.input_buffer.clear();
    ui.message.clear();
    ui.input_mode = match field {
        InputField::Target => InputMode::EditingWatchTarget(idx),
        InputField::Extensions => InputMode::EditingWatchExtensions(idx),
        InputField::Debounce => InputMode::EditingWatchDebounce(idx),
        InputField::Template => InputMode::EditingWatchTemplate(idx),
    };
}

/// Handle keys in text input mode. Returns true if the key was consumed.
fn handle_text_input(ui: &mut SettingsUi, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc => {
            ui.input_mode = InputMode::Normal;
            ui.input_buffer.clear();
            true
        }
        KeyCode::Enter => {
            let input = std::mem::take(&mut ui.input_buffer);
            match &ui.input_mode {
                InputMode::AddingTargetName => {
                    let input_trimmed = input.trim().to_string();
                    if input_trimmed.is_empty() {
                        ui.message = "Name cannot be empty".to_string();
                        ui.input_mode = InputMode::Normal;
                        return true;
                    }
                    if !input_trimmed
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    {
                        ui.message = "Name: only letters, numbers, '-', '_' allowed".to_string();
                        ui.input_mode = InputMode::Normal;
                        return true;
                    }
                    ui.pending_target_name = Some(input_trimmed);
                    ui.input_buffer.clear();
                    ui.input_mode = InputMode::AddingTargetPath;
                    ui.message.clear();
                    try_folder_picker(ui);
                    if !ui.input_buffer.is_empty() {
                        let code = KeyCode::Enter;
                        handle_text_input(ui, code);
                    }
                }
                InputMode::AddingTargetPath => {
                    let path = PathBuf::from(&input);
                    let name = ui.pending_target_name.take().unwrap_or_default();
                    match crate::config::validate_target_root(&path) {
                        Ok(()) => match ui.config.add_target(name, path) {
                            Ok(t) => {
                                ui.message =
                                    format!("{} {}", tr("cli.added_target", ui.lang()), t.name);
                                ui.pending_save = true;
                                ui.selected_index = ui.config.targets.len() - 1;
                            }
                            Err(e) => ui.message = format!("ERROR: {e:#}"),
                        },
                        Err(e) => ui.message = format!("Invalid path: {e:#}"),
                    }
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::AddingWatchPath => {
                    let path = PathBuf::from(&input);
                    if !path.is_dir() {
                        ui.message = format!("Directory does not exist: {}", path.display());
                        ui.input_mode = InputMode::Normal;
                        ui.input_buffer.clear();
                        return true;
                    }
                    ui.config.watch.push(WatchConfig {
                        path,
                        target: None,
                        debounce_secs: 2,
                        extensions: None,
                        template: None,
                    });
                    ui.selected_index = ui.config.watch.len() - 1;
                    ui.pending_save = true;
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::EditingTargetVault(idx) => {
                    let idx = *idx;
                    if input.trim().is_empty() {
                        ui.config.targets[idx].obsidian_vault = None;
                    } else {
                        ui.config.targets[idx].obsidian_vault = Some(input.trim().to_string());
                    }
                    ui.pending_save = true;
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::EditingWatchPath(idx) => {
                    let idx = *idx;
                    let path = PathBuf::from(&input);
                    if !path.is_dir() {
                        ui.message = format!("Directory does not exist: {}", path.display());
                        ui.input_mode = InputMode::Normal;
                        ui.input_buffer.clear();
                        return true;
                    }
                    ui.config.watch[idx].path = path;
                    ui.pending_save = true;
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::EditingWatchTarget(idx) => {
                    let idx = *idx;
                    let trimmed = input.trim().to_string();
                    ui.config.watch[idx].target = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    };
                    ui.pending_save = true;
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::EditingWatchExtensions(idx) => {
                    let idx = *idx;
                    let trimmed = input.trim().to_string();
                    if trimmed.is_empty() {
                        ui.config.watch[idx].extensions = None;
                    } else {
                        let exts: Vec<String> = trimmed
                            .split(',')
                            .map(|e| {
                                let e = e.trim().to_string();
                                if e.starts_with('.') {
                                    e
                                } else {
                                    format!(".{e}")
                                }
                            })
                            .collect();
                        ui.config.watch[idx].extensions = Some(StringList::Many(exts));
                    }
                    ui.pending_save = true;
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::EditingWatchDebounce(idx) => {
                    let idx = *idx;
                    match input.trim().parse::<u64>() {
                        Ok(secs) if secs > 0 => {
                            ui.config.watch[idx].debounce_secs = secs;
                        }
                        _ => {
                            ui.message = "Must be a positive number of seconds".to_string();
                        }
                    }
                    ui.pending_save = true;
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::EditingWatchTemplate(idx) => {
                    let idx = *idx;
                    let trimmed = input.trim().to_string();
                    ui.config.watch[idx].template = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed)
                    };
                    ui.pending_save = true;
                    ui.input_mode = InputMode::Normal;
                    ui.input_buffer.clear();
                }
                InputMode::Normal => {}
            }
            true
        }
        KeyCode::Backspace => {
            ui.input_buffer.pop();
            true
        }
        KeyCode::Tab => {
            try_path_complete(ui);
            true
        }
        KeyCode::Char('o') if is_path_input_mode(&ui.input_mode) => {
            try_folder_picker(ui);
            true
        }
        KeyCode::Char(c) => {
            ui.input_buffer.push(c);
            true
        }
        _ => false,
    }
}

fn render(frame: &mut ratatui::Frame, ui: &SettingsUi) {
    let lang = ui.lang();
    let size = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(size);

    // Title with unsaved marker
    let title_text = if ui.pending_save {
        format!("{} {}", tr("tui.title", lang), tr("tui.unsaved", lang))
    } else {
        tr("tui.title", lang).to_string()
    };

    // Tabs
    let titles = ui.tab_titles();
    let active_index = ui.active_tab as usize;
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(title_text))
        .select(active_index)
        .style(Style::default())
        .highlight_style(Style::default().fg(Color::Yellow));
    frame.render_widget(tabs, chunks[0]);

    // Content
    match ui.active_tab {
        TabId::Targets => render_targets(frame, ui, chunks[1]),
        TabId::Import => render_import(frame, ui, chunks[1]),
        TabId::Watch => render_watch(frame, ui, chunks[1]),
        TabId::Templates => render_templates(frame, ui, chunks[1]),
        TabId::Jobs => render_jobs(frame, ui, chunks[1]),
        TabId::Service => render_service(frame, ui, chunks[1]),
    }

    // Contextual footer
    let help_key = match ui.active_tab {
        TabId::Targets => "tui.help_targets",
        TabId::Import => "tui.help_import",
        TabId::Watch => "tui.help_watch",
        TabId::Templates => "tui.help_templates",
        TabId::Jobs => {
            if ui.showing_items_for.is_some() {
                "tui.help_jobs_detail"
            } else {
                "tui.help_jobs"
            }
        }
        TabId::Service => "tui.help_service",
    };
    let footer_text = if ui.message.is_empty() {
        tr(help_key, lang).to_string()
    } else {
        format!("\u{26a0} {}", ui.message)
    };
    let footer =
        Paragraph::new(Span::raw(&footer_text)).block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

fn render_input_overlay(frame: &mut ratatui::Frame, ui: &SettingsUi) {
    let lang = ui.lang();
    let area = frame.area();
    let overlay_area = Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4).max(40),
        height: 7,
    };

    let (prompt, step_hint): (String, String) = match &ui.input_mode {
        InputMode::AddingTargetName => (
            "Target name: ".to_string(),
            "alphanumeric, '-', '_'".to_string(),
        ),
        InputMode::AddingTargetPath => ("Vault path: ".to_string(), String::new()),
        InputMode::AddingWatchPath => (
            format!("{}: ", tr("tui.prompt_watch_path", lang)),
            String::new(),
        ),
        InputMode::EditingTargetVault(idx) => {
            let current = ui.config.targets[*idx]
                .obsidian_vault
                .as_deref()
                .unwrap_or("")
                .to_string();
            (
                format!("{}: ", tr("tui.prompt_obsidian_vault", lang)),
                if current.is_empty() {
                    tr("tui.obsidian_hint", lang).to_string()
                } else {
                    current
                },
            )
        }
        InputMode::EditingWatchPath(idx) => (
            format!("{}: ", tr("tui.prompt_watch_path", lang)),
            ui.config.watch[*idx].path.display().to_string(),
        ),
        InputMode::EditingWatchTarget(idx) => (
            format!("{}: ", tr("tui.prompt_watch_target", lang)),
            ui.config.watch[*idx]
                .target
                .as_deref()
                .unwrap_or("")
                .to_string(),
        ),
        InputMode::EditingWatchExtensions(idx) => (
            format!("{}: ", tr("tui.prompt_watch_extensions", lang)),
            ui.config.watch[*idx]
                .extensions
                .as_ref()
                .map(|e| e.values().join(", "))
                .unwrap_or_default(),
        ),
        InputMode::EditingWatchDebounce(idx) => (
            format!("{}: ", tr("tui.prompt_watch_debounce", lang)),
            format!("{}s", ui.config.watch[*idx].debounce_secs),
        ),
        InputMode::EditingWatchTemplate(idx) => (
            format!("{}: ", tr("tui.prompt_watch_template", lang)),
            ui.config.watch[*idx]
                .template
                .as_deref()
                .unwrap_or("")
                .to_string(),
        ),
        InputMode::Normal => (String::new(), String::new()),
    };

    let title = match &ui.input_mode {
        InputMode::AddingTargetName => " Add Target (1/2) ",
        InputMode::AddingTargetPath => " Add Target (2/2) ",
        InputMode::AddingWatchPath => " Add Watch Path ",
        InputMode::EditingTargetVault(_) => " Edit Vault Name ",
        InputMode::EditingWatchPath(_) => " Edit Watch Path ",
        InputMode::EditingWatchTarget(_) => " Edit Watch Target ",
        InputMode::EditingWatchExtensions(_) => " Edit Extensions ",
        InputMode::EditingWatchDebounce(_) => " Edit Debounce ",
        InputMode::EditingWatchTemplate(_) => " Edit Template ",
        InputMode::Normal => " Input ",
    };

    let mut lines = vec![
        Line::from(Span::raw(format!("{prompt}[{step_hint}]"))),
        Line::from(Span::styled(
            &ui.input_buffer,
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            if is_path_input_mode(&ui.input_mode) {
                "[Enter] confirm  [Esc] cancel  [Tab] complete  [o] browse folder"
            } else {
                "[Enter] confirm  [Esc] cancel  [Backspace] delete"
            },
            Style::default().fg(Color::DarkGray),
        )),
    ];

    if let InputMode::AddingTargetPath = ui.input_mode {
        if let Some(ref name) = ui.pending_target_name {
            lines.insert(1, Line::from(Span::raw(format!("Name: {name}"))));
        }
    }

    let block = Block::default().borders(Borders::ALL).title(title);
    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(Clear, overlay_area);
    frame.render_widget(paragraph, overlay_area);
}

fn render_targets(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect) {
    let lang = ui.lang();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let desc = Paragraph::new(Span::styled(
        tr("tui.desc_targets", lang),
        Style::default().fg(Color::Cyan),
    ));
    frame.render_widget(desc, chunks[0]);

    let header = Row::new(vec![
        Cell::from(tr("tui.default_col", lang)),
        Cell::from(tr("tui.name_col", lang)),
        Cell::from(tr("tui.status_col", lang)),
        Cell::from(tr("tui.path_col", lang)),
        Cell::from(tr("tui.vault_col", lang)),
    ])
    .style(Style::default().fg(Color::Yellow));

    let rows: Vec<Row> = ui
        .config
        .targets
        .iter()
        .enumerate()
        .map(|(i, target)| {
            let default_marker = if i == 0 && target.is_active() {
                "*"
            } else {
                ""
            };
            let row = Row::new(vec![
                Cell::from(default_marker),
                Cell::from(target.name.clone()),
                Cell::from(target.status.clone()),
                Cell::from(target.root_path.display().to_string()),
                Cell::from(target.obsidian_vault.as_deref().unwrap_or("")),
            ]);
            if i == ui.selected_index {
                row.style(Style::default().bg(Color::DarkGray).fg(Color::White))
            } else {
                row
            }
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Min(15),
            Constraint::Length(15),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.tab_targets", lang)),
    );
    frame.render_widget(table, chunks[1]);
}

fn render_import(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect) {
    let lang = ui.lang();
    let fm_status = if ui.config.import.inject_frontmatter {
        "ON"
    } else {
        "OFF"
    };
    let lang_display = ui.config.import.language.as_deref().unwrap_or("en");
    let lang_label = if lang_display == "zh-CN" {
        "\u{7b80}\u{4f53}\u{4e2d}\u{6587}"
    } else {
        "English"
    };

    let lines = vec![
        Line::from(Span::styled(
            tr("tui.desc_import", lang),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::default()),
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.max_file_size", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{} MB", ui.config.import.max_file_size_mb)),
            Span::styled("  [+/-]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.frontmatter", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(fm_status),
            Span::styled("  [f]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.language", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(lang_label),
            Span::styled("  [l]", Style::default().fg(Color::DarkGray)),
        ]),
    ];
    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.tab_import", lang)),
    );
    frame.render_widget(para, area);
}

fn render_watch(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect) {
    let lang = ui.lang();
    if ui.config.watch.is_empty() {
        let lines = vec![
            Line::from(Span::styled(
                tr("tui.desc_watch", lang),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::default()),
            Line::from(tr("tui.no_watch_configs", lang)),
        ];
        let para = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr("tui.tab_watch", lang)),
        );
        frame.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let desc = Paragraph::new(Span::styled(
        tr("tui.desc_watch", lang),
        Style::default().fg(Color::Cyan),
    ));
    frame.render_widget(desc, chunks[0]);

    let header = Row::new(vec![
        Cell::from(tr("tui.path_col", lang)),
        Cell::from(tr("tui.target_col", lang)),
        Cell::from(tr("tui.template_col", lang)),
        Cell::from(tr("tui.extensions_col", lang)),
        Cell::from(tr("tui.debounce_col", lang)),
    ])
    .style(Style::default().fg(Color::Yellow));

    let rows: Vec<Row> = ui
        .config
        .watch
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let row = Row::new(vec![
                Cell::from(w.path.display().to_string()),
                Cell::from(w.target.as_deref().unwrap_or("(default)")),
                Cell::from(w.template.as_deref().unwrap_or("(none)")),
                Cell::from(
                    w.extensions
                        .as_ref()
                        .map(|e| e.values().join(", "))
                        .unwrap_or_else(|| tr("tui.all_extensions", lang)),
                ),
                Cell::from(format!("{}s", w.debounce_secs)),
            ]);
            if i == ui.selected_index {
                row.style(Style::default().bg(Color::DarkGray).fg(Color::White))
            } else {
                row
            }
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(15),
            Constraint::Length(15),
            Constraint::Length(15),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.tab_watch", lang)),
    );
    frame.render_widget(table, chunks[1]);
}

fn render_templates(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect) {
    let lang = ui.lang();
    if ui.config.templates.is_empty() {
        let lines = vec![
            Line::from(Span::styled(
                tr("tui.desc_templates", lang),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(Span::default()),
            Line::from(tr("tui.no_templates", lang)),
        ];
        let para = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr("tui.tab_templates", lang)),
        );
        frame.render_widget(para, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let desc = Paragraph::new(Span::styled(
        tr("tui.desc_templates", lang),
        Style::default().fg(Color::Cyan),
    ));
    frame.render_widget(desc, chunks[0]);

    let header = Row::new(vec![
        Cell::from(tr("tui.name_col", lang)),
        Cell::from(tr("tui.base_col", lang)),
        Cell::from(tr("tui.subfolder_col", lang)),
        Cell::from(tr("tui.tags_col", lang)),
    ])
    .style(Style::default().fg(Color::Yellow));

    let rows: Vec<Row> = ui
        .config
        .templates
        .iter()
        .map(|t| {
            Row::new(vec![
                Cell::from(t.name.clone()),
                Cell::from(t.base_template.as_deref().unwrap_or("(none)").to_string()),
                Cell::from(t.subfolder.as_deref().unwrap_or("(root)").to_string()),
                Cell::from(t.tags.join(", ")),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Min(30),
            Constraint::Length(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.tab_templates", lang)),
    );
    frame.render_widget(table, chunks[1]);
}

// --- Jobs tab rendering ---

fn render_jobs(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect) {
    let lang = ui.lang();

    if let Some(batch_id) = &ui.showing_items_for {
        render_job_items(frame, ui, area, batch_id);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let desc = Paragraph::new(Span::styled(
        tr("tui.desc_jobs", lang),
        Style::default().fg(Color::Cyan),
    ));
    frame.render_widget(desc, chunks[0]);

    if ui.jobs_cache.is_empty() {
        let lines = vec![Line::from(tr("tui.no_jobs", lang))];
        let para = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr("tui.tab_jobs", lang)),
        );
        frame.render_widget(para, chunks[1]);
        return;
    }

    let header = Row::new(vec![
        Cell::from(tr("tui.batch_col", lang)),
        Cell::from(tr("tui.source_col", lang)),
        Cell::from(tr("tui.target_col", lang)),
        Cell::from(tr("tui.status_col", lang)),
        Cell::from(tr("tui.items", lang).replace("Items: ", "")),
        Cell::from(tr("tui.time_col", lang)),
    ])
    .style(Style::default().fg(Color::Yellow));

    let rows: Vec<Row> = ui
        .jobs_cache
        .iter()
        .enumerate()
        .map(|(i, batch)| {
            let short_id = &batch.batch_id[..8.min(batch.batch_id.len())];
            let status_label = status_label(&batch.status, lang);
            let time = batch.created_at.format("%m-%d %H:%M").to_string();
            let row = Row::new(vec![
                Cell::from(short_id),
                Cell::from(batch.source.clone()),
                Cell::from(batch.target_id.clone()),
                Cell::from(status_label),
                Cell::from(batch.source_count.to_string()),
                Cell::from(time),
            ]);
            if i == ui.selected_index {
                row.style(Style::default().bg(Color::DarkGray).fg(Color::White))
            } else {
                row
            }
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.tab_jobs", lang)),
    );
    frame.render_widget(table, chunks[1]);
}

fn render_job_items(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect, batch_id: &str) {
    let lang = ui.lang();
    let short_id = &batch_id[..8.min(batch_id.len())];

    let header = Row::new(vec![
        Cell::from(tr("tui.item_name_col", lang)),
        Cell::from(tr("tui.item_status_col", lang)),
        Cell::from(tr("tui.item_path_col", lang)),
    ])
    .style(Style::default().fg(Color::Yellow));

    let rows: Vec<Row> = ui
        .items_cache
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let status_label = status_label(&item.status, lang);
            let row = Row::new(vec![
                Cell::from(item.source_name.clone()),
                Cell::from(status_label),
                Cell::from(
                    item.stored_path
                        .as_deref()
                        .unwrap_or(&item.source_path)
                        .to_string(),
                ),
            ]);
            if i == ui.selected_index {
                row.style(Style::default().bg(Color::DarkGray).fg(Color::White))
            } else {
                row
            }
        })
        .collect();

    let title = tr("tui.detail_title", lang).replace("{}", short_id);
    let table = Table::new(
        rows,
        [
            Constraint::Length(25),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(table, area);
}

fn status_label(status: &str, lang: &str) -> String {
    match status {
        "success" => tr("tui.status_success", lang).to_string(),
        "failed" => tr("tui.status_failed", lang).to_string(),
        "duplicate" => tr("tui.status_duplicate", lang).to_string(),
        "queued" => tr("tui.status_queued", lang).to_string(),
        "running" => tr("tui.status_running", lang).to_string(),
        "undone" | "undo_skipped_modified" | "partially_undone" => {
            tr("tui.status_undone", lang).to_string()
        }
        _ => status.to_string(),
    }
}

// --- Service tab rendering ---

fn render_service(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect) {
    let lang = ui.lang();

    let svc_status = if ui.service_status.is_empty() {
        "..."
    } else {
        &ui.service_status
    };
    let svc_label = match svc_status {
        "running" => tr("tui.status_success", lang),
        "stopped" => tr("tui.status_failed", lang),
        _ => tr(svc_status, lang),
    };
    let explorer_label = if ui.explorer_installed {
        tr("tui.installed", lang)
    } else {
        tr("tui.not_installed", lang)
    };
    let watch_label = if ui.config.agent.watch_in_service {
        tr("tui.on", lang)
    } else {
        tr("tui.off", lang)
    };

    let lines = vec![
        Line::from(Span::styled(
            tr("tui.desc_service", lang),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(Span::default()),
        // Service section
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.service_status_label", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(svc_label),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.watch_in_service_label", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(watch_label),
            Span::styled("  [w]", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled(
            format!(
                "  [i] {}  [s] {}  [S] {}  [u] {}",
                tr("tui.service_installed", lang),
                tr("tui.service_started", lang),
                tr("tui.service_stopped", lang),
                tr("tui.service_uninstalled", lang),
            ),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::default()),
        // Explorer section
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.explorer_status_label", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(explorer_label),
        ]),
        Line::from(Span::styled(
            format!(
                "  [m] {}  [M] {}",
                tr("tui.explorer_installed_msg", lang),
                tr("tui.explorer_uninstalled_msg", lang),
            ),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::default()),
        // Doctor section
        Line::from(Span::styled(
            format!(
                "[D] {}",
                tr("tui.desc_service", lang).split(',').next().unwrap_or("")
            ),
            Style::default().fg(Color::DarkGray),
        )),
    ];
    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.tab_service", lang)),
    );
    frame.render_widget(para, area);
}

// --- Action handlers ---

fn handle_add(ui: &mut SettingsUi) {
    match ui.active_tab {
        TabId::Targets => {
            ui.input_mode = InputMode::AddingTargetName;
            ui.input_buffer.clear();
            ui.pending_target_name = None;
            ui.message.clear();
        }
        TabId::Watch => {
            let old_buffer = ui.input_buffer.clone();
            ui.input_buffer.clear();
            ui.input_mode = InputMode::AddingWatchPath;
            try_folder_picker(ui);
            if ui.input_buffer.is_empty() {
                ui.input_buffer = old_buffer;
            } else {
                let path = PathBuf::from(&ui.input_buffer);
                ui.config.watch.push(WatchConfig {
                    path,
                    target: None,
                    debounce_secs: 2,
                    extensions: None,
                    template: None,
                });
                ui.selected_index = ui.config.watch.len() - 1;
                ui.pending_save = true;
                ui.input_mode = InputMode::Normal;
                ui.input_buffer.clear();
            }
        }
        _ => {}
    }
}

fn handle_remove(ui: &mut SettingsUi) {
    if ui.active_tab == TabId::Targets {
        let idx = ui.selected_index;
        if ui.config.targets.len() <= 1 {
            ui.message = tr("tui.cannot_remove_last", ui.lang()).to_string();
            return;
        }
        let removed = ui.config.targets.remove(idx);
        ui.message = format!("{} {}", tr("tui.removed", ui.lang()), removed.name);
        ui.pending_save = true;
        ui.clamp_selected();
    }
}

fn handle_default(ui: &mut SettingsUi) {
    if ui.active_tab == TabId::Targets && ui.config.targets.len() > 1 {
        let idx = ui.selected_index;
        if idx == 0 {
            return;
        }
        let target = ui.config.targets.remove(idx);
        ui.config.targets.insert(0, target);
        ui.selected_index = 0;
        ui.message = tr("tui.default_changed", ui.lang()).to_string();
        ui.pending_save = true;
    }
}

fn handle_toggle_frontmatter(ui: &mut SettingsUi) {
    if ui.active_tab == TabId::Import {
        ui.config.import.inject_frontmatter = !ui.config.import.inject_frontmatter;
        ui.pending_save = true;
    }
}

fn handle_toggle_language(ui: &mut SettingsUi) {
    if ui.active_tab == TabId::Import {
        let current = ui.config.import.language.as_deref().unwrap_or("en");
        ui.config.import.language = if current == "en" {
            Some("zh-CN".to_string())
        } else {
            Some("en".to_string())
        };
        ui.pending_save = true;
    }
}

fn handle_size_adjust(ui: &mut SettingsUi, code: KeyCode) {
    if ui.active_tab == TabId::Import {
        match code {
            KeyCode::Char('+') => ui.config.import.max_file_size_mb += 64,
            KeyCode::Char('-') if ui.config.import.max_file_size_mb > 64 => {
                ui.config.import.max_file_size_mb -= 64;
            }
            _ => {}
        }
        ui.pending_save = true;
    }
}

fn handle_edit(ui: &mut SettingsUi) {
    match ui.active_tab {
        TabId::Targets if !ui.config.targets.is_empty() => {
            let idx = ui.selected_index;
            if idx >= ui.config.targets.len() {
                return;
            }
            ui.input_mode = InputMode::EditingTargetVault(idx);
            ui.input_buffer.clear();
            ui.message.clear();
        }
        TabId::Watch if !ui.config.watch.is_empty() => {
            let idx = ui.selected_index;
            if idx >= ui.config.watch.len() {
                return;
            }
            ui.input_mode = InputMode::EditingWatchPath(idx);
            ui.input_buffer.clear();
            ui.message.clear();
        }
        _ => {}
    }
}

fn is_path_input_mode(mode: &InputMode) -> bool {
    matches!(
        mode,
        InputMode::AddingTargetPath | InputMode::AddingWatchPath | InputMode::EditingWatchPath(_)
    )
}

fn try_path_complete(ui: &mut SettingsUi) {
    if !is_path_input_mode(&ui.input_mode) {
        return;
    }
    let input = &ui.input_buffer;
    if input.is_empty() {
        return;
    }

    let path = PathBuf::from(input);
    let (parent, prefix) = if input.ends_with('\\') || input.ends_with('/') {
        (path.clone(), String::new())
    } else {
        let parent = path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let prefix = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        (parent, prefix)
    };

    let prefix_lower = prefix.to_lowercase();
    let Ok(entries) = std::fs::read_dir(&parent) else {
        return;
    };
    let mut matches: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_string()
                .to_lowercase()
                .starts_with(&prefix_lower)
        })
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let sep = if e.path().is_dir() { "\\" } else { "" };
            format!("{}{}", name, sep)
        })
        .collect();

    if matches.is_empty() {
        return;
    }
    matches.sort();

    if matches.len() == 1 {
        ui.input_buffer = parent.join(&matches[0]).display().to_string();
    } else {
        let mut common = matches[0].clone();
        for m in &matches[1..] {
            let common_len = common
                .chars()
                .zip(m.chars())
                .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
                .count();
            common.truncate(common_len);
        }
        if common.len() > prefix.len() {
            ui.input_buffer = parent.join(&common).display().to_string();
        }
        ui.message = format!("Matches: {}", matches.join(", "));
    }
}

// --- Jobs actions ---

fn refresh_jobs(ui: &mut SettingsUi) {
    ui.showing_items_for = None;
    ui.items_cache.clear();
    match ui.open_conn() {
        Ok(conn) => {
            let repo = crate::queue::repository::Repository::new(&conn);
            match repo.list_batches(50) {
                Ok(batches) => {
                    ui.jobs_cache = batches;
                    ui.clamp_selected();
                    ui.message.clear();
                }
                Err(e) => {
                    ui.message = format!("DB error: {e:#}");
                }
            }
        }
        Err(e) => {
            ui.message = format!("DB error: {e:#}");
        }
    }
}

fn show_job_detail(ui: &mut SettingsUi) {
    if ui.jobs_cache.is_empty() {
        return;
    }
    let idx = ui.selected_index;
    if idx >= ui.jobs_cache.len() {
        return;
    }
    let batch_id = ui.jobs_cache[idx].batch_id.clone();
    match ui.open_conn() {
        Ok(conn) => {
            let repo = crate::queue::repository::Repository::new(&conn);
            match repo.list_items_by_batch(&batch_id) {
                Ok(items) => {
                    ui.items_cache = items;
                    ui.showing_items_for = Some(batch_id);
                    ui.selected_index = 0;
                }
                Err(e) => {
                    ui.message = format!("DB error: {e:#}");
                }
            }
        }
        Err(e) => {
            ui.message = format!("DB error: {e:#}");
        }
    }
}

fn retry_selected_job(ui: &mut SettingsUi) {
    if ui.showing_items_for.is_some() || ui.jobs_cache.is_empty() {
        return;
    }
    let idx = ui.selected_index;
    if idx >= ui.jobs_cache.len() {
        return;
    }
    let batch_id = ui.jobs_cache[idx].batch_id.clone();
    let lang = ui.lang().to_string();
    match ui.open_conn() {
        Ok(conn) => {
            let repo = crate::queue::repository::Repository::new(&conn);
            match repo.retry_failed_items_by_batch(&batch_id) {
                Ok(count) => {
                    ui.message = format!("{} ({})", tr("tui.job_retried", &lang), count);
                    refresh_jobs(ui);
                }
                Err(e) => {
                    ui.message = tr("tui.job_retry_failed", &lang).replace("{}", &format!("{e:#}"));
                }
            }
        }
        Err(e) => {
            ui.message = format!("DB error: {e:#}");
        }
    }
}

fn undo_selected_job(ui: &mut SettingsUi) {
    if ui.showing_items_for.is_some() || ui.jobs_cache.is_empty() {
        return;
    }
    let idx = ui.selected_index;
    if idx >= ui.jobs_cache.len() {
        return;
    }
    let batch_id = ui.jobs_cache[idx].batch_id.clone();
    let lang = ui.lang().to_string();
    match crate::cli::handle_undo_batch_via_tui(&ui.app_data_dir, &batch_id) {
        Ok(msg) => {
            ui.message = msg;
            refresh_jobs(ui);
        }
        Err(e) => {
            ui.message = tr("tui.job_undo_failed", &lang).replace("{}", &format!("{e:#}"));
        }
    }
}

// --- Service actions ---

fn refresh_service_status(ui: &mut SettingsUi) {
    ui.service_status = crate::service::status().unwrap_or_else(|e| format!("error: {e}"));
    ui.explorer_installed = crate::explorer::is_installed().unwrap_or(false);
    ui.message.clear();
}

fn install_service(ui: &mut SettingsUi) {
    let lang = ui.lang().to_string();
    match crate::service::install(&ui.app_data_dir) {
        Ok(()) => {
            ui.message = tr("tui.service_installed", &lang).to_string();
        }
        Err(e) => {
            ui.message = tr("tui.service_op_failed", &lang).replace("{}", &format!("{e:#}"));
        }
    }
    refresh_service_status(ui);
}

fn start_service(ui: &mut SettingsUi) {
    let lang = ui.lang().to_string();
    match crate::service::start() {
        Ok(()) => {
            ui.message = tr("tui.service_started", &lang).to_string();
        }
        Err(e) => {
            ui.message = tr("tui.service_op_failed", &lang).replace("{}", &format!("{e:#}"));
        }
    }
    refresh_service_status(ui);
}

fn stop_service(ui: &mut SettingsUi) {
    let lang = ui.lang().to_string();
    match crate::service::stop() {
        Ok(()) => {
            ui.message = tr("tui.service_stopped", &lang).to_string();
        }
        Err(e) => {
            ui.message = tr("tui.service_op_failed", &lang).replace("{}", &format!("{e:#}"));
        }
    }
    refresh_service_status(ui);
}

fn uninstall_service(ui: &mut SettingsUi) {
    let lang = ui.lang().to_string();
    match crate::service::uninstall() {
        Ok(()) => {
            ui.message = tr("tui.service_uninstalled", &lang).to_string();
        }
        Err(e) => {
            ui.message = tr("tui.service_op_failed", &lang).replace("{}", &format!("{e:#}"));
        }
    }
    refresh_service_status(ui);
}

fn install_explorer(ui: &mut SettingsUi) {
    let lang = ui.lang().to_string();
    match crate::explorer::default_install_options(&lang) {
        Ok(options) => match crate::explorer::install(&options) {
            Ok(_menus) => {
                ui.message = tr("tui.explorer_installed_msg", &lang).to_string();
            }
            Err(e) => {
                ui.message = tr("tui.explorer_op_failed", &lang).replace("{}", &format!("{e:#}"));
            }
        },
        Err(e) => {
            ui.message = format!("ERROR: {e:#}");
        }
    }
    refresh_service_status(ui);
}

fn uninstall_explorer(ui: &mut SettingsUi) {
    let lang = ui.lang().to_string();
    match crate::explorer::uninstall() {
        Ok(()) => {
            ui.message = tr("tui.explorer_uninstalled_msg", &lang).to_string();
        }
        Err(e) => {
            ui.message = tr("tui.explorer_op_failed", &lang).replace("{}", &format!("{e:#}"));
        }
    }
    refresh_service_status(ui);
}

fn toggle_watch_in_service(ui: &mut SettingsUi) {
    ui.config.agent.watch_in_service = !ui.config.agent.watch_in_service;
    ui.pending_save = true;
}

fn run_doctor(ui: &mut SettingsUi) {
    let lang = ui.lang().to_string();
    match crate::app::App::bootstrap_in(ui.app_data_dir.clone()) {
        Ok(app) => {
            match crate::cli::run_doctor_checks(&app) {
                Ok(issues) => {
                    if issues.is_empty() {
                        ui.message = tr("tui.doctor_ok", &lang).to_string();
                    } else {
                        ui.message =
                            tr("tui.doctor_issues", &lang).replace("{}", &issues.len().to_string());
                        // Show issues in a multi-line message (use first one)
                        for issue in &issues {
                            ui.message.push_str(&format!("\n  - {issue}"));
                        }
                    }
                }
                Err(e) => {
                    ui.message = format!("Doctor error: {e:#}");
                }
            }
        }
        Err(e) => {
            ui.message = format!("Bootstrap error: {e:#}");
        }
    }
}

// --- Folder picker ---

#[cfg(windows)]
fn try_folder_picker(ui: &mut SettingsUi) {
    if !is_path_input_mode(&ui.input_mode) {
        return;
    }

    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileDialog, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);

    let result = unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let com_initialized = hr.is_ok();
        let result = (|| -> Option<String> {
            let dialog: IFileDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
            let mut options = dialog.GetOptions().ok()?;
            options |= FOS_PICKFOLDERS;
            dialog.SetOptions(options).ok()?;
            if dialog.Show(None).is_err() {
                return None;
            }
            let item = dialog.GetResult().ok()?;
            let display_name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            display_name.to_string().ok()
        })();
        if com_initialized {
            CoUninitialize();
        }
        result
    };

    let _ = execute!(io::stdout(), EnterAlternateScreen);
    let _ = enable_raw_mode();

    if let Some(path) = result {
        ui.input_buffer = path;
    }
}

#[cfg(not(windows))]
fn try_folder_picker(_ui: &mut SettingsUi) {}
