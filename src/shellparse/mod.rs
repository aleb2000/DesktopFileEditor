/*
* Copyright © 2025 Alessandro Balducci
*
* This file is part of Desktop File Editor.
* Desktop File Editor is free software: you can redistribute it and/or modify it under the terms of the 
* GNU General Public License as published by the Free Software Foundation, 
* either version 3 of the License, or (at your option) any later version.
* Desktop File Editor is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
* without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
* See the GNU General Public License for more details.
* You should have received a copy of the GNU General Public License along with Desktop File Editor. If not, see <https://www.gnu.org/licenses/>.
*/

use std::fmt::Display;

#[cfg(feature = "steam")]
mod steamutil;

#[derive(Debug, PartialEq, Clone)]
pub struct Command {
    pub command: String,
    pub args: Vec<String>,
    pub variables: Vec<(String, String)>,
}

#[cfg(feature = "steam")]
impl Command {
    const STEAM_ARG_FORMAT: &str = "steam://rungameid/";
    fn find_steam_appid(&self) -> Option<u64> {
        for arg in self.args.iter() {
            let arg = arg.trim();
            if arg.starts_with(Command::STEAM_ARG_FORMAT) {
                let appid = arg.trim_start_matches(Command::STEAM_ARG_FORMAT);
                return appid.parse::<u64>().ok();
            }
        }
        None
    }

    pub fn is_steam_app(&self) -> bool {
        self.command == "steam" && self.find_steam_appid().is_some()
    }

    pub fn is_steam_app_installed(&self) -> bool {
        if !self.is_steam_app() {
            return false;
        }

        let app_id = match self.find_steam_appid() {
            Some(app_id) => app_id,
            None => return false,
        };

        steamutil::is_app_installed(app_id)
    }

}

impl Command {
    pub fn is_env(&self) -> bool {
        self.command == "env"
    }

    /// "Flatten" commands that use the env command to start another binary by replacing the env
    /// commmand with the final binary and moving the environment variables to the variables list
    pub fn flatten_env(&mut self) {
        if !self.is_env() {
            return;
        }

        let mut binary_index = None;
        for (i, arg) in self.args.iter().enumerate() {
            if !arg.starts_with("-") && !arg.contains("=") {
                binary_index = Some(i);
                break;
            }
        }

        let binary_index = match binary_index {
            Some(binary_index) => binary_index,
            None => return,
        };

        let drain_iter = self.args.drain(0..binary_index).filter_map(|arg| {
            let (var, value) = parse_variable(&arg)?;
            Some((var.to_string(), value.to_string()))
        });
        self.variables.extend(drain_iter);

        let binary = self.args.remove(0);
        self.command = binary;
    }
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (var, value) in self.variables.iter() {
            write!(f, "{var}={value}")?;
            write!(f, " ")?;
        }

        write!(f, "{}", self.command)?;
        write!(f, " ")?;

        for arg in self.args[0..self.args.len() - 1].iter() {
            write!(f, "{arg}")?;
            write!(f, " ")?;
        }

        if let Some(last_arg) = self.args.last() {
            write!(f, "{last_arg}")
        } else {
            Ok(())
        }
    }
}

impl From<Command> for Vec<String> {
    fn from(value: Command) -> Self {
        value
            .variables
            .into_iter()
            .map(|(var, value)| format!("{var}={value}"))
            .chain(std::iter::once(value.command))
            .chain(value.args)
            .collect()
    }
}

fn parse_variable(token: &str) -> Option<(&str, &str)> {
    let parts: Vec<_> = token.splitn(2, '=').collect();
    if parts.len() != 2 {
        return None;
    }

    Some((parts[0], parts[1]))
}

