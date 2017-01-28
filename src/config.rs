use rusoto;
use clap::ArgMatches;
use std::path::PathBuf;
use std::str::FromStr;
use std::env;
use result::*;

#[derive(Debug, Clone)]
pub struct Config {
    pub config_file: Option<PathBuf>,
    pub credentials_file: Option<PathBuf>,
    pub profile: Option<String>,
    pub role: Option<String>,
    pub region: Option<rusoto::Region>,
    pub name: Option<String>,
}

impl Config {
    pub fn new_for_matches(args: &ArgMatches) -> Result<Config> {
        let region = if let Some(region_name) = args.value_of("region") {
            Some(try!(rusoto::Region::from_str(region_name)))
        } else {
            None
        };

        let default_config_location = Some(try!(Config::default_config_location()));

        Ok(Config {
            config_file: args.value_of("config").map(|s| PathBuf::from(s)).or_else(|| default_config_location),
            credentials_file: args.value_of("credentials").map(|s| PathBuf::from(s)),
            profile: args.value_of("profile").map(|s| s.to_owned()),
            role: args.value_of("role").map(|s| s.to_owned()),
            region: region,
            name: args.value_of("name").map(|s| s.to_owned()),
        })
    }

    fn default_config_location() -> Result<PathBuf> {
        match env::home_dir() {
            Some(home_path) => {
                let mut config_path = PathBuf::from(".aws");

                config_path.push("config");

                Ok(home_path.join(config_path))
            }
            None => Err(StsCliError::Error("The environment variable HOME must be set.".to_owned())),
        }
    }
}
