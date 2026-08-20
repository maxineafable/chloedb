use chloedb::{DB, DBError};

fn main() -> Result<(), DBError> {
    let mut db = DB::open("./logs")?
        .set_max_bytes(100) // 100 bytes to rotate log file
        .set_max_logs(3); // 3 log files and will trigger log compact

    db.set(b"name", b"example")?;

    Ok(())
}
