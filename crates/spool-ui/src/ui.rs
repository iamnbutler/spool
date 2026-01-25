use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus, InputMode, View};

/// Returns styled color for priority levels.
fn priority_style(priority: &str) -> Style {
    match priority {
        "p0" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        "p1" => Style::default().fg(Color::Yellow),
        "p2" => Style::default().fg(Color::Blue),
        _ => Style::default().fg(Color::DarkGray),
    }
}

/// Returns color for operation types.
fn operation_color(op: &str) -> Color {
    match op {
        "create" => Color::Green,
        "complete" => Color::Blue,
        "reopen" => Color::Yellow,
        "update" => Color::Yellow,
        "assign" => Color::Cyan,
        "comment" => Color::Magenta,
        "set_stream" => Color::Blue,
        "create_stream" => Color::Green,
        "update_stream" => Color::Yellow,
        "delete_stream" => Color::Red,
        _ => Color::DarkGray,
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    // Determine if we need a message/input bar
    let has_message = app.message.is_some();
    let in_input_mode = app.input_mode == InputMode::NewTask;
    let show_bar = has_message || in_input_mode;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if show_bar {
            vec![
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Message/Input bar
                Constraint::Length(1), // Footer
            ]
        } else {
            vec![
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Footer
            ]
        })
        .split(f.area());

    draw_header(f, chunks[0], app);
    draw_main(f, chunks[1], app);

    if show_bar {
        draw_input_bar(f, chunks[2], app);
        draw_footer(f, chunks[3], app);
    } else {
        draw_footer(f, chunks[2], app);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let title = match app.view {
        View::Tasks => {
            let search_indicator = if !app.search_query.is_empty() {
                format!("  \"{}\"", app.search_query)
            } else {
                String::new()
            };

            let stream_indicator = if app.stream_filter.is_some() {
                format!("  stream: {}", app.stream_filter_label())
            } else {
                String::new()
            };

            format!(
                " spool  {} tasks  [{}]  sort: {}{}{}",
                app.tasks.len(),
                app.status_filter.label(),
                app.sort_by.label(),
                stream_indicator,
                search_indicator,
            )
        }
        View::History => {
            format!(" spool  History  {} events", app.history_events.len())
        }
    };

    let header = Paragraph::new(title).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(header, area);
}

fn draw_main(f: &mut Frame, area: Rect, app: &mut App) {
    match app.view {
        View::Tasks => {
            if app.show_detail {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(area);

                draw_task_list(f, chunks[0], app);
                draw_task_detail(f, chunks[1], app);
            } else {
                draw_task_list(f, area, app);
            }
        }
        View::History => {
            draw_history(f, area, app);
        }
    }
}

fn draw_task_list(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let priority = task.priority.as_deref().unwrap_or("--");
            let pstyle = priority_style(priority);

            let status_marker = match task.status {
                spool::state::TaskStatus::Open => " ",
                spool::state::TaskStatus::Complete => "✓",
            };

            let assignee = task
                .assignee
                .as_deref()
                .map(|a| format!(" {}", a))
                .unwrap_or_default();

            let line = Line::from(vec![
                Span::styled(status_marker, Style::default().fg(Color::Green)),
                Span::styled(format!("{:4} ", priority), pstyle),
                Span::raw(&task.title),
                Span::styled(assignee, Style::default().fg(Color::DarkGray)),
            ]);

            let style = if i == app.selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let border_style = if app.focus == Focus::TaskList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = if app.search_mode {
        format!(" Tasks  /{}▌", app.search_query)
    } else {
        " Tasks ".to_string()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title),
    );

    f.render_widget(list, area);
}

