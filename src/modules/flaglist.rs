use std::fs::*;
use std::io::*;

use crate::constants;

#[derive(serde::Deserialize, serde::Serialize)]
struct FlaglistConfig {
    enabled: Vec<String>,
    disabled: Vec<String>,
}

pub fn load() -> String {
    let flaglist_path: String =
        std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\flags.json";
    let mut flaglist_file = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(&flaglist_path)
        .unwrap();

    if flaglist_file.metadata().unwrap().len() == 0 {
        flaglist_file
            .write_all(constants::DEFAULT_FLAGS.as_bytes())
            .unwrap();
    }

    let flaglist_string = std::fs::read_to_string(&flaglist_path).unwrap();

    let mut flaglist = match serde_json::from_str::<FlaglistConfig>(&flaglist_string) {
        Ok(config) => config,
        Err(_) => {
            flaglist_file
                .write_all(constants::DEFAULT_FLAGS.as_bytes())
                .unwrap();
            serde_json::from_str::<FlaglistConfig>(constants::DEFAULT_FLAGS).unwrap()
        }
    };

    let default_urls = serde_json::from_str::<FlaglistConfig>(constants::DEFAULT_FLAGS).unwrap();

    for url in default_urls.enabled {
        if !flaglist.enabled.contains(&url) && !flaglist.disabled.contains(&url) {
            flaglist.enabled.push(url);
        }
    }

    let mut args_str = String::new();
    for flag in &flaglist.enabled {
        args_str = args_str + flag + " ";
    }
    args_str
}
