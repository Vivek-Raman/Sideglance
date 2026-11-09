fn main() {
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => sideglance_lib::run(),
        [argument] if argument == "--mcp" => {
            if let Err(error) = sideglance_lib::mcp::run() {
                eprintln!("Sideglance MCP server failed: {error}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Usage: sideglance [--mcp]");
            std::process::exit(2);
        }
    }
}