fn draw_task_detail(f: &mut Frame, area: Rect, app: &mut App) {
    let border_style = if app.focus == Focus::Detail {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = if let Some(task) = app.selected_task() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::raw(&task.id),
            ]),
            Line::from(vec![
                Span::styled("Title: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&task.title, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:?}", task.status),
                    match task.status {
                        spool::state::TaskStatus::Open => Style::default().fg(Color::Yellow),
                        spool::state::TaskStatus::Complete => Style::default().fg(Color::Green),
                    },
                ),
            ]),
        ];

        if let Some(p) = &task.priority {
            lines.push(Line::from(vec![
                Span::styled("Priority: ", Style::default().fg(Color::DarkGray)),
                Span::styled(p, priority_style(p)),
            ]));
        }

        if let Some(assignee) = &task.assignee {
            lines.push(Line::from(vec![
                Span::styled("Assignee: ", Style::default().fg(Color::DarkGray)),
                Span::styled(assignee, Style::default().fg(Color::Cyan)),
            ]));
        }

        if !task.tags.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Tags: ", Style::default().fg(Color::DarkGray)),
                Span::styled(task.tags.join(", "), Style::default().fg(Color::Magenta)),
            ]));
        }

        if let Some(stream_id) = &task.stream {
            let stream_name = app
                .get_stream(stream_id)
                .map(|s| s.name.as_str())
                .unwrap_or(stream_id);
            lines.push(Line::from(vec![
                Span::styled("Stream: ", Style::default().fg(Color::DarkGray)),
                Span::styled(stream_name, Style::default().fg(Color::Blue)),
            ]));
        }

        // Created timestamp
        lines.push(Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
            Span::raw(task.created.format("%Y-%m-%d %H:%M").to_string()),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Updated: ", Style::default().fg(Color::DarkGray)),
            Span::raw(task.updated.format("%Y-%m-%d %H:%M").to_string()),
        ]));

        if let Some(desc) = &task.description {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Description:",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            )]));
            for line in desc.lines() {
                lines.push(Line::from(line));
            }
        }

        // Event history
        if !app.task_events.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Event History:",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            )]));

            for event in &app.task_events {
                let op_color = operation_color(&event.op.to_string());
                lines.push(Line::from(vec![
                    Span::styled(
                        event.ts.format("%m-%d %H:%M").to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(" "),
                    Span::styled(format!("{:12}", event.op), Style::default().fg(op_color)),
                    Span::styled(
                        format!(" by {}", event.by),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }

        lines
    } else {
        vec![Line::from("No task selected")]
    };

    let title = " Detail ";

    // Update scroll bounds (area height minus borders)
    let content_height = content.len() as u16;
    let visible_height = area.height.saturating_sub(2);

    let detail = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));

    f.render_widget(detail, area);

    // Set after content is consumed
    app.detail_content_height = content_height;
    app.detail_visible_height = visible_height;
}

fn draw_history(f: &mut Frame, area: Rect, app: &App) {
    if app.history_show_detail {
        // Split layout: list on left, detail on right
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        draw_history_list(f, chunks[0], app);
        draw_history_detail(f, chunks[1], app);
    } else {
        draw_history_list(f, area, app);
    }
}

// History column definitions: (label, width)
const HISTORY_COLS: &[(&str, usize)] = &[
    ("Event", 16),
    ("Name", 40),
    ("Date", 17),
    ("Assignee", 14),
    ("Branch", 20),
    ("ID", 24),
];

fn draw_history_list(f: &mut Frame, area: Rect, app: &App) {
    let scroll_x = app.history_scroll_x as usize;

    // Build header row
    let header_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::UNDERLINED);
    let header_cols: Vec<(String, Style)> = HISTORY_COLS
        .iter()
        .map(|(label, width)| (fixed_width(label, *width), header_style))
        .collect();
    let header_spans = build_scrolled_spans(&header_cols, scroll_x);
    let header_line = Line::from(header_spans);

    let items: Vec<ListItem> = app
        .history_events
        .iter()
        .enumerate()
        .map(|(i, event)| {
            let op_str = event.op.to_string();
            let op_color = operation_color(&op_str);

            // Get task/stream name
            let name = if let Some(title) = app.get_task_title(&event.id) {
                title.to_string()
            } else if let Some(stream) = app.get_stream(&event.id) {
                stream.name.clone()
            } else {
                "-".to_string()
            };

            // Build columns with exact fixed widths (matching HISTORY_COLS order)
            let columns: Vec<(String, Style)> = vec![
                (
                    fixed_width(&op_str, HISTORY_COLS[0].1),
                    Style::default().fg(op_color),
                ),
                (
                    fixed_width(&name, HISTORY_COLS[1].1),
                    Style::default().fg(Color::White),
                ),
                (
                    fixed_width(
                        &event.ts.format("%y.%m.%d %H:%M").to_string(),
                        HISTORY_COLS[2].1,
                    ),
                    Style::default().fg(Color::White),
                ),
                (
                    fixed_width(&event.by, HISTORY_COLS[3].1),
                    Style::default().fg(Color::Cyan),
                ),
                (
                    fixed_width(&event.branch, HISTORY_COLS[4].1),
                    Style::default().fg(Color::DarkGray),
                ),
                (
                    fixed_width(&event.id, HISTORY_COLS[5].1),
                    Style::default().fg(Color::DarkGray),
                ),
            ];

            let spans = build_scrolled_spans(&columns, scroll_x);

            let line = Line::from(spans);

            let style = if i == app.history_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(style)
        })
        .collect();

    let scroll_indicator = if scroll_x > 0 {
        format!(" << +{}", scroll_x)
    } else {
        String::new()
    };

    let border_style = if !app.history_show_detail {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(
        " History ({} events){} ",
        app.history_events.len(),
        scroll_indicator
    );

    // Split area for header + list
    let inner = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);
    let inner_area = inner.inner(area);
    f.render_widget(inner, area);

    if inner_area.height < 2 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner_area);

    // Render header
    let header = Paragraph::new(header_line);
    f.render_widget(header, chunks[0]);

    // Render list
    let list = List::new(items);
    f.render_widget(list, chunks[1]);
}

