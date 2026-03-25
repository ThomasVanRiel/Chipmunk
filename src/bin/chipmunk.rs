use anyhow::Error;
use chipmunk::{
    core::toolpath::ToolpathSegment,
    io::job::load_job,
    nc::{self, ir::NCBlock},
    toolpath::patterns::Pattern,
};
use clap::Parser;
use std::path::Path;

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
                    // TODO: Replace this LLM generated placeholder implementation that was used to
                    // test the function of the program.
                    let pp = nc::postprocessors::find_postprocessor(&job.postprocessor);
                    let name: String = job.name.unwrap_or_else(|| {
                        Path::new(path)
                            .file_stem()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned()
                    });

                    let units_str = match job.units {
                        chipmunk::core::units::Units::Mm => "MM",
                        chipmunk::core::units::Units::Inch => "INCH",
                    };

                    // TODO: Clearance might be defined per operation as an override
                    let clearance = job.clearance;
                    // Operations will be a single operation in this test
                    let operation = job.operations.first().unwrap();
                    let tool = chipmunk::core::tool::Tool {
                        tool_number: operation.tool_number.unwrap_or(1),
                        name: operation.tool_name.clone().unwrap_or_default(),
                        diameter: operation.tool_diameter.unwrap_or(0.0),
                        spindle_speed: operation.spindle_speed,
                        spindle_direction: chipmunk::core::tool::SpindleDirection::Cw,
                    };

                    let pattern = operation.pattern.as_ref().unwrap();
                    tracing::info!("{:?}", pattern);
                    let points = match &pattern {
                        Pattern::List { points } => points,
                        _ => &vec![[0f64, 0f64]], // Execute at 0,0 if no pattern was provided
                    };
                    let segments = points
                        .iter()
                        .map(|[x, y]| ToolpathSegment {
                            move_type: chipmunk::core::toolpath::MoveType::Rapid,
                            x: *x,
                            y: *y,
                            z: clearance,
                        })
                        .collect::<Vec<_>>();
                    let blocks: Vec<NCBlock> = nc::compiler::compile_manual_drill(
                        &name, units_str, &tool, clearance, &segments,
                    );

                    // Load post-processor Lua files
                    // TODO: Should we include the built in postprocessors in the binary?
                    let pp_path = pp.unwrap_or_else(|| {
                        eprintln!("Error: post-processor '{}' not found", job.postprocessor);
                        std::process::exit(1);
                    });
                    let pp_lua = std::fs::read_to_string(&pp_path).unwrap_or_else(|e| {
                        eprintln!("Error reading post-processor: {}", e);
                        std::process::exit(1);
                    });
                    let nc_output = nc::bridge::generate_nc(&pp_lua, &blocks, &name, units_str)
                        .unwrap_or_else(|e| {
                            eprintln!("Error generating NC: {}", e);
                            std::process::exit(1);
                        });

                    // Output to file or stdout
                    match &cli.output {
                        Some(output_path) => {
                            if output_path == "-" {
                                print!("{}", nc_output);
                            } else {
                                std::fs::write(output_path, &nc_output).unwrap_or_else(|e| {
                                    eprintln!("Error writing output: {}", e);
                                    std::process::exit(1);
                                });
                                println!("Written to {}", output_path);
                            }
                        }
                        None => print!("{}", nc_output),
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
