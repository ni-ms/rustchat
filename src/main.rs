// Macro use allows us to use macros from the rocket crate throughout our code
#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

use rocket::{State, Shutdown};
use rocket::fs::{relative, FileServer};
use rocket::form::Form;
use rocket::response::stream::{EventStream, Event};
use rocket::serde::{Serialize, Deserialize};
use rocket::tokio::sync::broadcast::{channel, Sender, error::RecvError};
use rocket::tokio::select;
use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};
use rocket::Request;

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use diesel::SqliteConnection;
use rocket::response::Redirect;

use diesel::prelude::*;
mod schema;
use schema::users;
mod user;
use user::NewUser;
use rand::Rng;

#[derive(Clone, Debug, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct User {
    username: String,
    room: Option<String>,
}

struct AppState {
    users: Mutex<HashMap<String, Arc<Mutex<User>>>>,
}

fn generate_room_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let room_id: String = (0..6).map(|_| rng.gen_range(0..10)).map(|n| std::char::from_digit(n, 10).unwrap()).collect();
    room_id
}


#[post("/request_chat", data = "<username>")]
fn request_chat(username: String, state: &State<AppState>) -> Redirect {
    let mut users = state.users.lock().unwrap();
    Redirect::to(format!("/room/{}", generate_room_id()))
}

#[get("/leave_chat", data = "<username>")]
fn leave_chat(username: String, state: &State<AppState>) -> Redirect {
    let mut users = state.users.lock().unwrap();
    if let Some(user) = users.get(&username) {
        user.lock().unwrap().room = None;
    }
    Redirect::to("/")
}

// struct derives debug, clone, fromform for form data, and serde for json, serialize and deserialize
#[derive(Clone, Debug, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..30))]
    pub username: String,
    pub message: String,
}

#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    let res = queue.send(form.into_inner());
}

#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    let mut rx = queue.subscribe();
    EventStream! {
       loop{
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };

            yield Event::json(&msg);
        }
    }
}

#[get("/random")]
async fn random() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join("random.html")).await.ok()
}

// /api/set_user and api/get_user for setting and getting user based on ip?
#[post("/api/set_user", data = "<user_form>")]
fn set_user(user_form: Form<NewUser>, request: &Request<'_>) -> String {
    let mut connection = establish_connection();
    let ip = get_client_ip(request);
    if let Some(ip) = ip {
        diesel::insert_into(users::table)
            .values(&NewUser { ip: ip.to_string(), username: user_form.into_inner().username })
            .execute(&mut connection)
            .expect("Error saving new user");
        return format!("Username set for IP: {}", ip);
    }
    "Failed to set username.".to_string()
}

#[get("/api/get_user")]
fn get_user(request: &Request<'_>) -> Option<String> {
    let mut connection = establish_connection();
    let ip = get_client_ip(request);
    if let Some(ip) = ip {
        let user: User = users::table.find(ip.to_string()).first(&mut connection).ok()?;
        return Some(user.username);
    }
    None
}

fn get_client_ip(request: &Request<'_>) -> Option<IpAddr> {
    if let Some(real_ip) = request.headers().get_one("X-Real-IP") {
        if let Ok(ip) = real_ip.parse() {
            return Some(ip);
        }
    }
    if let Some(forwarded_for) = request.headers().get_one("X-Forwarded-For") {
        if let Some(ip) = forwarded_for.split(',').next() {
            if let Ok(ip) = ip.trim().parse() {
                return Some(ip);
            }
        }
    }
    request.client_ip()
}

pub fn establish_connection() -> SqliteConnection {
    let database_url = "sqlite:users.db";
    SqliteConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}


#[launch]
fn rocket() -> _ {
    let user_map: HashMap<String, String> = HashMap::new();
    rocket::build()
        .manage(Mutex::new(user_map))
        .manage(channel::<Message>(1024).0)
        .mount("/", routes![post, events, random])
        .mount("/", FileServer::from(relative!("static")))
}
