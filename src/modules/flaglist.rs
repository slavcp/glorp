use std::{collections::HashSet, fs::*, io::*};

use crate::constants;

#[derive(serde::Deserialize, serde::Serialize)]
struct FlaglistConfig {
    enabled: HashSet<String>,
    disabled: HashSet<String>,
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
        flaglist.enabled.insert(url);
    }

    for url in default_urls.disabled {
        flaglist.disabled.insert(url);
    }

    flaglist
        .enabled
        .retain(|url| !flaglist.disabled.contains(url));

    let updated_flaglist_string = serde_json::to_string_pretty(&flaglist).unwrap();
    flaglist_file.set_len(0).unwrap();
    flaglist_file.seek(std::io::SeekFrom::Start(0)).unwrap();
    flaglist_file
        .write_all(updated_flaglist_string.as_bytes())
        .unwrap();

    let mut args_str = String::new();
    for flag in &flaglist.enabled {
        args_str = args_str + flag + " ";
    }
    args_str
}
