mod app;
mod ui;

use std::io;

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, InputMode, View};

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let app = App::new()?;
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {err:?}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                // Ctrl+C quits from any mode
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    return Ok(());
                }

                // Clear message on any keypress
                app.clear_message();

                match app.input_mode {
                    InputMode::NewTask => match key.code {
                        KeyCode::Esc => app.cancel_input(),
                        KeyCode::Enter => app.submit_new_task(),
                        KeyCode::Backspace => app.input_backspace(),
                        KeyCode::Char(c) => app.input_char(c),
                        _ => {}
                    },
                    InputMode::Normal if app.search_mode => match key.code {
                        KeyCode::Esc => app.toggle_search(),
                        KeyCode::Enter => app.toggle_search(),
                        KeyCode::Backspace => app.search_backspace(),
                        KeyCode::Char(c) => app.search_input(c),
                        _ => {}
                    },
                    InputMode::Normal if app.view == View::History => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('h') => app.toggle_history_view(),
                        KeyCode::Esc => {
                            if app.history_show_detail {
                                app.close_history_detail();
                            } else {
                                return Ok(());
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if app.history_show_detail {
                                app.history_detail_scroll_down();
                            } else {
                                app.history_next();
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if app.history_show_detail {
                                app.history_detail_scroll_up();
                            } else {
                                app.history_previous();
                            }
                        }
                        KeyCode::Char('g') => app.history_first(),
                        KeyCode::Char('G') => app.history_last(),
                        KeyCode::Char('l') | KeyCode::Right => app.history_scroll_right(),
                        KeyCode::Left => app.history_scroll_left(),
                        KeyCode::Enter => app.toggle_history_detail(),
                        KeyCode::Tab => {
                            // Allow navigating list even when detail is open
                            if app.history_show_detail {
                                app.history_next();
                            }
                        }
                        KeyCode::BackTab => {
                            if app.history_show_detail {
                                app.history_previous();
                            }
                        }
                        _ => {}
                    },
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => {
                            if app.focus == app::Focus::Detail {
                                app.scroll_detail_down();
                            } else {
                                app.next_task();
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if app.focus == app::Focus::Detail {
                                app.scroll_detail_up();
                            } else {
                                app.previous_task();
                            }
                        }
                        KeyCode::Char('g') => app.first_task(),
                        KeyCode::Char('G') => app.last_task(),
                        KeyCode::Tab => app.toggle_focus(),
                        KeyCode::Enter => app.toggle_detail(),
                        KeyCode::Char('v') => app.cycle_status_filter(),
                        KeyCode::Char('s') => app.cycle_sort(),
                        KeyCode::Char('S') => app.cycle_stream_filter(),
                        KeyCode::Char('/') => app.toggle_search(),
                        KeyCode::Esc => {
                            if app.search_query.is_empty() {
                                return Ok(());
                            } else {
                                app.clear_search();
                            }
                        }
                        // Task editing
                        KeyCode::Char('c') => app.complete_selected_task(),
                        KeyCode::Char('r') => app.reopen_selected_task(),
                        KeyCode::Char('n') => app.start_new_task(),
                        KeyCode::Char('h') => app.toggle_history_view(),
                        _ => {}
                    },
                }
            }
        }
    }
}
