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
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Terminal,
};

use crate::config::{AppConfig, WatchConfig};
use crate::i18n::tr;

/// Active tab in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TabId {
    Targets = 0,
    Import = 1,
    Watch = 2,
    Templates = 3,
}

const TAB_TITLES: &[&str] = &["Targets", "Import", "Watch", "Templates"];

/// Text input mode for the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    AddingTargetName,
    AddingTargetPath,
    AddingWatchPath,
}

/// Top-level TUI state.
struct SettingsUi {
    config: AppConfig,
    active_tab: TabId,
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
        println!("{}", tr("tui.exiting", ui.config.language()));
    } else {
        eprintln!("{}", tr("tui.exiting", ui.config.language()));
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
            if ui.input_mode != InputMode::Normal
                && handle_text_input(ui, key.code)
            {
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('1') => ui.active_tab = TabId::Targets,
                KeyCode::Char('2') => ui.active_tab = TabId::Import,
                KeyCode::Char('3') => ui.active_tab = TabId::Watch,
                KeyCode::Char('4') => ui.active_tab = TabId::Templates,
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
                KeyCode::Char('+') | KeyCode::Char('-') => handle_size_adjust(ui, key.code),
                _ => {}
            }
        }
    }
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
            match ui.input_mode {
                InputMode::AddingTargetName => {
                    let input_trimmed = input.trim().to_string();
                    if input_trimmed.is_empty() {
                        ui.message = "Name cannot be empty".to_string();
                        ui.input_mode = InputMode::Normal;
                        return true;
                    }
                    // Validate characters.
                    if !input_trimmed
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    {
                        ui.message = "Name: only letters, numbers, '-', '_' allowed".to_string();
                        ui.input_mode = InputMode::Normal;
                        return true;
                    }
                    // Store name and move to path input.
                    ui.pending_target_name = Some(input_trimmed);
                    ui.input_buffer.clear();
                    ui.input_mode = InputMode::AddingTargetPath;
                    ui.message.clear();
                }
                InputMode::AddingTargetPath => {
                    let path = PathBuf::from(&input);
                    let name = ui.pending_target_name.take().unwrap_or_default();
                    match crate::config::validate_target_root(&path) {
                        Ok(()) => {
                            match ui.config.add_target(name, path) {
                                Ok(t) => {
                                    let lang = ui.config.language().to_string();
                                    ui.message = format!(
                                        "{} {}",
                                        tr("cli.added_target", &lang),
                                        t.name
                                    );
                                    ui.pending_save = true;
                                }
                                Err(e) => ui.message = format!("ERROR: {e:#}"),
                            }
                        }
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
                    ui.message = "Added watch path (press 's' to save)".to_string();
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
        _ => false, // Let other keys pass through.
    }
}

fn render(frame: &mut ratatui::Frame, ui: &SettingsUi) {
    let lang = ui.config.language();
    let size = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(size);

    // Tabs
    let titles: Vec<Span> = TAB_TITLES
        .iter()
        .map(|t| Span::raw(*t))
        .collect();
    let active_index = ui.active_tab as usize;
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("Tabs [1-4]"))
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

    // Footer
    let footer_text = if ui.message.is_empty() {
        tr("tui.footer", lang)
    } else {
        format!("\u{26a0} {}", ui.message)
    };
    let footer = Paragraph::new(Span::raw(&footer_text)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help "),
    );
    frame.render_widget(footer, chunks[2]);
}