pub fn parse(input: &str) -> Option<Command> {
    let mut token = String::new();
    let mut command = None;
    let mut args = Vec::new();
    let mut whitespace = false;
    let mut string_delim = None;
    let mut escape = false;
    let mut variables = Vec::new();

    fn token_finished(
        command: &mut Option<String>,
        args: &mut Vec<String>,
        variables: &mut Vec<(String, String)>,
        token: &mut String,
    ) {
        if token.is_empty() {
            return;
        }

        if command.is_none() {
            if let Some((varname, value)) = parse_variable(token) {
                // println!("Found variable {varname}={value}");
                variables.push((varname.to_string(), value.to_string()));
            } else {
                // println!("Found command {token}");
                *command = Some(token.clone());
            }
        } else {
            // println!("Found arg {token}");
            args.push(token.clone());
        }
        token.clear();
    }

    for c in input.chars() {
        let mut escape_set_this_iter = false;

        if whitespace && !c.is_whitespace() {
            token_finished(&mut command, &mut args, &mut variables, &mut token);
            whitespace = false;
        }

        match c {
            '\\' if !escape => {
                escape = true;
                escape_set_this_iter = true;
            }
            quote @ '"' | quote @ '\'' if !escape => match string_delim {
                Some(delim) if quote == delim => string_delim = None,
                None => string_delim = Some(quote),
                _ => token.push(c),
            },

            _ if c.is_whitespace() && string_delim.is_none() && !escape => {
                whitespace = true;
            }

            _ => {
                token.push(c);
            }
        }

        if escape && !escape_set_this_iter {
            escape = false;
        }
    }

    token_finished(&mut command, &mut args, &mut variables, &mut token);

    Some(Command {
        command: command?,
        args,
        variables,
    })
}

#[cfg(test)]
mod test {
    use crate::shellparse::Command;

    use super::parse;

    fn cmd(command: &str, args: &[&str]) -> Option<Command> {
        cmd_vars(command, args, &[])
    }

    fn cmd_vars(command: &str, args: &[&str], vars: &[(&str, &str)]) -> Option<Command> {
        if command.is_empty() {
            return None;
        }

        Some(Command {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            variables: vars
                .iter()
                .map(|(name, val)| (name.to_string(), val.to_string()))
                .collect(),
        })
    }

    #[test]
    fn empty() {
        let command = parse("");
        assert_eq!(command, None)
    }

    #[test]
    fn single() {
        let command = parse("binary_name");
        assert_eq!(command, cmd("binary_name", &[]))
    }

    #[test]
    fn simple_one_arg() {
        let command = parse("cmd one");
        assert_eq!(command, cmd("cmd", &["one"]))
    }

    #[test]
    fn simple_two_args() {
        let command = parse("cmd one two");
        assert_eq!(command, cmd("cmd", &["one", "two"]))
    }

    #[test]
    fn simple() {
        let command = parse("./bin how are you doing?");
        assert_eq!(command, cmd("./bin", &["how", "are", "you", "doing?"]))
    }

    #[test]
    fn multi_whitespace() {
        let command = parse("./bin  how   are    you     doing?");
        assert_eq!(command, cmd("./bin", &["how", "are", "you", "doing?"]))
    }

    #[test]
    fn leading_trailing_whitespace() {
        let command = parse("    ./bin  how   are    you     doing?       ");
        assert_eq!(command, cmd("./bin", &["how", "are", "you", "doing?"]))
    }

