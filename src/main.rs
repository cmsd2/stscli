extern crate clap;
extern crate rusoto;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate regex;
extern crate ini;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

pub mod print;
pub mod result;
pub mod config;
pub mod aws_config;

use clap::{Arg, ArgMatches, App, SubCommand};
use std::io::Write;
use std::collections::HashMap;
use std::ffi::OsString;
use std::process;
use std::path;
use std::env;
use rusoto::*;
use rusoto::sts::*;
use print::*;
use result::*;
use config::*;

pub fn main() {
    env_logger::init().unwrap();
    
    let mut stderr = std::io::stderr();

    let matches = App::new("stscli")
        .version("1.0")
        .author("Chris Dawes <cmsd2@cantab.net>")
        .about("Acquire session tokens from Amazon STS")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Sets a custom aws config file")
            .takes_value(true)
            )
        .arg(Arg::with_name("credentials")
            .short("d")
            .long("credentials")
            .value_name("FILE")
            .help("Sets a custom aws credentials file")
            .takes_value(true)
            )
        .arg(Arg::with_name("profile")
            .short("p")
            .long("profile")
            .help("Select which profile to use from the config or credentials file")
            .takes_value(true)
            )
        .arg(Arg::with_name("role")
            .short("r")
            .long("role")
            .help("Set the arn of the role to assume")
            .takes_value(true)
            )
        .arg(Arg::with_name("region")
            .short("R")
            .long("region")
            .help("Set the name of the region to use, e.g. eu-west-1")
            .takes_value(true)
            )
        .arg(Arg::with_name("name")
            .short("n")
            .long("name")
            .help("The name of the session to use if assuming a role. It will appear in CloudTrail logs. [\\w+=,.@-]*")
            .takes_value(true)
            )
        .arg(Arg::with_name("serial_number")
            .short("s")
            .long("serial-number")
            .help("The serial number or ARN of the MFA device.")
            .takes_value(true)
            )
        .arg(Arg::with_name("token_code")
            .short("t")
            .long("token-code")
            .help("The code from the MFA device.")
            .takes_value(true)
            )
        .subcommand(SubCommand::with_name("get")
            .about("get some fresh session tokens and display them")
            .version("1.0")
            .author("various")
            .arg(Arg::with_name("export")
                .long("export")
                .short("e")
                .required(false)
                .takes_value(false)
                .help("print the variables for exporting to a shell")
                )
            .arg(Arg::with_name("format")
                .long("format")
                .short("f")
                .required(false)
                .takes_value(true)
                .help("format to use when printing the variables. one of json, bash, fish or powershell. default bash")
                )
            )
        .subcommand(SubCommand::with_name("exec")
            .about("runs a command with session tokens injected into the environment")
            .version("1.0")
            .author("various")
            .arg(Arg::with_name("command")
                .long("command")
                .required(true)
                .takes_value(true)
                .index(1)
                .multiple(true)
                .help("shell command to run")
                )
            )
        .subcommand(SubCommand::with_name("list")
            .about("lists the available profiles")
            .version("1.0")
            .author("various")
            )
        .get_matches();
    
    match run_subcommand(&matches) {
        Err(err) => {
            writeln!(&mut stderr, "Error: {}", err).unwrap();
            process::exit(1);
        },
        _ => {}
    }
}

fn run_subcommand(matches: &ArgMatches) -> Result<()> {
    let config = try!(Config::new_for_matches(matches));
    debug!("config: {:?}", config);

    match matches.subcommand() {
        ("get", Some(sub_matches)) => get_token(sub_matches, &config),
        ("exec", Some(sub_matches)) => exec_command(sub_matches, &config),
        ("list", Some(sub_matches)) => list_profiles(sub_matches, &config),
        _ => Ok(())
    }
}

