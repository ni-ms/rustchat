#[derive(Insertable)]
#[table_name="users"]
pub struct NewUser {
    pub ip: String,
    pub username: String,
}
