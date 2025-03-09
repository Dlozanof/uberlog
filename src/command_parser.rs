use std::sync::mpsc::Sender;

use crossterm::event::KeyCode;

use crate::{commander::Command, configuration::Alias};

pub enum State {
    Idle,
    Parsing,
}

pub struct Instruction {
    opcode: String,
    operation: fn(&Sender<Command>, Vec<String>) -> Result<(), String> // This is bullshit, better to have a command_tx and interact with Commander
}

pub struct CommandParser {
    parsed_command: String,
    state: State,
    registered_instructions: Vec<Instruction>,
    command_tx: Sender<Command>,
    aliases: Vec<Alias>,
}

impl CommandParser {

    /// Create a new commander
    pub fn new(command_tx: Sender<Command>, aliases: Vec<Alias>) -> CommandParser {
        CommandParser {
            parsed_command: String::new(),
            state: State::Idle,
            registered_instructions: Vec::new(),
            command_tx,
            aliases,
        }
    }

    /// Check if it is currently idle or processing a command
    pub fn is_idle(&self) -> bool {
        match self.state {
            State::Idle => true,
            State::Parsing => false,
        }
    }

    /// Getter for the currently passed string
    pub fn get_parsed_cmd(&self) -> String {
        self.parsed_command.clone()
    }

    /// Register an instruction to the command parser
    pub fn register_instruction(&mut self, opcode: String, operation: fn(&Sender<Command>, Vec<String>) -> Result<(), String>) {
        self.registered_instructions.push(Instruction {
            opcode,
            operation
        });
    }

    // Print message
    pub fn print_message(&mut self, msg: String) {
        self.parsed_command = msg;
    }

    /// Command complete, process it
    fn execute_order_66(&mut self) {

        // Handle special case of `/` for search
        self.parsed_command = self.parsed_command.replacen("/", ":find ", 1);

        // Split with spaces
        let mut tokenized_instruction: Vec<String> = self.parsed_command.split_ascii_whitespace().map(|x| {
            String::from(x.trim())
        }).collect();

        if tokenized_instruction.len() == 0 {
            return;
        }

        // Identify and apply aliases
        for alias in &self.aliases {
            if alias.alias.eq(&tokenized_instruction[0]){
                tokenized_instruction.remove(0);

                let mut alias_token: Vec<String> = alias.expanded.split_ascii_whitespace().map(|x| String::from(x.trim())).collect();
                alias_token.append(&mut tokenized_instruction);
                tokenized_instruction = alias_token;
            }
        }

        // Separate command from arguments
        let (inst, args) = tokenized_instruction.split_at(1);
        let inst: String = inst.last().expect("Really bad").to_owned();

        //let _ = self.command_tx.send(Command::PrintMessage(format!("inst: {:?}, args {:?}", inst, args)));
        
        // Execute command
        for registered_inst in &self.registered_instructions {
            if registered_inst.opcode.eq(&inst) {
                match (registered_inst.operation)(&self.command_tx, args.to_vec()) {
                    Ok(()) => (),
                    Err(e) => {
                        let _ = self.command_tx.send(Command::PrintMessage(e));
                    }
                }
                break;
            }
        }
        self.state = State::Idle;
    }

    /// Utility function, just cancel parsing
    pub fn cancel_parsing(&mut self) {
        self.parsed_command.clear();
        self.state = State::Idle;
    }

    /// Process keypresses received
    pub fn process_key(&mut self, key: KeyCode) {
        match key {
            // First time means start parsing command, subsequent times
            // mean it is just another character
            KeyCode::Char(':') | KeyCode::Char('/') => {
                if let KeyCode::Char(c) = key {
                    match self.state {
                        State::Idle => {
                            self.state = State::Parsing;
                            self.parsed_command.clear();
                            self.parsed_command.push(c);
                        },
                        State::Parsing => self.parsed_command.push(c),
                    }
                };
            },
            // Append to the command that is being parsed
            KeyCode::Char(c) => self.parsed_command.push(c),
            
            // Search for the matching instruction
            KeyCode::Enter => {
                self.execute_order_66();
            },
            
            // Cancel
            KeyCode::Esc => self.cancel_parsing(),
            
            // Remove last character
            KeyCode::Backspace => {
                let _ = self.parsed_command.pop();
            },
            _ => ()
        }
    }
}