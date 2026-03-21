use std::io::{BufRead, Write};

use crate::server::parse_error_response;
use crate::McpServer;

#[derive(Debug, thiserror::Error)]
pub enum StdioTransportError {
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub fn run_stdio_session<R, W>(
    server: &McpServer,
    reader: R,
    writer: &mut W,
) -> Result<(), StdioTransportError>
where
    R: BufRead,
    W: Write,
{
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str(&line) {
            Ok(message) => server.handle_message(message)?,
            Err(_) => Some(parse_error_response()),
        };

        if let Some(response) = response {
            serde_json::to_writer(&mut *writer, &response)?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
    }

    Ok(())
}
