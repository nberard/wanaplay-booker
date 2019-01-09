extern crate reqwest;
use reqwest::{header, RedirectPolicy};
extern crate crypto;
extern crate env_logger;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
extern crate structopt;
use structopt::StructOpt;
extern crate chrono;
use chrono::prelude::*;
use chrono::Duration;
use chrono::NaiveTime;
use chrono::Weekday;
use std::env;
use std::thread;
use std::time;
extern crate select;
#[macro_use]
extern crate failure;
use failure::bail;
use select::document::Document;
use select::predicate::{Attr, Class};
pub type Error = failure::Error;
pub type Result<T> = std::result::Result<T, Error>;

const WANAPLAY_END_POINT: &str = "http://fr.wanaplay.com/";

struct WanaplayPassword {
    secret_password: String,
}

impl WanaplayPassword {
    pub fn crypted(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.input_str(self.secret_password.as_str());
        hasher.result_str()
    }
}

#[derive(Debug)]
struct UserInfos {
    id: String,
    name: String,
}

struct WanaplayCredentials {
    login: String,
    password: WanaplayPassword,
}

struct Parameters {
    weekday: Weekday,
    court_time: NaiveTime,
    wanaplay_credentials: WanaplayCredentials,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "wanaplay-booker", about = " Book a court when available")]
struct Opt {
    /// week day to book
    #[structopt(short = "w", long = "weekday")]
    weekday: String,
    /// court time
    #[structopt(short = "c", long = "court_time")]
    court_time: NaiveTime,
}

fn validate_args(opt: &mut Opt) -> Result<Parameters> {
    let mut valid_times = vec![];
    let mut start_time = NaiveTime::from_hms(9, 0, 0);
    let end_time = NaiveTime::from_hms(23, 0, 0);
    while start_time <= end_time {
        valid_times.push(start_time.clone());
        start_time = match start_time.overflowing_add_signed(Duration::minutes(40)) {
            (val, _) => val,
        };
    }
    if !valid_times.contains(&opt.court_time) {
        bail!(format!(
            "{} is not a valid court time, should be one of {:?}",
            opt.court_time, valid_times
        ));
    }
    let weekday = match opt.weekday.parse::<Weekday>() {
        Ok(v) => v,
        Err(_) => bail!(format!("{} is not a valid week day", opt.weekday)),
    };
    match (env::var("wanaplay_login"), env::var("wanaplay_password")) {
        (Ok(login), Ok(password)) => Ok(Parameters {
            weekday,
            court_time: opt.court_time,
            wanaplay_credentials: WanaplayCredentials {
                login,
                password: WanaplayPassword {
                    secret_password: password,
                },
            },
        }),
        (_, _) => Err(format_err!("environment variable wanaplay_login and wanaplay_password should be set")),
    }
}

fn wanaplay_route(route: &str) -> String {
    format!("{}{}", WANAPLAY_END_POINT, route)
}

fn is_openned(client: &reqwest::Client, target_date: NaiveDate) -> bool {
    let forbidden = "Vous ne pouvez pas voir le planning";
    println!("watch_openning {:?} at {:?}", target_date, Local::now());
    let mut response = client
        .post(wanaplay_route("reservation/planning2").as_str())
        .form(&[("date", target_date.format("%Y-%m-%d").to_string())])
        .send()
        .unwrap();
    !response.text().unwrap().contains(forbidden)
}

fn get_user_infos(client: &reqwest::Client, reservation_id: &String) -> UserInfos {
    let response = client
        .post(wanaplay_route("reservation/takeReservationShow").as_str())
        .form(&[("idTspl", reservation_id)])
        .send()
        .unwrap();

    let document = Document::from_read(response).unwrap();
    let infos = document
        .find(Attr("id", "users_0"))
        .next()
        .unwrap()
        .children()
        .next()
        .unwrap();
    UserInfos {
        id: infos.attr("value").unwrap().to_string(),
        name: infos.text(),
    }
}

