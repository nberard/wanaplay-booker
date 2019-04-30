#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
extern crate rocket_contrib;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate regex;
extern crate serde_json;
extern crate serde_yaml;
pub type Error = failure::Error;
use failure::bail;
use regex::Regex;
use rocket::http::Status;
use rocket::response::status;
use rocket_contrib::json::Json;
use serde_yaml::from_reader;
use std::str;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::result::Result;
use wanaplay_booker::*;
use select::document::Document;
use select::predicate::Class;

const WANAPLAY_SERVICE_LABEL: &str = "wanaplay_type=bot";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Service {
    image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volumes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<String>>,
}

impl From<Json<Watcher>> for Service {
    fn from(watcher: Json<Watcher>) -> Self {
        Service {
            image: "touplitoui/wanaplay-booker-bot".to_string(),
            environment: Some(vec![
                format!("wanaplay_login={}", env::var("wanaplay_login").unwrap()),
                format!(
                    "wanaplay_password={}",
                    env::var("wanaplay_password").unwrap()
                ),
            ]),
            command: Some(format!(
                "wanaplay-booker -c {}:00 -w {}",
                watcher.court_time, watcher.week_day
            )),
            volumes: None,
            ports: None,
            labels: Some(vec![WANAPLAY_SERVICE_LABEL.to_string()])
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Compose {
    #[serde(skip)]
    path: PathBuf,
    version: String,
    services: BTreeMap<String, Service>,
}

impl Compose {
    pub fn get() -> Self {
        let path =
            fs::canonicalize(&PathBuf::from(env::var("compose_file_path").unwrap())).unwrap();
        let mut compose: Self = from_reader(std::fs::File::open(path.clone()).unwrap()).unwrap();
        compose.path = path;
        compose
    }

    pub fn update(&self) {
        let serialized_report = serde_yaml::to_string(&self).unwrap();
        std::fs::write(self.path.clone(), serialized_report).unwrap();
    }

    pub fn add_service(&mut self, name: String, service: Service) {
        self.services.insert(name, service);
    }

    pub fn remove_service(&mut self, name: &str) -> Result<(), Error> {
        if self.services.contains_key(name) {
            self.services.remove(name);
            Ok(())
        } else {
            bail!("service {:?} not found", name);
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
enum WatcherStatus {
    Created,
    Running,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ErrorContainer {
    pub errors: Vec<String>,
}

impl ErrorContainer {
    pub fn new(errors: Vec<String>) -> Self {
        ErrorContainer { errors }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Watcher {
    name: String,
    status: String,
    court_time: String,
    week_day: String,
}

impl From<Service> for Watcher {
    fn from(service: Service) -> Self {
        let re = Regex::new(r"wanaplay-booker -c (\d{2}:\d{2}):\d{2} -w (\w+)").unwrap();
        let command = &service.command.unwrap();
        let matches = re.captures(command).unwrap();
        Watcher {
            name: "".to_string(),
            status: "Created".to_string(),
            court_time: matches.get(1).unwrap().as_str().to_string(),
            week_day: matches.get(2).unwrap().as_str().to_string(),
        }
    }
}

fn get_bots() -> Vec<Watcher> {
    let compose = Compose::get();
    let bots: Vec<Watcher> = compose
        .services
        .into_iter()
        .filter_map(|(name, elt)| {
            if let Some(labels) = elt.labels.clone() {
                if labels.into_iter().find(|label| *label == WANAPLAY_SERVICE_LABEL.to_string()).is_some() {
                    let mut watcher = Watcher::from(elt);
                    watcher.name = name.clone();
                    let output = Command::new("docker")
                        .arg("-H")
                        .arg("unix:///var/run/docker.sock")
                        .arg("ps")
                        .arg("--filter")
                        .arg(format!("name={}", name.clone()))
                        .arg("--format")
                        .arg("{{.Status}}")
                        .output()
                        .expect("failed to execute process");
                    if !output.stdout.is_empty() {
                        watcher.status = String::from_utf8(output.stdout).unwrap().trim().to_string();
                    }
                    return Some(watcher);
                }
            }
            None
        })
        .collect();
    bots
}

#[get("/bots")]
fn get_all_bots() -> Json<Vec<Watcher>> {
    Json(get_bots())
}

#[get("/bots/<id>")]
fn get_bot(id: String) -> Option<Json<Watcher>> {
    get_bots()
        .into_iter()
        .find(|bot| bot.name == id)
        .map(|bot| Json(bot))
}

#[delete("/bots/<id>")]
fn remove_bot(id: String) -> Status {
    let mut compose = Compose::get();
    let removed = compose.remove_service(&id);
    match removed {
        Ok(_) => {
            let output = Command::new("docker")
                .arg("-H")
                .arg("unix:///var/run/docker.sock")
                .arg("service")
                .arg("rm")
                .arg(format!("wanaplay_{}", id.clone()))
                .output()
                .expect("failed to execute process");
            match output.status.success() {
                true => {
                    compose.update();
                    Status::NoContent
                },
                false => Status::InternalServerError
            }
        },
        Err(_) => Status::NotFound,
    }
}

#[post("/bots", format = "json", data = "<watcher>")]
fn new_bot(
    watcher: Json<Watcher>,
) -> Result<status::Created<Json<Watcher>>, status::BadRequest<Json<ErrorContainer>>> {
    match get_bot(watcher.name.clone()) {
        Some(_) => Err(status::BadRequest(Some(Json(ErrorContainer::new(vec![
            "watcher already exists".to_string(),
        ]))))),
        None => {
            let mut compose = Compose::get();
            let watcher_result = watcher.clone();
            compose.add_service(watcher.name.clone(), Service::from(watcher));
            compose.update();
            Ok(status::Created(
                format!("/bots/{}", watcher_result.name.clone()),
                Some(Json(watcher_result)),
            ))
        }
    }
}

#[put("/bots/<id>", format = "json", data = "<watcher>")]
fn update_bot(id: String, watcher: Json<Watcher>) -> Status {
    let bot = get_bot(id.clone());
    if bot.is_some() {
        if id == watcher.name {
            let mut compose = Compose::get();
            compose.remove_service(&watcher.name).unwrap();
            compose.add_service(watcher.name.clone(), Service::from(watcher));
            compose.update();
            Status::Ok
        } else {
            Status::Conflict
        }
    } else {
        Status::NotFound
    }
}

#[post("/bots/actions/deploy")]
fn deploy() -> Result<status::Created<()>, status::BadRequest<Json<ErrorContainer>>> {
    let output = Command::new("docker")
        .arg("-H")
        .arg("unix:///var/run/docker.sock")
        .arg("stack")
        .arg("deploy")
        .arg("-c")
        .arg("docker-compose.yml")
        .arg("wanaplay")
        .output()
        .expect("failed to execute process");
    match output.status.success() {
        true => Ok(status::Created("/bots".to_string(), None)),
        false => Err(status::BadRequest(Some(Json(ErrorContainer::new(vec![
            String::from_utf8(output.stderr).expect("Not UTF-8"),
        ]))))),
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Booking {
    id: String,
    date: String,
    court_time: String,
    court_number: u8,
}

fn get_bookings() -> Vec<Booking> {
    let client = get_logged_client().unwrap();
    let response = client
        .get(wanaplay_route("plannings/espacesportifpontoise").as_str()).send().unwrap();
    let document = Document::from_read(response).unwrap();
    document
        .find(Class("lienMyRes"))
        .map(|resa| {
            let re = Regex::new(r"(.+)\u{a0}(.+)\u{a0}Court (\d)").unwrap();
            let resa_line = resa.children().next().unwrap().text();
            let matches = re.captures(resa_line.as_str()).unwrap();
            Booking {
                id: resa.attr("href").unwrap().rsplit("/").collect::<Vec<_>>()[0].into(),
                date: matches.get(1).unwrap().as_str().into(),
                court_time: matches.get(2).unwrap().as_str().into(),
                court_number: matches.get(3).unwrap().as_str().parse().unwrap(),
            }
        }).collect::<Vec<_>>()
}

#[get("/bookings")]
fn get_all_bookings() -> Json<Vec<Booking>> {
    let bookings = get_bookings();
    Json(bookings)
}

#[delete("/bookings/<id>")]
fn remove_booking(id: String) -> Status {
    let client = get_logged_client().unwrap();
    let exists = get_bookings().iter().find(|booking| booking.id == id).is_some();
    if exists {
        client.get(wanaplay_route(format!("reservation/modifyReservationBase?idTspl={}&user_action=delete", id).as_str()).as_str()).send().unwrap();
        match get_bookings().iter().find(|booking| booking.id == id) {
            Some(_) => Status::BadRequest,
            None => Status::NoContent,
        }
    }
    else {
        Status::NotFound
    }
}

fn main() {
    for env_var in vec!["compose_file_path", "wanaplay_login", "wanaplay_password"] {
        if env::var(env_var).is_err() {
            println!("environment variable {} should be set", env_var);
            std::process::exit(1);
        }
    }
    rocket::ignite()
        .mount(
            "/",
            routes![
                get_all_bots,
                get_bot,
                new_bot,
                remove_bot,
                deploy,
                update_bot,
                get_all_bookings,
                remove_booking,
            ],
        )
        .launch();
}