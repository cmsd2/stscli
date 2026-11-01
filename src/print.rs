//! Turns session credentials into environment variables and the shell
//! syntax that sets them.

use std::collections::BTreeMap;

use clap::ValueEnum;

use crate::result::Result;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Bash,
    Fish,
    Powershell,
    Json,
}

/// Every line `get` prints, in a stable order.
pub fn render(vars: &BTreeMap<String, String>, format: Format, export: bool) -> Result<String> {
    if format == Format::Json {
        return Ok(format!("{}\n", serde_json::to_string(vars)?));
    }
    let mut out = String::new();
    for (k, v) in vars {
        out.push_str(&render_var(k, v, format, export));
        out.push('\n');
    }
    Ok(out)
}

fn render_var(k: &str, v: &str, format: Format, export: bool) -> String {
    match format {
        Format::Bash => {
            let prefix = if export { "export " } else { "" };
            format!(
                "{prefix}{k}=\"{}\"",
                escape(v, &['\\', '"', '$', '`'], '\\')
            )
        }
        Format::Fish => {
            let prefix = if export { "set -x" } else { "set" };
            format!("{prefix} {k} \"{}\"", escape(v, &['\\', '"', '$'], '\\'))
        }
        Format::Powershell => {
            let prefix = if export { "env:" } else { "" };
            format!("${prefix}{k} = \"{}\"", escape(v, &['`', '"', '$'], '`'))
        }
        Format::Json => unreachable!("JSON is rendered as one object"),
    }
}

/// Prefixes each character in `special` with `escape`, so the value reads
/// back literally inside a double-quoted string in that shell.
fn escape(v: &str, special: &[char], escape: char) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        if special.contains(&c) {
            out.push(escape);
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("B".to_owned(), "two".to_owned()),
            ("A".to_owned(), r#"a"b\c$d`e"#.to_owned()),
        ])
    }

    #[test]
    fn bash_escapes_and_sorts() {
        assert_eq!(
            render(&vars(), Format::Bash, true).unwrap(),
            "export A=\"a\\\"b\\\\c\\$d\\`e\"\nexport B=\"two\"\n"
        );
        assert_eq!(
            render(&vars(), Format::Bash, false).unwrap().lines().last(),
            Some("B=\"two\"")
        );
    }

    #[test]
    fn fish() {
        assert_eq!(
            render(&vars(), Format::Fish, true).unwrap(),
            "set -x A \"a\\\"b\\\\c\\$d`e\"\nset -x B \"two\"\n"
        );
        assert!(
            render(&vars(), Format::Fish, false)
                .unwrap()
                .starts_with("set A ")
        );
    }

    #[test]
    fn powershell() {
        assert_eq!(
            render(&vars(), Format::Powershell, true).unwrap(),
            "$env:A = \"a`\"b\\c`$d``e\"\n$env:B = \"two\"\n"
        );
        assert!(
            render(&vars(), Format::Powershell, false)
                .unwrap()
                .starts_with("$A = ")
        );
    }

    #[test]
    fn json_is_one_object() {
        let out = render(&vars(), Format::Json, false).unwrap();
        let back: BTreeMap<String, String> = serde_json::from_str(&out).unwrap();
        assert_eq!(back, vars());
    }
}
