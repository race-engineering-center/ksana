use clap::{Parser, Subcommand};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

mod commands;
mod io;
mod shm;
mod sims;
mod sleeper;
mod traits;

pub use traits::{Connector, Player, Sleeper};

#[cfg(not(windows))]
compile_error!("This project only supports Windows");

#[derive(Parser)]
#[command(name = "ksana")]
#[command(about = "Record and playback simulator shared memory")]
#[command(subcommand_required = false)]
#[command(disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Record shared memory to file (default)
    Dump {
        /// Frames per second [1-60]
        #[arg(short, long, default_value_t = 5)]
        fps: u32,
    },
    /// Play back recorded file to shared memory
    Play {
        /// Input file to play
        #[arg(short, long)]
        input: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let should_quit = Arc::new(AtomicBool::new(false));
    let quit_flag = should_quit.clone();

    ctrlc::set_handler(move || {
        should_quit.store(true, Ordering::Relaxed);
        println!("\nCtrl+C received. Stopping... Please wait patiently.");
    })?;

    match cli.command.unwrap_or(Commands::Dump { fps: 5 }) {
        Commands::Dump { fps } => {
            commands::dump::run(quit_flag, fps.clamp(1, 60))?;
        }
        Commands::Play { input } => {
            commands::play::run(quit_flag, &input)?;
        }
    }

    Ok(())
}