// Lines are built conditionally based on event type (task vs stream) and optional fields,
// so initializing with vec![] and pushing is clearer than a complex vec![...] literal.
#[allow(clippy::vec_init_then_push)]
fn draw_history_detail(f: &mut Frame, area: Rect, app: &App) {
    let content = if let Some(event) = app.selected_history_event() {
        let mut lines = vec![];

        // Event info
        lines.push(Line::from(vec![
            Span::styled("Operation: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                event.op.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Timestamp: ", Style::default().fg(Color::DarkGray)),
            Span::raw(event.ts.format("%Y-%m-%d %H:%M:%S").to_string()),
        ]));

        lines.push(Line::from(vec![
            Span::styled("By: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&event.by, Style::default().fg(Color::Yellow)),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Branch: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&event.branch),
        ]));

        lines.push(Line::from(""));

        // Task/Stream info if available
        if let Some(task) = app.get_task(&event.id) {
            lines.push(Line::from(vec![Span::styled(
                "Task Details:",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            )]));

            lines.push(Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::raw(&task.id),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Title: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&task.title, Style::default().add_modifier(Modifier::BOLD)),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:?}", task.status),
                    match task.status {
                        spool::state::TaskStatus::Open => Style::default().fg(Color::Yellow),
                        spool::state::TaskStatus::Complete => Style::default().fg(Color::Green),
                    },
                ),
            ]));

            if let Some(p) = &task.priority {
                lines.push(Line::from(vec![
                    Span::styled("Priority: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(p, priority_style(p)),
                ]));
            }

            if let Some(assignee) = &task.assignee {
                lines.push(Line::from(vec![
                    Span::styled("Assignee: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(assignee, Style::default().fg(Color::Cyan)),
                ]));
            }

            if !task.tags.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Tags: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(task.tags.join(", "), Style::default().fg(Color::Magenta)),
                ]));
            }

            if let Some(stream_id) = &task.stream {
                let stream_name = app
                    .get_stream(stream_id)
                    .map(|s| s.name.as_str())
                    .unwrap_or(stream_id);
                lines.push(Line::from(vec![
                    Span::styled("Stream: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(stream_name, Style::default().fg(Color::Blue)),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
                Span::raw(task.created.format("%Y-%m-%d %H:%M").to_string()),
            ]));

            if let Some(desc) = &task.description {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![Span::styled(
                    "Description:",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::UNDERLINED),
                )]));
                for line in desc.lines() {
                    lines.push(Line::from(line));
                }
            }
        } else if let Some(stream) = app.get_stream(&event.id) {
            lines.push(Line::from(vec![Span::styled(
                "Stream Details:",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            )]));

            lines.push(Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::raw(&event.id),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&stream.name, Style::default().add_modifier(Modifier::BOLD)),
            ]));

            if let Some(desc) = &stream.description {
                lines.push(Line::from(""));
                lines.push(Line::from(vec![Span::styled(
                    "Description:",
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::UNDERLINED),
                )]));
                for line in desc.lines() {
                    lines.push(Line::from(line));
                }
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::raw(&event.id),
            ]));
        }

        // Show event data if present
        if !event.d.is_null() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Event Data:",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::UNDERLINED),
            )]));
            if let Ok(pretty) = serde_json::to_string_pretty(&event.d) {
                for line in pretty.lines() {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
        }

        lines
    } else {
        vec![Line::from("No event selected")]
    };

    let detail = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Detail "),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.history_detail_scroll, 0));

    f.render_widget(detail, area);
}

