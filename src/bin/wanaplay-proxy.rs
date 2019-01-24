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
use std::result::Result;
use regex::Regex;
use serde_yaml::from_reader;
use std::collections::BTreeMap;
use std::env;
use std::process::Command;
use rocket_contrib::json::Json;
use rocket::response::status;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Service {
    image: String,
    environment: Vec<String>,
    pub command: String,
}

impl From<Json<Watcher>> for Service {
    fn from(watcher: Json<Watcher>) -> Self {
        Service {
            image: "wanaplay_booker".to_string(),
            environment: vec![
                format!("wanaplay_login={}", env::var("wanaplay_login").unwrap()),
                format!("wanaplay_password={}", env::var("wanaplay_password").unwrap()),
            ],
            command: format!("wanaplay-booker -c {}:00 -w {}", watcher.court_time, watcher.week_day),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Compose {
    version: String,
    services: BTreeMap<String, Service>,
}

impl Compose {
    pub fn get() -> Self {
        from_reader(std::fs::File::open(env::var("compose_file_path").unwrap()).unwrap()).unwrap()
    }

    pub fn update(&self) {
        let serialized_report = serde_yaml::to_string(&self).unwrap();
        std::fs::write(env::var("compose_file_path").unwrap(), serialized_report).unwrap();
    }

    pub fn add_service(&mut self, name: String, service: Service) {
        self.services.insert(name, service);
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
enum WatcherStatus {
    Created,
    Running,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ErrorContainer {
    pub errors: Vec<String>
}

impl ErrorContainer {
    pub fn new(errors: Vec<String>) -> Self {
        ErrorContainer {
            errors
        }
    }
}


#[derive(Debug, Deserialize, Serialize, Clone)]
struct Watcher {
    name: String,
    status: WatcherStatus,
    court_time: String,
    week_day: String,
}

impl From<Service> for Watcher {
    fn from(service: Service) -> Self {
        let re = Regex::new(r"wanaplay-booker -c (\d{2}:\d{2}):\d{2} -w (\w+)").unwrap();
        let matches = re.captures(&service.command).unwrap();
        Watcher {
            name: "".to_string(),
            status: WatcherStatus::Created,
            court_time: matches.get(1).unwrap().as_str().to_string(),
            week_day: matches.get(2).unwrap().as_str().to_string(),
        }
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
        .mount("/", routes![get_all_bots, get_bot, new_bot])
        .launch();
}

fn get_bots() -> Vec<Watcher> {
    let compose = Compose::get();
    let bots: Vec<Watcher> = compose
        .services
        .into_iter()
        .map(|(name, elt)| {
            let output = Command::new("docker-compose")
                .arg("ps")
                .arg("-q")
                .arg(name.clone())
                .output()
                .expect("failed to execute process");
            let mut watcher = Watcher::from(elt);
            watcher.name = name;
            if !output.stdout.is_empty() {
                watcher.status = WatcherStatus::Running;
            }
            watcher
        })
        .collect();
    dbg!(bots.clone());
    bots
}

#[get("/bots")]
fn get_all_bots() -> Json<Vec<Watcher>> {
    Json(get_bots())
}

#[get("/bots/<id>")]
fn get_bot(id: String) -> Option<Json<Watcher>> {
    get_bots().into_iter().find(|bot| bot.name == id).map(|bot| Json(bot))
}

#[post("/bots", format = "json", data = "<watcher>")]
fn new_bot(watcher: Json<Watcher>) -> Result<status::Created<Json<Watcher>>, status::BadRequest<Json<ErrorContainer>>> {
    match get_bot(watcher.name.clone()) {
        Some(_) => Err(status::BadRequest(Some(Json(ErrorContainer::new(vec!["watcher already exists".to_string()]))))),
        None => {
            let mut compose = Compose::get();
            let watcher_result = watcher.clone();
            compose.add_service(watcher.name.clone(), Service::from(watcher));
            compose.update();
            Ok(status::Created(format!("/bots/{}", watcher_result.name.clone()), Some(Json(watcher_result))))
        },
    }
}

