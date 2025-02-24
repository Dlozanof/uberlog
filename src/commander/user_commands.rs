use std::sync::mpsc::Sender;

use super::Command;

/// Start streaming into a file
pub fn stream_start(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.len() != 1 {
        return Err(String::from("Wrong arguments, expected just the path"));
    }
    let _ = sender.send(Command::StreamLogs(true, input[0].clone()));
    Ok(())
}

/// Stop streaming into a file
pub fn stream_stop(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.len() != 0 {
        return Err(String::from("Too many arguments"));
    }
    let _ = sender.send(Command::StreamLogs(false, String::new()));
    Ok(())
}

/// Stop streaming into a file
pub fn find_log(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.len() != 1 {
        return Err(String::from("Nothing to search for"));
    }
    let _ = sender.send(Command::FindLog(input[0].clone()));
    Ok(())
}

/// Stream an input file
pub fn stream_file(sender: &Sender<Command>, input: Vec<String>) -> Result<(), String> {
    if input.is_empty() {
        return Err(String::from("path no"));
    }

    if input.len() > 1 {
        return Err(String::from("Too many arguments"));
    }

    let _ = sender.send(Command::StreamFile(input[0].clone()));

    Ok(())
}
