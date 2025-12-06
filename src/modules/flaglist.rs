use std::{collections::HashSet, fs::*, io::*};

use crate::constants;

#[derive(serde::Deserialize, serde::Serialize)]
struct UserBlocklist {
    flags: HashSet<String>,
    disabled_defaults: HashSet<String>,
}

pub fn load() -> String {
    let example_flags: &str = r#"
{
    "flags": [
        "--disable-gpu-vsync"
    ],
    "disabled_defaults": [
        ""
    ]
}"#;

    let defaults: Vec<String> = serde_json::from_str(constants::DEFAULT_FLAGS).unwrap();
    let flaglist_path: String = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\user_flags.json";
    let mut flaglist_file = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .truncate(false)
        .open(&flaglist_path)
        .unwrap();

    if flaglist_file.metadata().unwrap().len() == 0 {
        flaglist_file.write_all(example_flags.as_bytes()).ok();
    }

    let flaglist_string = std::fs::read_to_string(&flaglist_path).unwrap();

    let flaglist = match serde_json::from_str::<UserBlocklist>(&flaglist_string) {
        Ok(config) => config,
        Err(_) => {
            flaglist_file.set_len(0).ok();
            flaglist_file.write_all(example_flags.as_bytes()).ok();
            serde_json::from_str::<UserBlocklist>(example_flags).unwrap()
        }
    };

    let final_flags = defaults
        .into_iter()
        .filter(|url| !flaglist.disabled_defaults.contains(url))
        .chain(flaglist.flags);

    let mut args_str = String::new();
    for flag in final_flags {
        args_str = args_str + &flag + " ";
    }
    args_str
}
