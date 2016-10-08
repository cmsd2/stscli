use rusoto;
use clap::ArgMatches;
use std::path::PathBuf;
use std::str::FromStr;
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

        Ok(Config {
            config_file: args.value_of("config_file").map(|s| PathBuf::from(s)),
            credentials_file: args.value_of("credentials_file").map(|s| PathBuf::from(s)),
            profile: args.value_of("profile").map(|s| s.to_owned()),
            role: args.value_of("role").map(|s| s.to_owned()),
            region: region,
            name: args.value_of("name").map(|s| s.to_owned()),
        })
    }
}
