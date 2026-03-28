use chipmunk::{
    io::job::{load_job, run_job},
    nc,
};
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

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    match cli.input.as_deref() {
        Some("postprocessors") => {
            let postprocessors: Vec<String> = nc::postprocessors::list_postprocessors();
            println!("Available postprocessors: {}", postprocessors.join(","));
        }
        Some(path) => {
            tracing::info!("Processing job file: {}", path);
            match load_job(path) {
                Ok(job) => {
                    let nc_code: String = match run_job(job) {
                        Ok(nc) => nc,
                        Err(e) => {
                            tracing::error!("{}", e);
                            std::process::exit(1);
                        }
                    };

                    // Output to file or stdout
                    match &cli.output {
                        Some(output_path) => {
                            if output_path == "-" {
                                print!("{}", nc_code);
                            } else {
                                std::fs::write(output_path, &nc_code).unwrap_or_else(|e| {
                                    eprintln!("Error writing output: {}", e);
                                    std::process::exit(1);
                                });
                                println!("Written to {}", nc_code);
                            }
                        }
                        None => print!("{}", nc_code),
                    }
                }

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
