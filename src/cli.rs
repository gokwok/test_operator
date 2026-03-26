use std::path::PathBuf;
use std::time::Duration;

use crate::auth::default_key_dir;
use crate::error::{HdcError, Result};

#[derive(Debug, Clone)]
pub struct Cli {
    pub addr: String,
    pub key_dir: PathBuf,
    pub version: String,
    pub connect_key: Option<String>,
    pub timeout: Duration,
    shell_parts: Vec<String>,
}

impl Cli {
    pub fn parse() -> Result<Self> {
        Self::parse_from(std::env::args())
    }

    pub fn parse_from<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let args = args.into_iter().map(Into::into).collect::<Vec<String>>();
        if args.len() < 2 {
            return Err(HdcError::cli(
                "usage: hmdriver_rs tcp --addr <host:port> shell <command...>",
            ));
        }

        let mut index = 1;
        if args.get(index).map(String::as_str) != Some("tcp") {
            return Err(HdcError::cli("expected `tcp` subcommand"));
        }
        index += 1;

        let mut addr = None;
        let mut key_dir = default_key_dir();
        let mut version = crate::protocol::DEFAULT_VERSION.to_string();
        let mut connect_key = None;
        let mut timeout = Duration::from_secs(60);

        while index < args.len() {
            match args[index].as_str() {
                "shell" => {
                    index += 1;
                    break;
                }
                "--addr" => {
                    index += 1;
                    addr = Some(
                        args.get(index)
                            .cloned()
                            .ok_or_else(|| HdcError::cli("missing value for --addr"))?,
                    );
                }
                "--key-dir" => {
                    index += 1;
                    key_dir = PathBuf::from(
                        args.get(index)
                            .cloned()
                            .ok_or_else(|| HdcError::cli("missing value for --key-dir"))?,
                    );
                }
                "--version" => {
                    index += 1;
                    version = args
                        .get(index)
                        .cloned()
                        .ok_or_else(|| HdcError::cli("missing value for --version"))?;
                }
                "--connect-key" => {
                    index += 1;
                    connect_key = Some(
                        args.get(index)
                            .cloned()
                            .ok_or_else(|| HdcError::cli("missing value for --connect-key"))?,
                    );
                }
                "--timeout-ms" => {
                    index += 1;
                    let millis = args
                        .get(index)
                        .ok_or_else(|| HdcError::cli("missing value for --timeout-ms"))?
                        .parse::<u64>()?;
                    timeout = Duration::from_millis(millis);
                }
                other => return Err(HdcError::cli(format!("unknown argument `{other}`"))),
            }
            index += 1;
        }

        let shell_parts = args[index..].to_vec();
        if shell_parts.is_empty() {
            return Err(HdcError::cli("missing shell command"));
        }

        Ok(Self {
            addr: addr.ok_or_else(|| HdcError::cli("missing --addr"))?,
            key_dir,
            version,
            connect_key,
            timeout,
            shell_parts,
        })
    }

    pub fn shell_command(&self) -> String {
        self.shell_parts.join(" ")
    }

    pub fn effective_connect_key(&self) -> String {
        self.connect_key
            .clone()
            .unwrap_or_else(|| self.addr.clone())
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;

    #[test]
    fn cli_parses_shell_command_tail() {
        let args = vec![
            "hmdriver_rs",
            "tcp",
            "--addr",
            "192.168.8.43:35319",
            "shell",
            "ls",
            "/data/local/tmp",
        ];

        let cli = Cli::parse_from(args).unwrap();

        assert_eq!(cli.addr, "192.168.8.43:35319");
        assert_eq!(cli.shell_command(), "ls /data/local/tmp");
    }
}
