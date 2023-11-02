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


use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rocket::response::Redirect;

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
    
    if let Some((other_username, other_user)) = users.iter_mut().find(|(_, user)| user.lock().unwrap().room.is_none() && *user.lock().unwrap().username != username) {
        let room_id = generate_room_id();
        users.get(&username).unwrap().lock().unwrap().room = Some(room_id.clone());
        other_user.lock().unwrap().room = Some(room_id.clone());

        Redirect::to(format!("/room/{}", room_id))
    } else {

        users.insert(username.clone(), Arc::new(Mutex::new(User { username, room: None })));

        Redirect::to("/waiting")
    }
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

#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(channel::<Message>(1024).0)
        .mount("/", routes![post, events, random])
        .mount("/", FileServer::from(relative!("static")))
}
