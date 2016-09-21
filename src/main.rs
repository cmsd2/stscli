extern crate clap;
extern crate rusoto;
#[macro_use]
extern crate quick_error;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate rustc_serialize;
extern crate regex;

use clap::{Arg, ArgMatches, App, SubCommand};
use std::{result};
use std::io;
use std::io::Write;
use std::str::FromStr;
use std::path::PathBuf;
use std::collections::HashMap;
use std::process;
use std::ffi::{OsStr, OsString};
use regex::Regex;
use rustc_serialize::json;
use rusoto::*;

quick_error! {
    #[derive(Debug)]
    pub enum StsCliError {
        Io(err: io::Error) {
            from()
            description("io error")
            display("I/O error: {}", err)
            cause(err)
        }

        Credentials(err: rusoto::CredentialsError) {
            from()
            description("aws credentials error")
            display("AWS Credentials error: {}", err)
            cause(err)
        }

        Region(err: rusoto::ParseRegionError) {
            from()
            description("aws region error")
            display("AWS Region parser error: {}", err)
            cause(err)
        }

        ProcessKilled {
            description("process killed")
            display("process killed")
        }

        ChildExited(code: i32) {
            description("child exited")
            display("child exited: {}", code)
        }

        JsonEncoderError(err: json::EncoderError) {
            from()
            description("json encoder error")
            display("Json encoder error: {}", err)
            cause(err)
        }
    }
}

pub type Result<T> = result::Result<T, StsCliError>;

#[derive(Copy, Clone, Debug)]
pub enum OutputFormat {
    Bash { export: bool },
    Fish { export: bool },
    Powershell { export: bool },
    Json
}

#[derive(Debug, Clone)]
pub struct Config {
    config_file: Option<PathBuf>,
    credentials_file: Option<PathBuf>,
    profile: Option<String>,
    role: Option<String>,
    region: Option<rusoto::Region>,
    name: Option<String>,
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

pub fn main() {
    env_logger::init().unwrap();
    
    let mut stderr = std::io::stderr();

    let matches = App::new("rusoto-sts")
        .version("1.0")
        .author("various")
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
        .get_matches();
    
    match run_subcommand(&matches) {
        Err(err) => {
            writeln!(&mut stderr, "Error: {}", err).unwrap();
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
        _ => Ok(())
    }
}

fn get_credentials(config: &Config) -> Result<rusoto::AwsCredentials> {
    let mut profile_provider = try!(ProfileProvider::new());

    if let Some(ref config_file_name) = config.config_file {
        profile_provider.set_config_file_path(config_file_name);
    }

    if let Some(ref credentials_file_name) = config.credentials_file {
        profile_provider.set_file_path(credentials_file_name);
    }

    if let Some(ref profile) = config.profile {
        profile_provider.set_profile(&profile[..]);
    }

    let base_provider = ChainProvider::with_profile_provider(profile_provider);

    let mut provider = try!(StsProvider::new(base_provider));

    provider.set_region(config.region);
    provider.set_role_arn(config.role.clone());
    provider.set_profile(config.profile.clone());
    provider.set_session_name(config.name.clone());

    if config.config_file.is_some() {
        provider.set_config_file_path(config.config_file.clone());
    }

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

pub fn print_vars_json(_args: &ArgMatches, _config: &Config, vars: &HashMap<String, String>) -> Result<()> {
    let vars_json = try!(json::encode(vars).map_err(StsCliError::from));

    println!("{}", vars_json);

    Ok(())
}

pub fn print_vars(_args: &ArgMatches, _config: &Config, vars: &HashMap<String, String>, output_format: OutputFormat) -> Result<()> {
    for (k, v) in vars {
        print_var(k, v, output_format);
    }

    Ok(())
}

pub fn print_var(k: &str, v: &str, output_format: OutputFormat) {
    match output_format {
        OutputFormat::Bash{export} => {
            print_bash_var(k, v, export);
        },
        OutputFormat::Fish{export} => {
            print_var_fish(k, v, export);
        },
        OutputFormat::Powershell{export} => {
            print_var_ps(k, v, export);
        },
        _ => unreachable!()
    };
}

pub fn spawn_command<S>(command_str: &OsStr, args: &[S], env: &HashMap<String, String>) -> Result<()> where S: AsRef<OsStr> {
        
    let mut command = process::Command::new(command_str);
    command.args(&args);

    for (k,v) in env {
        command.env(k, v);
    }

    {
        let mut result = try!(command.spawn());
        
        let status = try!(result.wait());
        
        status.code().ok_or(StsCliError::ProcessKilled).and_then(|code| {
            if code == 0 {
                Ok(())
            } else {
                Err(StsCliError::ChildExited(code))
            }
        })
    }
}

fn print_bash_var(k: &str, v: &str, export_vars: bool) {
    let re = shell_re();

    let export_prefix = if export_vars { "export " } else { "" };

    let escaped = re.replace_all(v, shell_esc());
    println!("{}{}=\"{}\"", export_prefix, k, escaped);
}

fn print_var_fish(k: &str, v: &str, export_vars: bool) {
    let re = shell_re();

    let export_prefix = if export_vars { "set -x" } else { "set" };

    let escaped = re.replace_all(v, shell_esc());
    println!("{} {} \"{}\"", export_prefix, k, escaped);
}

fn print_var_ps(k: &str, v: &str, export_vars: bool) {
    let re = powershell_re();

    let export_prefix = if export_vars { "env:" } else { "" };

    let escaped = re.replace_all(v, powershell_esc());
    println!("${}{} = \"{}\"", export_prefix, k, escaped);
}

fn shell_re() -> Regex {
    let pattern = r#"[\\"]"#;
    Regex::new(pattern).unwrap()
}

fn shell_esc() -> &'static str {
    "\\$0"
}

fn powershell_re() -> Regex {
    let pattern = r#"[\0\r\n\t`"]"#;
    Regex::new(pattern).unwrap()
}

fn powershell_esc() -> &'static str {
    "`$0"
}