fn get_credentials(config: &Config) -> Result<rusoto::AwsCredentials> {
    let mut profile_provider = try!(ProfileProvider::new());

    if let Some(ref credentials_file_name) = config.credentials_file {
        profile_provider.set_file_path(credentials_file_name);
    }

    if let Some(ref profile) = config.profile {
        profile_provider.set_profile(&profile[..]);
    }

    if let Some(ref config_file_name) = config.config_file {
        let aws_config = try!(aws_config::Config::load_from_path(config_file_name));

        if let Some(ref profile) = config.profile {
            if let Some(ref profile_config) = aws_config.profiles.get(profile) {
                let region = config.region
                    .or_else(|| profile_config.region)
                    .or_else(|| aws_config.default_region)
                    .unwrap_or(Region::UsEast1);

                let mut profile_provider = profile_provider.clone();

                profile_provider.set_profile(profile_config.source_profile.clone().unwrap_or("default".to_owned()));

                let base_provider = ChainProvider::with_profile_provider(profile_provider);

                let sts_client = StsClient::new(try!(default_tls_client()), base_provider, region);
                
                if let Some(ref role_arn) = profile_config.role_arn {
                    let response = try!(sts_client.assume_role(&AssumeRoleRequest{
                        role_arn: role_arn.to_owned(),
                        role_session_name: config.name.clone().unwrap_or("stscli".to_owned()),
                        serial_number: config.serial_number.clone(),
                        token_code: config.token_code.clone(),
                        ..Default::default()
                    }));

                    let sts_creds = try!(response.credentials.ok_or(StsCliError::Error("STS AssumeRole did not return any credentials".to_owned())));

                    return Ok(try!(AwsCredentials::new_for_credentials(sts_creds)));
                }

                let response = try!(sts_client.get_session_token(&GetSessionTokenRequest {
                    ..Default::default()
                }));

                let sts_creds = try!(response.credentials.ok_or(StsCliError::Error("STS GetSessionTokenRequest did not return any credentials".to_owned())));

                return Ok(try!(AwsCredentials::new_for_credentials(sts_creds)));
            }
        }
    }

    let provider = ChainProvider::with_profile_provider(profile_provider);

    provider.credentials().map_err(StsCliError::from)
}

fn get_output_format(args: &ArgMatches) -> OutputFormat {
    let export = args.is_present("export");

    match args.value_of("format") {
        Some("json") => OutputFormat::Json,
        Some("fish") => OutputFormat::Fish { export: export },
        Some("powershell") => OutputFormat::Powershell { export: export },
        _ => OutputFormat::Bash { export: export }
    }
}

fn get_token(args: &ArgMatches, config: &Config) -> Result<()> {
    let creds = try!(get_credentials(config));
    let output_format = get_output_format(args);

    let vars = try!(get_vars(args, config, &creds));

    match output_format {
        OutputFormat::Json => { try!(print_vars_json(args, config, &vars)); },
        format => { try!(print_vars(args, config, &vars, format)); }
    };

    Ok(())
}

fn exec_command(matches: &ArgMatches, config: &Config) -> Result<()> {
    let creds = try!(get_credentials(config));

    let command_line: Vec<&str> = matches.values_of("command").unwrap().collect();
    
    let mut command_line_iter = command_line.into_iter();
    let command_name = command_line_iter.next().unwrap();
    let args: Vec<&str> = command_line_iter.collect();

    let env = try!(get_vars(matches, config, &creds));

    spawn_command(OsString::from(command_name).as_os_str(), &args[..], &env)
}

fn get_vars(_matches: &ArgMatches, config: &Config, creds: &rusoto::AwsCredentials) -> Result<HashMap<String, String>> {
    let mut env: HashMap<String, String> = HashMap::new();

    env.insert("AWS_ACCESS_KEY_ID".to_owned(), creds.aws_access_key_id().to_owned());
    env.insert("AWS_SECRET_ACCESS_KEY".to_owned(), creds.aws_secret_access_key().to_owned());

    if let Some(ref session_token) = *creds.token() {
        env.insert("AWS_SESSION_TOKEN".to_owned(), session_token.to_owned());
        env.insert("AWS_SECURITY_TOKEN".to_owned(), session_token.to_owned());
    }

    if let Some(region) = config.region {
        env.insert("AWS_DEFAULT_REGION".to_owned(), region.to_string());
    }

    Ok(env)
}

fn get_home_dir() -> Result<path::PathBuf> {
    env::home_dir()
        .ok_or_else(|| StsCliError::Error("can't find home directory".to_owned()))
}

fn get_default_config_path() -> Result<path::PathBuf> {
    let mut path = path::PathBuf::new();
    path.push(try!(get_home_dir()));
    path.push(".aws/config");
    Ok(path)
}

fn get_default_credentials_path() -> Result<path::PathBuf> {
    let mut path = path::PathBuf::new();
    path.push(try!(get_home_dir()));
    path.push(".aws/credentials");
    Ok(path)
}

fn list_profiles(_matches: &ArgMatches, config: &Config) -> Result<()> {
    let default_credentials_path = try!(get_default_credentials_path());
    let credentials_file = config.credentials_file.as_ref().unwrap_or(&default_credentials_path);
    let credentials_profiles = try!(aws_config::Config::load_from_path(credentials_file));
    for (p,_) in credentials_profiles.profiles {
        println!("{}", p);
    }

    let default_config_path = try!(get_default_config_path());
    let config_file = config.config_file.as_ref().unwrap_or(&default_config_path);
    let config_profiles = try!(aws_config::Config::load_from_path(config_file));
    for (p,_) in config_profiles.profiles {
        println!("{}", p);
    }

    Ok(())
}