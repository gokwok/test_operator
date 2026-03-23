use operator_mcp::run_stdio_server;

#[tokio::main]
async fn main() {
    std::process::exit(match run_stdio_server().await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    });
}
