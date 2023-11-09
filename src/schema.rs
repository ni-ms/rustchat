#[macro_use]
extern crate diesel;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

table! {
    users (ip) {
        ip -> Text,
        username -> Text,
    }
}

#[derive(Insertable)]
#[table_name="users"]
struct NewUser {
    ip: String,
    username: String,
}

#[derive(Queryable)]
struct User {
    ip: String,
    username: String,
}
