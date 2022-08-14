use bcrypt::verify;
use mongodb::bson::{doc, Document};
use rocket::{Build, Request, request, Rocket};
use rocket::futures::TryStreamExt;
use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::FromRequest;
use serde::Serialize;
use rocket::serde::json::Json;
use serde_json::Value;
use crate::{AUTH_COLLECTION, DOMAINS_COLLECTION, Domain, connect, some_return, ok_return};
use crate::database::{Auth, Permission};

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
            // TODO: random_redirect,
            i_create_post,
            i_edit_put,
            i_delete_delete,
            i_random_post,
            ],
    );
    let rocket = rocket.mount(
        "/api/v1/auth",
        routes![
               list_auth,
                add_auth,
        // TODO:  edit_auth, delete_auth
     ],
    );
    rocket
}

////////////
// DOMAINS
////////////

#[get("/")]
async fn check_domains(auth: Auth) -> Json<Response> {
    let document;
    if auth.permission.can_list() {
        document = None;
    } else if auth.permission.can_own() {
        document = Some(doc! { "owner": auth._id });
    } else {
        return Response::PERMISSIONS_TOO_LOW().json();
    }
    let conn = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let cursor = ok_return!(conn.find(document, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    let collected: Vec<Domain> = ok_return!(cursor.try_collect().await, Response::DATABASE_WHILST_TRYING_TO_COLLECT().json());
    let collected = ok_return!(serde_json::to_value(collected), Response::SERVER_WHILST_TRYING_TO_FORMAT().json());
    Response {
        success: true,
        response: collected,
    }.json()
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
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let domain = some_return!(domain, Response::USER_DID_NOT_PROVIDE_PARAM("domain").json());

    if auth.permission.can_own() {
        let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
        let dom: Option<Domain> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if dom.is_some() {
            return Response::REDIRECT_ALREADY_EXIST().json();
        }
        let res = db.insert_one(
            Domain {
                _id: Default::default(),
                name: name.clone(),
                domain: domain.clone(),
                owner: auth._id,
            }, None).await;
        return match res {
            Ok(_) => Response::new(true, &format!("Created redirect to '{}' named '{}'.", domain, name)).json(),
            Err(_) => Response::COULD_NOT("create", "redirect").json()
        };
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    }
}

#[put("/edit?<name>&<newname>&<domain>")]
async fn edit_redirect(name: Option<String>, newname: Option<String>, domain: Option<String>, auth: Auth) -> Json<Response> {
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let search_name = match get_search(auth, &name) {
        Ok(o) => o,
        Err(e) => return e
    };

    let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let dom: Option<Domain> = ok_return!(db.find_one(search_name, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    let dom = match dom {
        None => return Response::REDIRECT_DOESNT_EXIST().json(),
        Some(d) => d
    };
    let res = db
        .update_one(
            doc! { "_id" : dom._id },
            doc! { "$set": { "name": newname.clone().unwrap_or(name.clone()), "domain": domain.clone().unwrap_or(dom.domain.clone()) } },
            None)
        .await;
    match res {
        Ok(m) if m.modified_count > 0 => {
            match (newname.clone(), domain.clone()) {
                (n, d) if n.is_some() && d.is_some() => {
                    Response::new(true, &format!("Edited redirect. Name '{}' -> '{}' and domain '{}' -> '{}'", name, newname.unwrap(), dom.domain, domain.unwrap())).json()
                }
                (n, d) if n.is_none() && d.is_some() => {
                    Response::new(true, &format!("Edited redirect. Domain '{}' -> '{}'", dom.domain, domain.unwrap())).json()
                }
                (n, d) if n.is_some() && d.is_none() => {
                    Response::new(true, &format!("Edited redirect. Name '{}' -> '{}'", name, newname.unwrap())).json()
                }
                _ => Response::NOTHING_CHANGED().json()
            }
        }
        Ok(_) => Response::NOTHING_CHANGED().json(),
        Err(_) => Response::COULD_NOT("edit", "redirect").json()
    }
}

#[delete("/delete?<name>")]
async fn remove_redirect(name: Option<String>, auth: Auth) -> Json<Response> {
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let search_name = match get_search(auth, &name) {
        Ok(o) => o,
        Err(e) => return e
    };

    let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let dom: Option<Domain> = ok_return!(db.find_one(search_name, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    return if dom.is_some() {
        let res = db.delete_one(doc! { "_id": dom.unwrap()._id }, None).await;
        match res {
            Ok(r) if r.deleted_count > 0 => Response::new(true, &format!("Deleted redirect named '{}'", name)).json(),
            Ok(_) => Response::NOTHING_DELETED().json(),
            Err(_) => Response::COULD_NOT("delete", "redirect").json()
        }
    } else {
        Response::COULD_NOT("find", "redirect").json()
    };
}

////////////
// AUTHS
////////////

#[get("/")]
async fn list_auth(auth: Auth) -> Json<Response> {
    let document;
    if auth.permission.can_admin() {
        document = None
    } else if auth.permission.can_manage() {
        document = Some(doc! { "permission.0": {"$ne": 1}})
    } else {
        return Response::PERMISSIONS_TOO_LOW().json();
    }
    let conn = connect().await.collection::<Auth>(AUTH_COLLECTION);
    let cursor = ok_return!(conn.find(document, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    let collected: Vec<Auth> = ok_return!(cursor.try_collect().await, Response::DATABASE_WHILST_TRYING_TO_COLLECT().json());
    let collected = ok_return!(serde_json::to_value(collected), Response::SERVER_WHILST_TRYING_TO_FORMAT().json());
    Response {
        success: true,
        response: collected,
    }.json()
}

#[post("/add?<name>&<password>&<permission>")]
async fn add_auth(name: Option<String>, password: Option<String>, permission: Option<u8>, auth: Auth) -> Json<Response> {
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let password = some_return!(password, Response::USER_DID_NOT_PROVIDE_PARAM("password").json());
    let permission = match permission {
        None => Permission::default(),
        Some(p) => Permission::from_u8(p)
    };
    dbg!(permission);
    return if auth.permission.can_admin() || (auth.permission.can_manage() && !permission.can_manage()) {
        let db = connect().await.collection::<Auth>(AUTH_COLLECTION);
        let auth: Option<Auth> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if auth.is_some() {
            return Response::AUTH_ALREADY_EXIST().json();
        }
        let hashed = ok_return!(bcrypt::hash(password, bcrypt::DEFAULT_COST), Response::COULD_NOT("encrypt", "password").json());
        let res = db.insert_one(
            Auth {
                _id: Default::default(),
                name: name.clone(),
                token: hashed,
                permission,
            }, None).await;
        match res {
            Ok(_) => Response::new(true, &format!("Created auth named '{}' with permission: {}.", name, permission)).json(),
            Err(_) => Response::COULD_NOT("create", "auth").json()
        }
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    };
}

//////////
// AUTH
//////////

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

//////////////
// RESPONSES
//////////////

impl Response {
    fn new(success: bool, response: &str) -> Self {
        Self {
            success,
            response: Value::String(response.to_string()),
        }
    }

    fn json(self) -> Json<Self> {
        Json(self)
    }

    const DATABASE_WHILST_TRYING_TO_FIND: fn() -> Response = || Response::new(false, "Database error whilst trying to find.");
    const DATABASE_WHILST_TRYING_TO_COLLECT: fn() -> Response = || Response::new(false, "Database error whilst trying to collect data.");
    const SERVER_WHILST_TRYING_TO_FORMAT: fn() -> Response = || Response::new(false, "Server error whilst response formatting.");
    const USER_DID_NOT_PROVIDE_PARAM: fn(&str) -> Response = |param: &str| Response::new(false, &format!("User error, did not provide '{}' param.", param));
    const PERMISSIONS_TOO_LOW: fn() -> Response = || Response::new(false, "Could not do that. Permissions too low.");
    const REDIRECT_ALREADY_EXIST: fn() -> Response = || Response::new(false, "Redirect with that name already exists.");
    const REDIRECT_DOESNT_EXIST: fn() -> Response = || Response::new(false, "Redirect doesn't exists.");
    const AUTH_ALREADY_EXIST: fn() -> Response = || Response::new(false, "Auth with that name already exists.");
    const AUTH_DOESNT_EXIST: fn() -> Response = || Response::new(false, "Auth doesn't exists.");
    const COULD_NOT: fn(&str, &str) -> Response = |action: &str, thing: &str| Response::new(false, &format!("Could not {} {}.", action, thing));
    const NOTHING_CHANGED: fn() -> Response = || Response::new(false, "Nothing changed.");
    const NOTHING_DELETED: fn() -> Response = || Response::new(false, "Nothing deleted.");
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

//////////
// OTHER
//////////

fn get_search(auth: Auth, name: &str) -> Result<Document, Json<Response>> {
    if auth.permission.can_mod() {
        Ok(doc! { "name": name.clone() })
    } else if auth.permission.can_own() {
        Ok(doc! { "name": name.clone(), "owner": auth._id })
    } else {
        Err(Response::PERMISSIONS_TOO_LOW().json())
    }
}