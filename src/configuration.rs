use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LogBackend {
   Uart{dev: String, baud: u32},
   Rtt{elf_path: String}
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
   pub name: String,
   pub processor: String,
   pub log_backend: LogBackend,
   pub probe_id: String
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum PowerSupply {
   Dp100{voltage: f32, current: f32},
   None(),
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Configuration {
   pub power: PowerSupply,
   pub targets: Vec<Target>
}

pub fn load_configuration() -> Configuration {
   let cfg_string = fs::read_to_string(".gadget.yaml").expect("Unable to read configuration file");
   let cfg: Configuration = serde_yaml::from_str(&cfg_string).expect("Bad");
   println!("{:?}", cfg);

   cfg
}