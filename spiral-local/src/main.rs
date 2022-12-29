use anyhow::Error;
use backtrace::Backtrace as ExternalBacktrace;
use clap::{Args, Subcommand, Parser};
use log::{debug, info, warn};

use spiral::{EmptyPackage, Architecture};

use std::env;
use std::fs;
use std::panic;
use std::path::PathBuf;

// Constants
/// Program version (from `Cargo.toml`)
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Program description (from `Cargo.toml`)
const PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
/// Program repository (from `Cargo.toml`)
const PKG_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

#[derive(Args, Debug)]
struct GenerateOpts {
    #[clap(short = 'n', long = "name", help = "Name of the package")]
    package_name: String,
    #[clap(short = 'p', long = "package-version", help = "Version of the package")]
    package_version: String,
    #[clap(short = 'd', long = "depend", help = "Dependencies of the package")]
    dependencies: Vec<String>,
    #[clap(
        short = 'o',
        long = "output",
        help = "Output path of the generated package"
    )]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct InstallOpts {
    packages: Vec<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Generate(GenerateOpts),
}

#[derive(Parser, Debug)]
#[clap(author, version = PKG_VERSION, about = PKG_DESCRIPTION)]
struct Opts {
    #[command(subcommand)]
    commands: Commands,
}

/// Set panic hook with repository information
fn setup_panic_hook() {
    panic::set_hook(Box::new(move |panic_info: &panic::PanicInfo| {
        if let Some(info) = panic_info.payload().downcast_ref::<&str>() {
            println!("Panic occurred: {:?}", info);
        } else {
            println!("Panic occurred");
        }
        if let Some(location) = panic_info.location() {
            println!(
                r#"In file "{}" at line "{}""#,
                location.file(),
                location.line()
            );
        }
        println!("Traceback:");
        println!("{:#?}", ExternalBacktrace::new());
        println!();
        println!("Please report this error to {}/issues", PKG_REPOSITORY);
    }));
}

fn handle_generate(opts: GenerateOpts) -> Result<(), Error> {
    // Generate the package
    let package = EmptyPackage::new(
        opts.package_name.as_str(),
        opts.package_version.as_str(),
        Architecture::ALL,
        "Spiral Admin <admin@spiral.v2bv.net>",
        "Spiral package",
        opts.dependencies,
    );
    let output_path = if let Some(output) = opts.output {
        output
    } else {
        PathBuf::from(format!(
            "./{}-{}-noarch.package",
            opts.package_name, opts.package_version
        ))
    };
    fs::write(output_path, package.build()?)?;
    Ok(())
}

fn main() -> Result<(), Error> {
    // Setup panic hook
    setup_panic_hook();

    // Setup logger
    if env::var("SPIRAL_LOG").is_err() {
        env::set_var("SPIRAL_LOG", "WARN");
    }
    if let Err(e) = pretty_env_logger::try_init_custom_env("SPIRAL_LOG") {
        panic!("Failed to initialize logger: {}", e);
    }

    // Parse commandline options
    let opts: Opts = Opts::parse();
    debug!("Target: {:?}", opts);

    match opts.commands {
        Commands::Generate(o) => handle_generate(o),
    }
}