/// Pads or truncates string to exact width. Truncated strings end with `~`.
fn fixed_width(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count > width {
        // Truncate and add ~ indicator
        let truncated: String = s.chars().take(width.saturating_sub(1)).collect();
        format!("{}~", truncated)
    } else {
        // Pad with dots to exact width
        let padding = width - char_count;
        if padding > 0 {
            format!("{}{}", s, ".".repeat(padding))
        } else {
            s.to_string()
        }
    }
}

/// Build spans with horizontal scroll support, preserving colors.
fn build_scrolled_spans(columns: &[(String, Style)], scroll_x: usize) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars_skipped = 0;

    for (text, style) in columns {
        let col_len = text.chars().count();

        if chars_skipped + col_len <= scroll_x {
            // This entire column is scrolled off
            chars_skipped += col_len;
            continue;
        }

        if chars_skipped < scroll_x {
            // This column is partially scrolled
            let skip_in_col = scroll_x - chars_skipped;
            let visible: String = text.chars().skip(skip_in_col).collect();
            spans.push(Span::styled(visible, *style));
            chars_skipped = scroll_x;
        } else {
            // This column is fully visible
            spans.push(Span::styled(text.clone(), *style));
        }

        chars_skipped += col_len;
    }

    spans
}

fn draw_input_bar(f: &mut Frame, area: Rect, app: &App) {
    let content = if app.input_mode == InputMode::NewTask {
        Line::from(vec![
            Span::styled(" New task: ", Style::default().fg(Color::Cyan)),
            Span::raw(&app.input_buffer),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ])
    } else if let Some(msg) = &app.message {
        Line::from(vec![Span::styled(
            format!(" {}", msg),
            Style::default().fg(Color::Yellow),
        )])
    } else {
        Line::from("")
    };

    let bar = Paragraph::new(content);
    f.render_widget(bar, area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let help = match app.input_mode {
        InputMode::NewTask => " Enter:create  Esc:cancel ",
        InputMode::Normal if app.search_mode => " Type to search, Enter/Esc to close ",
        InputMode::Normal => match app.view {
            View::Tasks => {
                " q:quit  j/k:nav  c:complete  r:reopen  n:new  v:view  s:sort  /:search  h:history "
            }
            View::History => {
                if app.history_show_detail {
                    " q:quit  j/k:scroll  Tab/Shift-Tab:nav  Esc:close  h:back "
                } else {
                    " q:quit  j/k:nav  l/left:scroll  Enter:detail  h:back "
                }
            }
        },
    };
    let footer = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, area);
}
