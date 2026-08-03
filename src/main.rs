use clap::{Parser, Subcommand};

mod db;
mod file;
mod binarylog;

use db::DB;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Set { key: String, value: String },
    Get { key: String },
    Remove { key: String },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut db = DB::open("log")?;

    match args.command {
        Commands::Set { key, value } => db.set(key, value)?,
        Commands::Get { key } => match db.get(&key) {
            Ok(value) => println!("{}", value),
            Err(e) => println!("{}", e),
        },
        Commands::Remove { key } => db.remove(key)?,
    };

    db.compact_log()?;

    Ok(())
}