fn find_book_ids(
    client: &reqwest::Client,
    target_date: NaiveDate,
    court_time: NaiveTime,
) -> Vec<String> {
    println!("finding ids for {:?} at {:?}", target_date, court_time);
    let response = client
        .post(wanaplay_route("reservation/planning2").as_str())
        .form(&[("date", target_date.format("%Y-%m-%d").to_string())])
        .send()
        .unwrap();

    let document = Document::from_read(response).unwrap();
    let ids = document
        .find(Class("creneauLibre"))
        .filter(|node| {
            node.children()
                .next()
                .unwrap()
                .children()
                .next()
                .unwrap()
                .text()
                == court_time.format("%H:%M").to_string()
        })
        .filter(|node| node.attr("class").unwrap() == "creneauLibre")
        .map(|node| node.attr("onclick").unwrap())
        .map(|link| link.split("idTspl=").collect::<Vec<_>>()[1].replace("\"", ""))
        .collect::<Vec<_>>();
    println!("{:?}", ids);
    ids
}

fn authenticate(login: String, crypted_password: String) -> Result<reqwest::Client> {
    let authent_client = reqwest::Client::builder()
        .redirect(RedirectPolicy::none())
        .build()
        .unwrap();
    let authent_response = authent_client
        .post(wanaplay_route("auth/doLogin").as_str())
        .form(&[("login", login), ("sha1mdp", crypted_password)])
        .send()
        .unwrap();
    let location = authent_response.headers().get(header::LOCATION);
    if location.is_none()
        || location.unwrap().to_str().unwrap() != wanaplay_route("auth/infos").as_str()
    {
        bail!("unable to login");
    }
    let session_cookie = authent_response.headers().get(header::SET_COOKIE).unwrap();
    let mut headers = header::HeaderMap::new();
    headers.insert(header::COOKIE, session_cookie.clone());
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    // useless request but mandatory :/
    client
        .post(wanaplay_route("reservation/planning2").as_str())
        .form(&[("date", "2018-12-24")])
        .send()
        .unwrap();
    Ok(client)
}

fn book(client: &reqwest::Client, user_infos: &UserInfos, id_booking: &String, date: &NaiveDate) {
    println!("book");
    println!("{:?}", id_booking);
    client
        .post(wanaplay_route("reservation/takeReservationBase").as_str())
        .form(&[
            ("date", date.format("%Y-%m-%d").to_string()),
            ("idTspl", id_booking.to_string()),
            ("commit", "Confirmer".to_string()),
            ("nb_participants", "1".to_string()),
            ("tab_users_id_0", user_infos.id.clone()),
            ("tab_users_name_0", user_infos.name.clone()),
        ])
        .send()
        .unwrap();
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

fn run() -> Result<()> {
    let mut opt = Opt::from_args();
    let parameters = validate_args(&mut opt)?;
    //    let target_date = NaiveDate::from_ymd(2019, 1, 21);
    //    let openned = is_openned(&client, target_date);
    //    println!("openned = {:?}", openned);
    //    panic!("plop");
    //    let ids = find_book_ids(&client, target_date, parameters.court_time);
    //    if !ids.is_empty() {
    //        let id = ids.into_iter().next().unwrap();
    //        let user_infos = get_user_infos(&client, &id);
    //        book(&client, &user_infos, &id, &target_date);
    //    }
    loop {
        let now: DateTime<Local> = if env::var("fake_date").is_ok() {
            env::var("fake_date")
                .unwrap()
                .parse::<DateTime<Local>>()
                .unwrap()
        } else {
            Local::now()
        };
        println!("loop {:?}", now);
        let client = authenticate(
            parameters.wanaplay_credentials.login.clone(),
            parameters.wanaplay_credentials.password.crypted(),
        )?;
        if now.weekday() == parameters.weekday.pred() {
            let target_date = now + Duration::days(15);
            let target_date =
                NaiveDate::from_ymd(target_date.year(), target_date.month(), target_date.day());
            println!("target_date = {:?}", target_date);
            if now.hour() == 23 {
                if now.minute() >= 58 {
                    while !is_openned(&client, target_date) {
                        thread::sleep(time::Duration::from_secs(2));
                    }
                    let ids = find_book_ids(&client, target_date, parameters.court_time);
                    if !ids.is_empty() {
                        let id = ids.into_iter().next().unwrap();
                        let user_infos = get_user_infos(&client, &id);
                        book(&client, &user_infos, &id, &target_date);
                    }
                } else {
                    println!("sleep for 1 min");
                    thread::sleep(time::Duration::from_secs(60));
                }
            } else {
                println!("sleep for 50 min");
                thread::sleep(time::Duration::from_secs(50 * 60));
            }
        } else {
            println!("sleep for 23h");
            thread::sleep(time::Duration::from_secs(23 * 60 * 60));
        }
    }
}
