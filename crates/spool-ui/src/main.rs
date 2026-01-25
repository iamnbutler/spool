mod app;
mod ui;

use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;

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
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                if app.search_mode {
                    match key.code {
                        KeyCode::Esc => app.toggle_search(),
                        KeyCode::Enter => app.toggle_search(),
                        KeyCode::Backspace => app.search_backspace(),
                        KeyCode::Char(c) => app.search_input(c),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => app.next_task(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous_task(),
                        KeyCode::Char('g') => app.first_task(),
                        KeyCode::Char('G') => app.last_task(),
                        KeyCode::Tab => app.toggle_focus(),
                        KeyCode::Enter => app.toggle_detail(),
                        KeyCode::Char('e') => app.toggle_events(),
                        KeyCode::Char('f') => app.cycle_status_filter(),
                        KeyCode::Char('s') => app.cycle_sort(),
                        KeyCode::Char('/') => app.toggle_search(),
                        KeyCode::Esc => app.clear_search(),
                        _ => {}
                    }
                }
            }
        }
    }
}
