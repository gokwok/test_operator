use std::io::{self, Write};

use hmdriver_rs::cli::Cli;
use hmdriver_rs::{Driver, HdcError};

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, HdcError> {
    let cli = Cli::parse()?;
    let mut driver = Driver::builder(&cli.addr)
        .key_dir(cli.key_dir.clone())
        .connect_key(cli.effective_connect_key())
        .version(cli.version.clone())
        .timeout(cli.timeout)
        .connect()?;

    let result = driver.shell(cli.shell_command())?;
    io::stdout().write_all(&result.stdout)?;
    io::stdout().flush()?;
    for message in &result.messages {
        eprintln!("{}", message.text);
    }

    driver.close()?;
    Ok(if result.failed() { 1 } else { 0 })
}
