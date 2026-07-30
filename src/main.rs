use clap::{Parser, Subcommand};

mod db;
mod file;

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

    let mut db = DB::open("log.txt")?;

    match args.command {
        Commands::Set { key, value } => db.set(key, value)?,
        Commands::Get { key } => match db.get(&key) {
            Some(value) => println!("{value}"),
            None => println!("key \"{}\" not found", key),
        },
        Commands::Remove { key } => db.remove(key)?,
    };

    db.compact_log()?;

    Ok(())
}