    #[test]
    fn string() {
        let command = parse(r#"cmd "string" "string with space in between""#);
        assert_eq!(
            command,
            cmd("cmd", &["string", "string with space in between"])
        )
    }

    #[test]
    fn string_escape_quotes() {
        let command = parse(r#"cmd "string with \"escaped\" quotes""#);
        assert_eq!(command, cmd("cmd", &["string with \"escaped\" quotes"]))
    }

    #[test]
    fn string_different_delims() {
        let command = parse(
            r#"cmd "'single' quotes in doubly quoted string" '"double" quotes in singly quoted string'"#,
        );
        assert_eq!(
            command,
            cmd(
                "cmd",
                &[
                    "'single' quotes in doubly quoted string",
                    "\"double\" quotes in singly quoted string"
                ]
            )
        )
    }

    #[test]
    fn escape_whitespace() {
        let command = parse(r#"cmd escaped\ whitespace"#);
        assert_eq!(command, cmd("cmd", &["escaped whitespace"]))
    }

    #[test]
    fn vars() {
        let command = parse(r#"VAR1=value1 VAR2="value 2" VAR3=test"val" bin"#);
        assert_eq!(
            command,
            cmd_vars(
                "bin",
                &[],
                &[("VAR1", "value1"), ("VAR2", "value 2"), ("VAR3", "testval")]
            )
        )
    }

    // This currently fails, I don't know if I want to fix this
    // #[test]
    // fn quoted_var() {
    //     let command = parse(r#""TEST=testval" bin"#);
    //     assert_eq!(command, cmd("TEST=testval", &["bin"]));
    // }

    #[test]
    fn real_test1() {
        let command = parse(
            r#"/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=amberol --file-forwarding io.bassi.Amberol @@u %U @@"#,
        );
        assert_eq!(
            command,
            cmd(
                "/usr/bin/flatpak",
                &[
                    "run",
                    "--branch=stable",
                    "--arch=x86_64",
                    "--command=amberol",
                    "--file-forwarding",
                    "io.bassi.Amberol",
                    "@@u",
                    "%U",
                    "@@"
                ]
            )
        )
    }

    #[test]
    fn real_test2() {
        let command = parse(r#"steam steam://rungameid/221380"#);
        assert_eq!(command, cmd("steam", &["steam://rungameid/221380"]))
    }

    #[test]
    fn real_test3() {
        let command = parse(
            "env WINEPREFIX=\"/home/user/Games/league-of-legends\" wine C:\\\\ProgramData\\\\Microsoft\\\\Windows\\\\Start\\ Menu\\\\Programs\\\\Riot\\ Games\\\\League\\ of\\ Legends.lnk",
        );
        assert_eq!(
            command,
            cmd(
                "env",
                &[
                    "WINEPREFIX=/home/user/Games/league-of-legends",
                    "wine",
                    "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Riot Games\\League of Legends.lnk"
                ]
            )
        );
    }

    #[test]
    fn real_test4() {
        let command = parse("printf \"|||%%s|||\\\\n\" \"quoting terminal\" \"with 'complex' arguments,\" \"quotes \\\",\" \"\" 	\"empty args,\" \"new\nlines,\" \"and \\\"back\\\\slashes\\\"\"").unwrap();

        assert_eq!(
            command,
            cmd(
                "printf",
                &[
                    "|||%%s|||\\n",
                    "quoting terminal",
                    "with 'complex' arguments,",
                    "quotes \",",
                    "empty args,",
                    "new\nlines,",
                    "and \"back\\slashes\"",
                ]
            )
            .unwrap()
        );
    }

    #[cfg(feature = "steam")]
    mod steam {
        use crate::shellparse::{parse, test::cmd};

        #[test]
        fn not_steam() {
            let command = parse(r#"notsteam steam://rungameid/221380"#).unwrap();
            assert_eq!(
                command,
                cmd("notsteam", &["steam://rungameid/221380"]).unwrap()
            );
            assert!(!command.is_steam_app());
        }

        #[test]
        fn steam() {
            let command = parse(r#"steam steam://rungameid/221380"#).unwrap();
            assert_eq!(
                command,
                cmd("steam", &["steam://rungameid/221380"]).unwrap()
            );
            assert!(command.is_steam_app());
            assert_eq!(command.find_steam_appid(), Some(221380));
        }

        #[test]
        fn steam_invalid_appid() {
            let command = parse(r#"steam steam://rungameid/221380/asd"#).unwrap();
            assert_eq!(
                command,
                cmd("steam", &["steam://rungameid/221380/asd"]).unwrap()
            );
            assert!(!command.is_steam_app());
            assert_eq!(command.find_steam_appid(), None);
        }

        #[test]
        fn steam_invalid_appid2() {
            let command = parse(r#"steam steam://rungameid/"#).unwrap();
            assert_eq!(command, cmd("steam", &["steam://rungameid/"]).unwrap());
            assert!(!command.is_steam_app());
            assert_eq!(command.find_steam_appid(), None);
        }

        #[test]
        fn steam_invalid_appid3() {
            let command = parse(r#"steam steam://rungameid/aaabbb"#).unwrap();
            assert_eq!(
                command,
                cmd("steam", &["steam://rungameid/aaabbb"]).unwrap()
            );
            assert!(!command.is_steam_app());
            assert_eq!(command.find_steam_appid(), None);
        }

        #[test]
        fn steam_invalid_appid4() {
            let command = parse(r#"steam 221380"#).unwrap();
            assert_eq!(command, cmd("steam", &["221380"]).unwrap());
            assert!(!command.is_steam_app());
            assert_eq!(command.find_steam_appid(), None);
        }
    }
}
