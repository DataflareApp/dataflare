use chdb::Connection;

#[tokio::main]
async fn main() {
    let conn = Connection::connect(":memory:").await.unwrap();

    conn.execute(r"create table if not exists test (id UInt32, name String)")
        .unwrap();
    conn.execute("insert into test (id, name) values (1, 'Alice')")
        .unwrap();
    conn.execute("insert into test (id, name) values (2, 'Bob')")
        .unwrap();

    let query = conn.query(r#"select * from test"#).unwrap();
    dbg!(query);
}
