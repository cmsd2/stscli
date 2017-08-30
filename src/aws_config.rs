use result::*;
use ini::Ini;
use ini::ini;
use std::path::Path;
use std::fs::File;
use std::collections::HashMap;
use std::result;
use std::str::FromStr;
use rusoto_core::Region;

pub trait LoadFromPath where Self: Sized {
    type Error: Sized + 'static;

    fn load_from_path(filename: &Path) -> result::Result<Self, Self::Error>;
}

impl LoadFromPath for Ini {
    type Error = ini::Error;

    fn load_from_path(filename: &Path) -> result::Result<Ini, Self::Error> {
        let mut reader = match File::open(filename) {
            Err(e) => {
                return Err(ini::Error {
                    line: 0,
                    col: 0,
                    msg: format!("Unable to open `{:?}`: {}", filename, e),
                })
            }
            Ok(r) => r,
        };
        Ini::read_from(&mut reader)
    }
}

#[derive(Debug, Clone)]
pub struct ConfigProfile {
    pub name: String,
    pub role_arn: Option<String>,
    pub source_profile: Option<String>,
    pub region: Option<Region>,
}

impl ConfigProfile {
    pub fn new<S>(name: S) -> ConfigProfile where S: Into<String> {
        ConfigProfile {
            name: name.into(),
            role_arn: None,
            source_profile: None,
            region: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub default_region: Option<Region>,
    pub profiles: HashMap<String, ConfigProfile>,
}

impl Config {
    pub fn load_from_path(path: &Path) -> Result<Config> {
        parse_config_file(path)
    }
}

fn get_profile_name_from_section_name(section_name: &str) -> Option<String> {
    let prefix = "profile ";
    if section_name.starts_with(prefix) {
        Some(section_name.chars().skip(prefix.len()).collect())
    } else if section_name != "default" {
        Some(section_name.to_owned())
    } else {
        None
    }
}


fn parse_config_file(file_path: &Path) -> Result<Config> {
    let ini = try!(Ini::load_from_path(file_path).into());

    let default_section = ini.section(Some("default".to_owned()));
    let maybe_default_region_name = default_section.and_then(|s| s.get("region"));
    let default_region = if let Some(default_region_name) = maybe_default_region_name {
        Some(try!(Region::from_str(default_region_name)))
    } else {
        None
    };

    let mut profiles = HashMap::new();

    for key in ini.sections() {
        if let Some(section_name) = key.as_ref() {  
            let section = ini.section(key.to_owned()).unwrap();

            if let Some(profile_name) = get_profile_name_from_section_name(section_name) {
                let maybe_region_name = section.get("region");
                let region = if let Some(region_name) = maybe_region_name {
                    Some(try!(Region::from_str(region_name)))
                } else {
                    None
                };
                let source_profile = section.get("source_profile").map(|s| s.to_owned());
                let role_arn = section.get("role_arn").map(|s| s.to_owned());

                profiles.insert(profile_name.clone(), ConfigProfile {
                    name: profile_name,
                    role_arn: role_arn,
                    source_profile: source_profile,
                    region: region,
                });
            }
        }
    }

    Ok(Config {
        default_region: default_region,
        profiles: profiles,
    })
}
