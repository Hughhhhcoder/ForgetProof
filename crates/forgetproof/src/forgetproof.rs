fn main() {
    if let Err(error) = memoryproof::cli::run() {
        eprintln!("error: {error:#}");
        std::process::exit(2);
    }
}
