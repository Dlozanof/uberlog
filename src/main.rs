use std::{fs::{OpenOptions}, path::PathBuf, time::Duration};

use tracing::{error, info, span, Level};
use tracing_subscriber::{fmt, prelude::*, Registry};
use uberlog_lib::{command_parser::CommandParser, commander::{add_filter, find_log, stream_start, stream_stop, Command, CommandResponse, Commander}, configuration::{self, ApplicationConfiguration}, layout_section::LayoutSection, tui::{section_filters::SectionFilters, section_logs::SectionLogs, section_probe::SectionProbes}, LogFilter, LogMessage};

use tokio::runtime::Runtime;
use std::sync::mpsc::{Receiver, Sender};

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout}, prelude::{Backend, CrosstermBackend}, text::Line, widgets::{Block, Paragraph}, Frame, Terminal
};
use std::{error::Error, io};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};

pub struct App {
    
    // Configuration
    config: ApplicationConfiguration,

    current_screen: CurrentScreen,
    
    pub command_tx: Sender<Command>,
    pub command_response_rx: Receiver<CommandResponse>,
    pub rtt_data_rx: Receiver<LogMessage>,

    // Top section
    pub section_probes: SectionProbes,
    pub section_filters: SectionFilters,

    // Log section
    pub section_logs: SectionLogs,

    // Status line
    pub command_parser: CommandParser,

    pub message: String,
}

#[derive(Debug, Default)]
pub enum CurrentScreen {
    // Live log viewer
    #[default]
    Live,
    Filters,
}

fn open_file_command(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.is_empty() {
        return Err(String::from("path no"));
    }

    if input.len() > 1 {
        return Err(String::from("Too many arguments"));
    }

    let _ = sender.send(Command::OpenFile(PathBuf::from(&input[0])));

    Ok(())
}


fn main() -> Result<(), Box<dyn Error>> {

    let log_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("uberlog.log")
        .unwrap();

    if std::env::var("RUST_LOG").is_err() {
        unsafe {
            std::env::set_var("RUST_LOG", "info")
        }
    }

    let subscriber = Registry::default()
        .with(
            fmt::layer()
                .with_writer(log_file)
        )
        .with(tracing_subscriber::filter::EnvFilter::from_default_env());
    tracing::subscriber::set_global_default(subscriber).unwrap();

    info!("Starting app");
    // Load configuration file
    let target_cfg = configuration::load_target_cfg();
    let app_cfg = ApplicationConfiguration::load_cfg();

    // Prepare communication layer for gui-commander and commander-commander trheads
    let (commander_tx, commander_rx) = std::sync::mpsc::channel();
    let (commander_responwe_tx, commander_response_rx) = std::sync::mpsc::channel();
    let (rtt_data_tx, rtt_data_rx) = std::sync::mpsc::channel();

    // Instantiate application and commander
    let mut app = App::new(commander_tx.clone(), commander_response_rx, rtt_data_rx, &app_cfg);
    let mut commander = Commander::new(commander_tx.clone(), commander_rx, commander_responwe_tx, rtt_data_tx, target_cfg, &app_cfg);

    // Register commands -- File
    app.command_parser.register_instruction(String::from(":open"),open_file_command);
    app.command_parser.register_instruction(String::from(":sstart"), stream_start);
    app.command_parser.register_instruction(String::from(":sstop"), stream_stop);
    app.command_parser.register_instruction(String::from(":find"), find_log);
    // Register commands -- Filter
    app.command_parser.register_instruction(String::from(":filter"),add_filter);
    
    // Commander main loop
    let rt = Runtime::new().expect("Unable to create Runtime");
    let _enter = rt.enter();
    std::thread::spawn(move || {
        rt.block_on(async {
            // Hack-ish: send a probe update command
            let _ = commander_tx.send(Command::GetProbes);
            loop {
                let _span = span!(Level::DEBUG, "Commander cmd process").entered();
                match commander.process() {
                    Ok(_) => (),
                    Err(e) => {
                        error!("{}", e);
                        let _ = commander.command_response_tx.send(CommandResponse::TextMessage{message: "Internal error".to_string()});
                        break;
                    }
                }
            }
        });
    });

    // setup terminal
    enable_raw_mode()?;
    let mut stderr = io::stderr(); // This is a special case. Normally using stdout is fine
    execute!(stderr, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    // run it
    let res = run_app(&mut terminal, &mut app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}


fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<bool> {
    loop {
        // TODO: Try to fix this
        //app.section_logs.vertical_scroll_state = app.section_logs.vertical_scroll_state.content_length(app.section_logs.logs.len());
        
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {

                // Skip events that are not KeyEventKind::Press
                if key.kind == event::KeyEventKind::Release {
                    continue;
                }

                // If command parser is processing a command, append char and skip further processing
                if !app.command_parser.is_idle() {
                    app.command_parser.process_key(key.code);
                }
                else {
                    match app.current_screen {
                        CurrentScreen::Live => {
                            match key.code {
                                // Only exit the application from `Live` screen
                                KeyCode::Char('q') => {
                                    return Ok(true)
                                },

                                // So far only process comands in `Live` screen
                                KeyCode::Char(':') | KeyCode::Char('/') => {
                                    app.message.clear();
                                    app.command_parser.process_key(key.code);
                                },

                                // Switch to Filter view
                                KeyCode::Char('F') => {
                                    let _ = app.command_tx.send(Command::GetFilters);
                                    app.current_screen = CurrentScreen::Filters;
                                },

                                // Otherwise forward to sub-views
                                key => {
                                    app.section_probes.process_key(key);
                                    app.section_logs.process_key(key);
                                }
                            }
                        }
                        CurrentScreen::Filters => {
                            match key.code {
                                // Only exit the application from `Live` screen
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    app.current_screen = CurrentScreen::Live;
                                },
                                // Otherwise forward to sub-views
                                key => {
                                    app.section_filters.process_key(key);
                                }
                            }
                        }
                    } 
                }
            }
        }
        
        // Check for command responses
        if let Ok(response) = app.command_response_rx.try_recv() {
            match response {
                CommandResponse::ProbeInformation { probes } => {
                    app.section_probes.targets = probes;
                },
                CommandResponse::TextMessage { message } => {
                    app.command_parser.cancel_parsing();
                    app.message = message;
                },
                CommandResponse::UpdateFilterList(filters) => {
                    app.section_filters.set_filters(filters);
                }
                CommandResponse::UpdateLogs(logs) => {
                    app.section_logs.update_logs(logs);
                }
                CommandResponse::UpdateSearchLog(log) => {
                    app.section_logs.update_search_log(log);
                }
            }
        }

        // Read data
        while let Ok(message) = app.rtt_data_rx.try_recv() {
            app.section_logs.logs.push(message);
        }
    }
}

