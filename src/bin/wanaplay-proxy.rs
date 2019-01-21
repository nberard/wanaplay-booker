#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_yaml;
extern crate serde_json;

pub type Error = failure::Error;
pub type Result<T> = std::result::Result<T, Error>;
use serde_yaml::from_reader;
use std::collections::BTreeMap;
use rocket::response::content;
use chrono::NaiveTime;
use chrono::Weekday;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Service {
    image: String,
    environment: Option<Vec<String>>,
    command: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: String,
    services: BTreeMap<String, Service>,
}

struct Watcher {
    court_time: NaiveTime,
    week_day: Weekday,
}

impl From<Service> for Watcher {
    fn from(service: Service) -> Self {

    }
}

fn main() {
    env_logger::init();
    if let Err(err) = run() {
        for cause in err.iter_chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}

#[get("/watched_bookings")]
fn get_bookings() -> content::Json<String> {
    let f = std::fs::File::open("docker-compose.yml").unwrap();
    let compose: File = from_reader(f).unwrap();
    println!("Read YAML string: {:?}", compose);
//    let serialized_report = serde_yaml::to_string(&compose).unwrap();
//    std::fs::write("docker-compose.generated.yml", serialized_report).unwrap();
    let json = serde_json::to_string(&compose).unwrap();
    content::Json(json)
}

fn run() -> Result<()> {
    rocket::ignite().mount("/", routes![get_bookings]).launch();
    Ok(())
}
