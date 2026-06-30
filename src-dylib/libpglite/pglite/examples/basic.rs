use pglite::Connection;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(file!())
        .parent()
        .unwrap()
        .join(".pglite-data");
    let mut conn = Connection::open_with(path).unwrap();

    let rst = conn.query("SELECT version(), random()").unwrap();
    dbg!(rst);

    conn.query(
        "
        CREATE TABLE IF NOT EXISTS messages (
            id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            body text NOT NULL
        );
        INSERT INTO messages (body) VALUES ('hello from PGlite');
        ",
    )
    .unwrap();

    let rst = conn
        .query("SELECT id, body FROM messages ORDER BY id")
        .unwrap();
    dbg!(rst);
}
