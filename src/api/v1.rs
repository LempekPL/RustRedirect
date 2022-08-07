use std::fmt::Debug;
use bcrypt::verify;
use mongodb::bson::doc;
use mongodb::Cursor;
use rocket::{Build, Request, request, Rocket};
use rocket::futures::TryStreamExt;
use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::FromRequest;
use serde::Serialize;
use rocket::serde::json::Json;
use serde_json::Value;
use crate::{AUTH_COLLECTION, DOMAINS_COLLECTION, Domain, connect, some_return, ok_return};
use crate::database::Auth;

#[derive(Serialize)]
struct Response {
    success: bool,
    response: Value,
}

pub(crate) fn mount_v1(rocket: Rocket<Build>) -> Rocket<Build> {
    let rocket = rocket.mount(
        "/api/v1/redirect",
        routes![
            check_domains,
            create_redirect,
            edit_redirect,
            remove_redirect,
            random_redirect,
            i_create_post,
            i_edit_put,
            i_delete_delete,
            i_random_post,
            ],
    );
    // .mount("/api/v1/auth",
    //        routes![
    //     TODO: add_auth, edit_auth, delete_auth,
    //  ],
    // )
    rocket
}

#[get("/")]
async fn check_domains() -> Json<Response> {
    let col = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let cursor: Cursor<Domain> = ok_return!(col.find(None, None).await, Json(Response::DATABASE_WHILST_TRYING_TO_FIND()));
    let collected: Vec<Domain> = ok_return!(cursor.try_collect().await, Json(Response::DATABASE_WHILST_TRYING_TO_COLLECT()));
    let collected: Value = ok_return!(serde_json::to_value(collected), Json(Response::SERVER_WHILST_TRYING_TO_FORMAT()));
    Json(Response {
        success: true,
        response: collected,
    })
}

#[post("/random?<domain>")]
fn random_redirect(domain: Option<String>, auth: Auth) -> Json<Response> {
    // if auth.permission.can_random() {
    //
    // }
    Json(Response {
        success: true,
        response: Value::String(format!("Created redirect to '' named ''. Made by using token named: ")),
    })
}

#[post("/create?<name>&<domain>")]
async fn create_redirect(name: Option<String>, domain: Option<String>, auth: Auth) -> Json<Response> {
    let name: String = some_return!(name, Json(Response::USER_DID_NOT_PROVIDE_PARAM("name")));
    let domain: String = some_return!(domain, Json(Response::USER_DID_NOT_PROVIDE_PARAM("domain")));

    if auth.permission.can_own() {
        let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
        let dom: Option<Domain> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Json(Response::DATABASE_WHILST_TRYING_TO_FIND()));
        if dom.is_some() {
            return Json(Response::REDIRECT_ALREADY_EXIST());
        }
        let res = db.insert_one(
            Domain {
                name: name.clone(),
                domain: domain.clone(),
                owner: auth._id
            }, None).await;
        return match res {
            Ok(_) => Json(Response::new(true, &format!("Created redirect to '{}' named '{}'. Using token named: {}", domain, name, auth.name))),
            Err(_) => Json(Response::COULD_NOT_CREATE_REDIRECT())
        };
    } else {
        Json(Response::PERMISSIONS_TOO_LOW())
    }
}

#[put("/edit?<name>&<domain>")]
fn edit_redirect(name: Option<String>, domain: Option<String>, auth: Auth) -> &'static str {
    "Hello, world!"
}

#[delete("/delete?<name>")]
fn remove_redirect(name: Option<String>, auth: Auth) -> &'static str {
    "Hello, world!"
}

#[rocket::async_trait]
impl<'a> FromRequest<'a> for Auth {
    type Error = AuthError;

    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        let name: &str = some_return!(request.headers().get_one("name"), Outcome::Failure((Status::BadRequest, AuthError::MissingName)));
        let token: &str = some_return!(request.headers().get_one("token"), Outcome::Failure((Status::BadRequest, AuthError::MissingToken)));
        // connect to database and select auth collection
        let col = connect().await.collection::<Auth>(AUTH_COLLECTION);
        let found: Option<Auth> = ok_return!(col.find_one(doc! {"name": name}, None).await, Outcome::Failure((Status::InternalServerError, AuthError::ServerError)));
        let auth: Auth = some_return!(found, Outcome::Failure((Status::Unauthorized, AuthError::NotFound)));
        let ver: bool = ok_return!(verify(token, &auth.token), Outcome::Failure((Status::Unauthorized, AuthError::Unknown)));
        return if ver {
            Outcome::Success(auth)
        } else {
            Outcome::Failure((Status::Unauthorized, AuthError::Invalid))
        };
    }
}

/////////////
// Responses
/////////////

impl Response {
    fn new(success: bool, response: &str) -> Self {
        Self {
            success,
            response: Value::String(response.to_string()),
        }
    }

    const DATABASE_WHILST_TRYING_TO_FIND: fn() -> Response = || Response::new(false, "Database error whilst trying to find.");
    const DATABASE_WHILST_TRYING_TO_COLLECT: fn() -> Response = || Response::new(false, "Database error whilst trying to collect data.");
    const SERVER_WHILST_TRYING_TO_FORMAT: fn() -> Response = || Response::new(false, "Server error whilst response formatting.");
    const USER_DID_NOT_PROVIDE_PARAM: fn(&str) -> Response = |param: &str| Response::new(false, &format!("User error, did not provide '{}' param", param));
    const PERMISSIONS_TOO_LOW: fn() -> Response = || Response::new(false, "Could not do that. Permissions too low.");
    const REDIRECT_ALREADY_EXIST: fn() -> Response = || Response::new(false, "Redirect with that name already exists");
    const COULD_NOT_CREATE_REDIRECT: fn() -> Response = || Response::new(false, "Could not create redirect.");
}


/////////////
// ERRORS
/////////////

#[derive(Debug)]
pub(crate) enum AuthError {
    MissingToken,
    MissingName,
    Invalid,
    ServerError,
    NotFound,
    Unknown,
}

//////////////////
// INFO PATHS
//////////////////

#[get("/create")]
fn i_create_post() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use post".to_string()),
    })
}

#[get("/edit")]
fn i_edit_put() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use put".to_string()),
    })
}

#[get("/delete")]
fn i_delete_delete() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use delete".to_string()),
    })
}

#[get("/random")]
fn i_random_post() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use post".to_string()),
    })
}