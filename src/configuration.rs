use std::{
    fs::{self, File},
    io::Write,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LogBackend {
    Uart { dev: String, baud: u32 },
    Rtt { elf_path: String },
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub name: String,
    pub processor: String,
    pub log_backend: LogBackend,
    pub probe_id: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum PowerSupply {
    Dp100 { voltage: f32, current: f32 },
    None(),
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetConfiguration {
    //pub power: PowerSupply,
    pub targets: Vec<Target>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Alias {
    pub alias: String,
    pub expanded: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ApplicationConfiguration {
    /// TODO: Default color for non filtered data

    /// Alias list
    pub alias_list: Vec<Alias>,
}

impl ApplicationConfiguration {
    fn generate_default() -> ApplicationConfiguration {
        ApplicationConfiguration {
            alias_list: vec![
                Alias {
                    alias: String::from(":fy"),
                    expanded: String::from(":filter h yellow"),
                },
                Alias {
                    alias: String::from(":fr"),
                    expanded: String::from(":filter h red"),
                },
                Alias {
                    alias: String::from(":fe"),
                    expanded: String::from(":filter e"),
                },
                Alias {
                    alias: String::from(":fi"),
                    expanded: String::from(":filter i"),
                },
                Alias {
                    alias: String::from(":fh"),
                    expanded: String::from(":filter h"),
                },
            ],
        }
    }

    pub fn load_cfg() -> ApplicationConfiguration {
        // Please be aware that the warning about home_dir is benign, the project doc says in a future release will be removed
        let mut p = std::env::home_dir().expect("Unable to get HOME");
        p.push(".config/uberlog/config.yaml");

        // If config does not exist, create it
        if !p.exists() {
            // Generate output
            let default_settings = ApplicationConfiguration::generate_default();
            let yaml_contents = serde_yaml::to_string(&default_settings)
                .expect("Unable to generate default settings");

            // Create folder and write into config file
            let _ = fs::create_dir(p.parent().expect("Path broken"))
                .expect("Unable to create directory");
            let mut file = File::create(p.clone()).expect("Unable to open new config file");
            let _ = file.write_all(yaml_contents.as_bytes());
        }

        let cfg_string = fs::read_to_string(p).expect("Unable to read configuration file");
        let cfg: ApplicationConfiguration = serde_yaml::from_str(&cfg_string).expect("Bad");
        cfg
    }
}

pub fn load_target_cfg() -> TargetConfiguration {
    let cfg_string = fs::read_to_string(".gadget.yaml").expect("Unable to read configuration file");
    let cfg: TargetConfiguration = serde_yaml::from_str(&cfg_string).expect("Bad");
    cfg
}
