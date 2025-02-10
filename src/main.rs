use std::{fs::{self, OpenOptions}, time::Duration};

use probe_rs::rtt::Rtt;
use serde::{Deserialize, Serialize};
use tracing::error;
use tracing_subscriber::{fmt, prelude::*, Registry};
use rtt_viewer_lib::{commander::{Command, Commander}, configuration, gui::MyApp};

use tokio::runtime::Runtime;


fn main() -> Result<(), eframe::Error> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        unsafe { std::env::set_var("RUST_LIB_BACKTRACE", "1") }
    }
    color_eyre::install().expect("Bad");

    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "info") }
    }

    // Log to a file
    let log_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("uberlog.log")
        .unwrap();
    let subscriber = Registry::default()
        .with(
            fmt::layer()
                .with_writer(log_file)
        )
        .with(tracing_subscriber::filter::EnvFilter::from_default_env());

    tracing::subscriber::set_global_default(subscriber).unwrap();


    /*
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();
    */

    let options = eframe::NativeOptions {
        //initial_window_size: Some(egui::vec2(600.0, 800.0)),
        ..Default::default()
    };

    // Load configuration file
    let cfg = configuration::load_configuration();

    // Prepare communication layer for gui-commander and commander-commander trheads
    let (commander_tx, commander_rx) = std::sync::mpsc::channel();
    let (commander_responwe_tx, commander_response_rx) = std::sync::mpsc::channel();
    let (rtt_data_tx, rtt_data_rx) = std::sync::mpsc::channel();

    let app = MyApp::new(commander_tx.clone(), commander_response_rx, rtt_data_rx);
    let mut commander = Commander::new(commander_tx.clone(), commander_rx, commander_responwe_tx, rtt_data_tx, cfg);

    // Commander main loop
    let rt = Runtime::new().expect("Unable to create Runtime");
    let _enter = rt.enter();
    std::thread::spawn(move || {
        rt.block_on(async {
            // Hack-ish: send a probe update command
            let _ = commander_tx.send(Command::GetProbes);
            loop {
                match commander.process() {
                    Ok(_) => (),
                    Err(e) => error!("{}", e),
                }
            }
        });
    });

    eframe::run_native(
        "Acua",
        options,
        Box::new(|_cc| {
            Ok(Box::new(app))
        }),
    )
}

