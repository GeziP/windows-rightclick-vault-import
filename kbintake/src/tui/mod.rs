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
use crate::i18n::tr;

/// Active tab in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabId {
    Targets = 0,
    Import = 1,
    Watch = 2,
    Templates = 3,
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
}

impl SettingsUi {
    fn new(config: AppConfig) -> Self {
        Self {
            config,
            active_tab: TabId::Targets,
            selected_index: 0,
            message: String::new(),
            pending_save: false,
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            pending_target_name: None,
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
            TabId::Import => 0,
        }
    }

    fn clamp_selected(&mut self) {
        let count = self.item_count();
        if self.selected_index >= count && count > 0 {
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
}

/// Run the interactive settings TUI.
pub fn run_settings_tui(config: AppConfig) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut ui = SettingsUi::new(config);
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
                KeyCode::Up => ui.move_up(),
                KeyCode::Down => ui.move_down(),
                KeyCode::Char('s') => {
                    if let Err(e) = ui.save_config() {
                        ui.message = format!("ERROR: {e:#}");
                    } else {
                        ui.pending_save = false;
                    }
                }
                KeyCode::Char('a') => handle_add(ui),
                KeyCode::Char('r') => handle_remove(ui),
                KeyCode::Char('d') => handle_default(ui),
                KeyCode::Char('f') => handle_toggle_frontmatter(ui),
                KeyCode::Char('l') => handle_toggle_language(ui),
                KeyCode::Char('e') => handle_edit(ui),
                KeyCode::Char('t') => handle_edit_watch_field(ui, InputField::Target),
                KeyCode::Char('x') => handle_edit_watch_field(ui, InputField::Extensions),
                KeyCode::Char('b') => handle_edit_watch_field(ui, InputField::Debounce),
                KeyCode::Char('p') => handle_edit_watch_field(ui, InputField::Template),
                KeyCode::Char('+') | KeyCode::Char('-') => handle_size_adjust(ui, key.code),
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
    }

    // Contextual footer
    let help_key = match ui.active_tab {
        TabId::Targets => "tui.help_targets",
        TabId::Import => "tui.help_import",
        TabId::Watch => "tui.help_watch",
        TabId::Templates => "tui.help_templates",
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
            "[Enter] confirm  [Esc] cancel  [Backspace] delete",
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
    frame.render_widget(table, area);
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
        "简体中文"
    } else {
        "English"
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.max_file_size", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("{} MB", ui.config.import.max_file_size_mb)),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.frontmatter", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(fm_status),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{} ", tr("tui.language", lang)),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(lang_label),
        ]),
        Line::from(Span::default()),
        Line::from(vec![
            Span::styled("[+]", Style::default().fg(Color::Green)),
            Span::raw(format!(" {}", tr("tui.size_up", lang))),
        ]),
        Line::from(vec![
            Span::styled("[-]", Style::default().fg(Color::Red)),
            Span::raw(format!(" {}", tr("tui.size_down", lang))),
        ]),
        Line::from(vec![
            Span::styled("[f]", Style::default().fg(Color::Yellow)),
            Span::raw(format!(" {}", tr("tui.toggle_frontmatter", lang))),
        ]),
        Line::from(vec![
            Span::styled("[l]", Style::default().fg(Color::Yellow)),
            Span::raw(format!(" {}", tr("tui.toggle_language", lang))),
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
        let para = Paragraph::new(tr("tui.no_watch_configs", lang)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr("tui.tab_watch", lang)),
        );
        frame.render_widget(para, area);
        return;
    }

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
    frame.render_widget(table, area);
}

fn render_templates(frame: &mut ratatui::Frame, ui: &SettingsUi, area: Rect) {
    let lang = ui.lang();
    if ui.config.templates.is_empty() {
        let para = Paragraph::new(tr("tui.no_templates", lang)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr("tui.tab_templates", lang)),
        );
        frame.render_widget(para, area);
        return;
    }

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
    frame.render_widget(table, area);
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
            ui.input_mode = InputMode::AddingWatchPath;
            ui.input_buffer.clear();
            ui.message.clear();
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
