use clap::{Parser, Subcommand};

use chloedb::db::{DB, DBError};

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
    List,
}

fn main() -> Result<(), DBError> {
    let args = Args::parse();

    let mut db = DB::open()?;

    match args.command {
        Commands::Set { key, value } => match db.set(key, value) {
            Ok(_) => (),
            Err(e) => println!("{}", e),
        },
        Commands::Get { key } => match db.get(&key) {
            Ok(value) => println!("{}", value),
            Err(e) => println!("{}", e),
        },
        Commands::Remove { key } => match db.remove(key) {
            Ok(_) => (),
            Err(e) => println!("{}", e),
        },
        Commands::List => {
            let keys = db.list();
            println!("Current Keys:");
            for k in keys {
                println!("{}", k);
            }
        }
    };

    db.compact_log()?;

    Ok(())
}
