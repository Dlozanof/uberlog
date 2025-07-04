use std::{fs::OpenOptions, time::Duration};

use tracing::{Level, error, info, span};
use tracing_subscriber::{Registry, fmt, prelude::*};
use uberlog_lib::{
    command_parser::CommandParser, commander::{self, add_filter, Command, Commander, UiCommand}, configuration::{self, ApplicationConfiguration}, tui::{
        section_filters::SectionFilters, section_logs::SectionLogs, section_sources::SectionSources, LayoutSection,
    }, LogMessage
};

use std::sync::mpsc::{Receiver, Sender};
use tokio::runtime::Runtime;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::crossterm::event::DisableMouseCapture;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use ratatui::{
    Frame, Terminal,
    layout::{Constraint, Direction, Layout},
    prelude::{Backend, CrosstermBackend},
    text::Line,
    widgets::{Block, Paragraph},
};
use std::{error::Error, io};

pub struct App {
    current_screen: CurrentScreen,

    pub command_tx: Sender<Command>,
    pub command_response_rx: Receiver<UiCommand>,
    pub rtt_data_rx: Receiver<LogMessage>,

    // Top section
    pub section_probes: SectionSources,
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
    Probes,
}

fn main() -> Result<(), Box<dyn Error>> {
    let log_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("uberlog.log")
        .unwrap();

    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") }
    }

    let subscriber = Registry::default()
        .with(fmt::layer().with_writer(log_file).with_ansi(false))
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
    let mut app = App::new(
        commander_tx.clone(),
        commander_response_rx,
        rtt_data_rx,
        &app_cfg,
    );
    let mut commander = Commander::new(
        commander_tx.clone(),
        commander_rx,
        commander_responwe_tx,
        rtt_data_tx,
        target_cfg,
        &app_cfg,
    );

    // Register commands -- File
    app.command_parser
        .register_instruction(String::from(":stream_in"), commander::stream_file);
    app.command_parser
        .register_instruction(String::from(":stream_out"), commander::stream_start);
    app.command_parser
        .register_instruction(String::from(":stream_out_stop"), commander::stream_stop);
    // Register commands -- Internal
    app.command_parser
        .register_instruction(String::from(":find"), commander::find_log);
    // Register commands -- Filter
    app.command_parser
        .register_instruction(String::from(":filter"), commander::add_filter);

    // Commander main loop
    let rt = Runtime::new().expect("Unable to create Runtime");
    let _enter = rt.enter();
    std::thread::spawn(move || {
        rt.block_on(async {
            loop {
                let _span = span!(Level::DEBUG, "Commander cmd process").entered();
                match commander.process() {
                    Ok(_) => (),
                    Err(e) => {
                        error!("Commander error: {}", e);
                        let _ = commander.command_response_tx.send(UiCommand::TextMessage {
                            message: "Internal error".to_string(),
                        });
                        break;
                    }
                }
            }
        });
    });

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // create the backend/terminal
    let backend = CrosstermBackend::new(stdout);
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
                } else {
                    match app.current_screen {
                        CurrentScreen::Live => {
                            match key.code {
                                // Only exit the application from `Live` screen
                                KeyCode::Char('q') => return Ok(true),

                                // Switch to Filter view
                                KeyCode::Char('F') => {
                                    let _ = app.command_tx.send(Command::GetFilters);
                                    app.current_screen = CurrentScreen::Filters;
                                }

                                // Switch to Probe view
                                KeyCode::Char('P') => {
                                    let _ = app.command_tx.send(Command::RefreshProbeInfo);
                                    app.current_screen = CurrentScreen::Probes;
                                }

                                // So far only process comands in `Live` screen
                                KeyCode::Char(':') | KeyCode::Char('/') => {
                                    app.message.clear();
                                    app.command_parser.process_key(key.code);
                                }

                                // Otherwise forward to sub-views
                                key => {
                                    app.section_logs.process_key(key);
                                }
                            }
                        }
                        CurrentScreen::Filters => {
                            match key.code {
                                // Only exit the application from `Live` screen
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    app.current_screen = CurrentScreen::Live;
                                }

                                // Switch to Probe view
                                KeyCode::Char('P') => {
                                    let _ = app.command_tx.send(Command::RefreshProbeInfo);
                                    app.current_screen = CurrentScreen::Probes;
                                }

                                // Otherwise forward to sub-views
                                key => {
                                    app.section_filters.process_key(key);
                                }
                            }
                        }
                        CurrentScreen::Probes => {
                            match key.code {
                                // Only exit the application from `Live` screen
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    app.current_screen = CurrentScreen::Live;
                                }

                                // Switch to Filter view
                                KeyCode::Char('F') => {
                                    let _ = app.command_tx.send(Command::GetFilters);
                                    app.current_screen = CurrentScreen::Filters;
                                }

                                // Otherwise forward to sub-views
                                key => {
                                    app.section_probes.process_key(key);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check for command responses
        if let Ok(response) = app.command_response_rx.try_recv() {
            info!("Ui Processing {}", response);
            match response {
                UiCommand::TextMessage { message } => {
                    app.command_parser.cancel_parsing();
                    app.message = message;
                }
                UiCommand::UpdateFilterList(filters) => {
                    app.section_filters.set_filters(filters);
                }
                UiCommand::UpdateLogs(logs) => {
                    app.section_logs.update_logs(logs);
                }
                UiCommand::UpdateSearchLog(log) => {
                    app.section_logs.update_search_log(log);
                }
                UiCommand::AddNewSource(id, display_text) => {
                    app.section_probes.add_source(id, display_text);
                }
                UiCommand::SetConnectionSource(id, is_connected) => {
                    app.section_probes.set_connected(id, is_connected);
                }
                UiCommand::RemoveSource(id) => {
                    app.section_probes.delete_source(id);
                }
            }
        }

        // Read data
        while let Ok(message) = app.rtt_data_rx.try_recv() {
            app.section_logs.append_log(message);
        }
    }
}

pub fn ui(frame: &mut Frame, app: &mut App) {
    // Depending on the current view allocate some lines on top
    let top_side_lines = match app.current_screen {
        CurrentScreen::Live => 0,
        CurrentScreen::Filters => app.section_filters.min_lines(),
        CurrentScreen::Probes => app.section_probes.min_lines(),
    };

    // Prepare chunks
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_side_lines as u16), // Probes
            Constraint::Min(1),                        // Logs
            Constraint::Length(1),                     // Modal editor
        ])
        .split(frame.area());

    // If a view other than Live is selected, show it
    match app.current_screen {
        CurrentScreen::Probes => app.section_probes.ui(frame, chunks[0]),
        CurrentScreen::Filters => app.section_filters.ui(frame, chunks[0]),
        CurrentScreen::Live => (),
    }

    // Show Logs section
    app.section_logs.ui(frame, chunks[1]);

    // And Status line
    let text_to_print = match app.command_parser.is_idle() {
        true => app.message.clone(),
        false => app.command_parser.get_parsed_cmd(),
    };
    let status_line = Paragraph::new(Line::from(text_to_print)).block(Block::default());
    frame.render_widget(status_line, chunks[2]);
}

impl App {
    pub fn new(
        command_tx: Sender<Command>,
        command_response_rx: Receiver<UiCommand>,
        rtt_data_rx: Receiver<LogMessage>,
        cfg: &ApplicationConfiguration,
    ) -> App {
        let aliases = cfg.alias_list.clone();
        App {
            command_tx: command_tx.clone(),
            command_response_rx,
            rtt_data_rx,
            current_screen: CurrentScreen::Live,
            section_logs: SectionLogs::new(command_tx.clone()),
            section_probes: SectionSources::new(command_tx.clone()),
            section_filters: SectionFilters::new(command_tx.clone()),
            command_parser: CommandParser::new(command_tx, aliases),
            message: String::new(),
        }
    }
}
