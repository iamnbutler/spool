use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus, InputMode};

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

    let title = format!(
        " spool  {} tasks  [{}]  sort: {}{}{}",
        app.tasks.len(),
        app.status_filter.label(),
        app.sort_by.label(),
        stream_indicator,
        search_indicator,
    );
    let header = Paragraph::new(title).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(header, area);
}

fn draw_main(f: &mut Frame, area: Rect, app: &mut App) {
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

fn draw_task_list(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let priority = task.priority.as_deref().unwrap_or("--");
            let priority_style = match priority {
                "p0" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                "p1" => Style::default().fg(Color::Yellow),
                "p2" => Style::default().fg(Color::Blue),
                _ => Style::default().fg(Color::DarkGray),
            };

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
                Span::styled(format!("{:4} ", priority), priority_style),
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

        if let Some(priority) = &task.priority {
            let priority_style = match priority.as_str() {
                "p0" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                "p1" => Style::default().fg(Color::Yellow),
                "p2" => Style::default().fg(Color::Blue),
                _ => Style::default().fg(Color::DarkGray),
            };
            lines.push(Line::from(vec![
                Span::styled("Priority: ", Style::default().fg(Color::DarkGray)),
                Span::styled(priority, priority_style),
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
                let op_style = match event.op.to_string().as_str() {
                    "create" => Style::default().fg(Color::Green),
                    "complete" => Style::default().fg(Color::Blue),
                    "update" => Style::default().fg(Color::Yellow),
                    "assign" => Style::default().fg(Color::Cyan),
                    _ => Style::default().fg(Color::DarkGray),
                };

                lines.push(Line::from(vec![
                    Span::styled(
                        event.ts.format("%m-%d %H:%M").to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(" "),
                    Span::styled(format!("{:12}", event.op), op_style),
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
        InputMode::Normal => {
            " q:quit  j/k:nav  c:complete  r:reopen  n:new  v:view  s:sort  /:search "
        }
    };
    let footer = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, area);
}
