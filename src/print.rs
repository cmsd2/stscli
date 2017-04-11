use regex::Regex;
use std::process;
use clap::ArgMatches;
use std::collections::HashMap;
use rustc_serialize::json;
use std::ffi::OsStr;
use result::*;
use config::*;

#[derive(Copy, Clone, Debug)]
pub enum OutputFormat {
    Bash { export: bool },
    Fish { export: bool },
    Powershell { export: bool },
    Json
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
    command.args(args);

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
