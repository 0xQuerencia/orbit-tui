mod app;
mod metrics;
mod ui;

use std::{
    io::{self, stdout},
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let _guard = TerminalGuard;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;
        if app.quit {
            break;
        }
        if event::poll(Duration::from_millis(150))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }
        if last_tick.elapsed() >= Duration::from_secs(1) {
            app.tick();
            last_tick = Instant::now();
        }
    }
    Ok(())
}
