mod app;
mod cert;
mod cli;
mod config;
mod docker;
mod files;
mod ftp;
mod logs;
mod service;
mod sites;
mod system;
mod theme;
mod ui;

use std::io::{self, Stdout};
use std::panic;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::App;

fn main() -> Result<()> {
    let cli = cli::parse(std::env::args()).map_err(anyhow::Error::msg)?;
    if cli.show_help {
        print!("{}", cli::help_text());
        return Ok(());
    }
    if cli.show_version {
        println!("nserver {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut app = App::new(cli);
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        restore_terminal();
        original_hook(info);
    }));

    let mut terminal = setup_terminal().context("无法初始化终端")?;
    let result = run(&mut terminal, &mut app);
    restore_terminal();
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(err) = execute!(stdout, EnterAlternateScreen, EnableBracketedPaste) {
        let _ = disable_raw_mode();
        return Err(err.into());
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(stdout, DisableBracketedPaste, LeaveAlternateScreen);
    let _ = execute!(stdout, crossterm::cursor::Show);
}

fn run(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if app.cursor_visible() {
            terminal.show_cursor()?;
        } else {
            terminal.hide_cursor()?;
        }
        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) => app.on_key(key),
                Event::Paste(text) => app.on_paste(&text),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        app.tick();
        if app.should_quit {
            break;
        }
    }
    Ok(())
}
