use chipmunk::io::job::load_job;
use clap::Parser;

#[derive(Parser)]
#[command(name = "chipmunk", about = "CLI CAM tool")]
struct Cli {
    /// YAML job file or "postprocessors" to list available postprocessors
    input: Option<String>,

    /// Optional output (defaults to stdout)
    #[arg(short, long)]
    output: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt::init();
    tracing::info!("Starting chipmunk");
    match cli.input.as_deref() {
        Some("postprocessors") => {
            println!("No postprocessors registered.");
        }
        Some(path) => {
            tracing::info!("Processing job file: {}", path);
            match load_job(path) {
                Ok(job) => println!("{:#?}", job),
                Err(e) => {
                    tracing::error!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            eprintln!("Error: no input file provided");
            std::process::exit(1);
        }
    }
}