pub fn ui(frame: &mut Frame, app: &mut App) {

    let top_side_lines = match app.current_screen {
        CurrentScreen::Live => app.section_probes.min_lines(),
        CurrentScreen::Filters => app.section_filters.min_lines(),
    };
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_side_lines as u16), // Probes
            Constraint::Min(1),    // Logs
            Constraint::Length(1), // Modal editor
        ])
        .split(frame.area());

    // Print either Probes or Filters depending on the current screen
    match app.current_screen {
        CurrentScreen::Live => app.section_probes.ui(frame, chunks[0]),
        CurrentScreen::Filters => app.section_filters.ui(frame, chunks[0]),
    }

    // Show Logs section
    app.section_logs.ui(frame, chunks[1]);

    // And Status line
    let text_to_print = match app.command_parser.is_idle() {
        true => app.message.clone(),
        false => app.command_parser.get_parsed_cmd(),
    };
    let status_line = Paragraph::new(Line::from(text_to_print))
        .block(Block::default());
    frame.render_widget(status_line, chunks[2]);
}

impl App {

    pub fn new(command_tx: Sender<Command>, command_response_rx: Receiver<CommandResponse>, rtt_data_rx: Receiver<LogMessage>, cfg: &ApplicationConfiguration) -> App {

        let aliases = cfg.alias_list.clone();
        App {
            config: cfg.clone(),
            command_tx: command_tx.clone(),
            command_response_rx,
            rtt_data_rx,
            current_screen: CurrentScreen::Live,
            section_logs: SectionLogs::new(command_tx.clone()),
            section_probes: SectionProbes {
                command_tx: command_tx.clone(),
                selected_probe: 0,
                targets: Vec::new(),
            },
            section_filters: SectionFilters::new(command_tx.clone()),
            command_parser: CommandParser::new(command_tx, aliases),
            message: String::new(),
        }
    }

}