fn render_input_overlay(frame: &mut ratatui::Frame, ui: &SettingsUi) {
    let area = frame.area();
    let overlay_area = ratatui::layout::Rect {
        x: area.x + 2,
        y: area.y + 2,
        width: area.width.saturating_sub(4).max(40),
        height: 7,
    };

    let (prompt, step_hint) = match ui.input_mode {
        InputMode::AddingTargetName => ("Target name: ", "alphanumeric, '-', '_'"),
        InputMode::AddingTargetPath => ("Vault path: ", ""),
        InputMode::AddingWatchPath => ("Watch directory: ", ""),
        InputMode::Normal => ("", ""),
    };

    let title = match ui.input_mode {
        InputMode::AddingTargetName => " Add Target (1/2) ",
        InputMode::AddingTargetPath => " Add Target (2/2) ",
        InputMode::AddingWatchPath => " Add Watch Path ",
        InputMode::Normal => " Input ",
    };

    let mut lines = vec![
        Line::from(Span::raw(format!("{prompt}{step_hint}"))),
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

fn render_targets(frame: &mut ratatui::Frame, ui: &SettingsUi, area: ratatui::layout::Rect) {
    let lang = ui.config.language();
    let header = Row::new(vec![
        Cell::from(tr("tui.default_col", lang)),
        Cell::from(tr("tui.target_col", lang)),
        Cell::from(tr("tui.name_col", lang)),
        Cell::from(tr("tui.status_col", lang)),
        Cell::from(tr("tui.path_col", lang)),
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
            Row::new(vec![
                Cell::from(default_marker),
                Cell::from(target.target_id.clone()),
                Cell::from(target.name.clone()),
                Cell::from(target.status.clone()),
                Cell::from(target.root_path.display().to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(38),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.targets_title", lang)),
    );
    frame.render_widget(table, area);
}

fn render_import(frame: &mut ratatui::Frame, ui: &SettingsUi, area: ratatui::layout::Rect) {
    let lang = ui.config.language();
    let lines = vec![
        Line::from(vec![
            Span::styled(tr("tui.max_file_size", lang), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {} MB", ui.config.import.max_file_size_mb)),
        ]),
        Line::from(vec![
            Span::styled(tr("tui.frontmatter", lang), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", if ui.config.import.inject_frontmatter { "ON" } else { "OFF" })),
        ]),
        Line::from(vec![
            Span::styled(tr("tui.language", lang), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {}", ui.config.import.language.as_deref().unwrap_or("en"))),
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
            .title(tr("tui.import_title", lang)),
    );
    frame.render_widget(para, area);
}

fn render_watch(frame: &mut ratatui::Frame, ui: &SettingsUi, area: ratatui::layout::Rect) {
    let lang = ui.config.language();
    if ui.config.watch.is_empty() {
        let para = Paragraph::new(tr("tui.no_watch_configs", lang)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr("tui.watch_title", lang)),
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
        .map(|w| {
            Row::new(vec![
                Cell::from(w.path.display().to_string()),
                Cell::from(w.target.as_deref().unwrap_or("(default)")),
                Cell::from(w.template.as_deref().unwrap_or("(none)")),
                Cell::from(
                    w.extensions
                        .as_ref()
                        .map(|e| format!("{e:?}"))
                        .unwrap_or_else(|| tr("tui.all_extensions", lang)),
                ),
                Cell::from(format!("{}s", w.debounce_secs)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(30),
            Constraint::Length(15),
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(tr("tui.watch_title", lang)),
    );
    frame.render_widget(table, area);
}

fn render_templates(frame: &mut ratatui::Frame, ui: &SettingsUi, area: ratatui::layout::Rect) {
    let lang = ui.config.language();
    if ui.config.templates.is_empty() {
        let para = Paragraph::new(tr("tui.no_templates", lang)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(tr("tui.templates_title", lang)),
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
            .title(tr("tui.templates_title", lang)),
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
        if ui.config.targets.len() > 1 {
            let removed = ui.config.targets.pop().unwrap();
            let name = removed.name;
            let lang = ui.config.language().to_string();
            ui.message = format!(
                "{} {}",
                tr("tui.removed", &lang),
                name
            );
            ui.pending_save = true;
        } else {
            ui.message = tr("tui.cannot_remove_last", ui.config.language());
        }
    }
}

fn handle_default(ui: &mut SettingsUi) {
    if ui.active_tab == TabId::Targets && ui.config.targets.len() > 1 {
        let second = ui.config.targets.remove(1);
        ui.config.targets.insert(0, second);
        ui.message = tr("tui.default_changed", ui.config.language());
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
    let current = ui.config.import.language.as_deref().unwrap_or("en");
    ui.config.import.language = if current == "en" {
        Some("zh-CN".to_string())
    } else {
        Some("en".to_string())
    };
    ui.pending_save = true;
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
