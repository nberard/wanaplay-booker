extern crate reqwest;
use reqwest::{header, RedirectPolicy};
pub type Error = failure::Error;
pub type Result<T> = std::result::Result<T, Error>;

#[macro_use]
extern crate failure;
use failure::bail;
extern crate crypto;
extern crate regex;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use regex::Regex;
use select::document::Document;
use select::predicate::Class;
use std::env;
#[macro_use]
extern crate serde_derive;
use chrono::NaiveDate;
use std::result::Result as StdResult;

const WANAPLAY_END_POINT: &str = "http://fr.wanaplay.com/";
const WANAPLAY_DATE_FORMAT: &str = "%d/%m/%Y";

pub fn wanaplay_route(route: &str) -> String {
    format!("{}{}", WANAPLAY_END_POINT, route)
}

pub struct WanaplayCredentials {
    pub login: String,
    pub password: WanaplayPassword,
}

pub struct WanaplayPassword {
    pub secret_password: String,
}

impl WanaplayPassword {
    pub fn crypted(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.input_str(self.secret_password.as_str());
        hasher.result_str()
    }
}

pub fn get_credentials() -> Result<WanaplayCredentials> {
    match (env::var("wanaplay_login"), env::var("wanaplay_password")) {
        (Ok(login), Ok(password)) => Ok(WanaplayCredentials {
            login,
            password: WanaplayPassword {
                secret_password: password,
            },
        }),
        (_, _) => Err(format_err!(
            "environment variable wanaplay_login and wanaplay_password should be set"
        )),
    }
}

pub fn authenticate(login: String, crypted_password: String) -> Result<reqwest::Client> {
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

pub fn get_logged_client() -> Result<reqwest::Client> {
    let credentials = WanaplayCredentials {
        login: env::var("wanaplay_login").unwrap(),
        password: WanaplayPassword {
            secret_password: env::var("wanaplay_password").unwrap(),
        },
    };
    authenticate(credentials.login, credentials.password.crypted())
}

pub fn ser_from_naive_date<S>(date: &NaiveDate, serializer: S) -> StdResult<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let s = format!("{}", date.format(WANAPLAY_DATE_FORMAT));
    serializer.serialize_str(&s)
}

#[derive(Debug, Serialize, Clone)]
pub struct Booking {
    pub id: String,
    #[serde(serialize_with = "ser_from_naive_date")]
    pub date: NaiveDate,
    pub court_time: String,
    pub court_number: u8,
}

pub fn get_bookings() -> Vec<Booking> {
    let client = get_logged_client().unwrap();
    let response = client
        .get(wanaplay_route("plannings/espacesportifpontoise").as_str())
        .send()
        .unwrap();
    let document = Document::from_read(response).unwrap();
    document
        .find(Class("lienMyRes"))
        .map(|resa| {
            let re = Regex::new(r"(.+)\u{a0}(.+)\u{a0}Court (\d)").unwrap();
            let resa_line = resa.children().next().unwrap().text();
            let matches = re.captures(resa_line.as_str()).unwrap();
            Booking {
                id: resa.attr("href").unwrap().rsplit("/").collect::<Vec<_>>()[0].into(),
                date: NaiveDate::parse_from_str(
                    matches.get(1).unwrap().as_str(),
                    WANAPLAY_DATE_FORMAT,
                )
                .unwrap(),
                court_time: matches.get(2).unwrap().as_str().into(),
                court_number: matches.get(3).unwrap().as_str().parse().unwrap(),
            }
        })
        .collect::<Vec<_>>()
}
