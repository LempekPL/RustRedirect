use mongodb::{
    Collection,
    bson::{doc, Document}
};
use rand::{
    SeedableRng,
    distributions::{Alphanumeric, DistString}
};
use rocket::{
    Build,
    Request,
    request,
    Rocket,
    futures::TryStreamExt,
    http::Status,
    outcome::Outcome,
    request::FromRequest,
    serde::json::Json
};
use serde::Serialize;
use serde_json::Value;
use crate::{AUTH_COLLECTION, DOMAINS_COLLECTION, Domain, connect, some_return, ok_return, add_and};
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
            random_redirect,
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
            create_auth,
            edit_auth,
            delete_auth,
            i_create_post,
            i_edit_put,
            i_delete_delete,
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
async fn random_redirect(domain: Option<String>, auth: Auth) -> Json<Response> {
    let domain = some_return!(domain, Response::USER_DID_NOT_PROVIDE_PARAM("domain").json());
    let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    if auth.permission.can_random() {
        let name = match get_check_random(&db, Alphanumeric.sample_string(&mut rand::rngs::SmallRng::from_entropy(), 8), 3).await {
            Ok(o) => o,
            Err(e) => return e
        };
        let res = db.insert_one(
            Domain {
                _id: Default::default(),
                name: name.clone(),
                domain: domain.clone(),
                owner: auth._id,
            }, None).await;
        return match res {
            Ok(_) => Response::new(true, &format!("Created random redirect to '{}' named '{}'.", domain, name)).json(),
            Err(_) => Response::COULD_NOT("create", "random redirect").json()
        };
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    }
}

#[async_recursion::async_recursion]
async fn get_check_random(db: &Collection<Domain>, name: String, tries: u32) -> Result<String, Json<Response>> {
    if tries == 0 {
        return Err(Response::COULD_NOT("create", "random redirect").json());
    }
    let dom = ok_return!(db.find_one(doc! { "name": name.clone() }, None).await, Err(Response::DATABASE_WHILST_TRYING_TO_FIND().json()));
    return match dom {
        Some(_) => get_check_random(db, Alphanumeric.sample_string(&mut rand::rngs::SmallRng::from_entropy(), 8), tries - 1).await,
        None => Ok(name)
    }
}

#[post("/create?<name>&<domain>")]
async fn create_redirect(name: Option<String>, domain: Option<String>, auth: Auth) -> Json<Response> {
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let domain = some_return!(domain, Response::USER_DID_NOT_PROVIDE_PARAM("domain").json());

    if auth.permission.can_own() {
        let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
        let dom: Option<Domain> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if dom.is_some() {
            return Response::EXIST("Redirect", "already").json();
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
    if let Some(newname) = newname.clone() {
        let existing_domain: Option<Domain> = ok_return!(db.find_one(doc! { "name" : newname }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if existing_domain.is_some() {
            return Response::EXIST("Domain with the new name", "already").json();
        }
    }
    let dom: Option<Domain> = ok_return!(db.find_one(search_name, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    let dom = match dom {
        None => return Response::EXIST("Redirect", "doesn't").json(),
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
            if newname.is_none() && domain.is_none() {
                return Response::NOTHING_CHANGED().json();
            }
            let mut str = "".to_string();
            if let Some(newname) = newname {
                str += &format!("name '{}' -> '{}'", name, newname);
            }
            if let Some(domain) = domain {
                add_and!(str);
                str += &format!("permission '{}' -> '{}'", dom.domain, domain);
            }
            return Response::new(true, &format!("Edited redirect, {}", str)).json();
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
    match dom {
        Some(dom) => {
            let res = db.delete_one(doc! { "_id": dom._id }, None).await;
            match res {
                Ok(r) if r.deleted_count > 0 => Response::new(true, &format!("Deleted redirect named '{}'", name)).json(),
                Ok(_) => Response::NOTHING_DELETED().json(),
                Err(_) => Response::COULD_NOT("delete", "redirect").json()
            }
        },
        None => Response::COULD_NOT("find", "redirect").json()
    }
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

#[post("/create?<name>&<password>&<permission>")]
async fn create_auth(name: Option<String>, password: Option<String>, permission: Option<u8>, auth: Auth) -> Json<Response> {
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let password = some_return!(password, Response::USER_DID_NOT_PROVIDE_PARAM("password").json());
    let permission = match permission {
        None => Permission::default(),
        Some(p) => Permission::from_u8(p)
    };
    return if auth.permission.can_admin() || (auth.permission.can_manage() && !permission.can_manage()) {
        let db = connect().await.collection::<Auth>(AUTH_COLLECTION);
        let existing_auth: Option<Auth> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if existing_auth.is_some() {
            return Response::EXIST("Auth with that name", "already").json();
        }
        let hashed = ok_return!(bcrypt::hash(password, bcrypt::DEFAULT_COST), Response::COULD_NOT("encrypt", "password").json());
        let res = db.insert_one(
            Auth {
                _id: Default::default(),
                name: name.clone(),
                password: hashed,
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

#[put("/edit?<name>&<newname>&<password>&<permission>")]
async fn edit_auth(name: Option<String>, newname: Option<String>, password: Option<String>, permission: Option<u8>, auth: Auth) -> Json<Response> {
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let permission = match permission {
        None => None,
        Some(p) => Some(Permission::from_u8(p))
    };

    return if auth.permission.can_admin() || (auth.permission.can_manage() && (permission.is_none() || !permission.clone().unwrap().can_manage())) {
        let db = connect().await.collection::<Auth>(AUTH_COLLECTION);
        if let Some(newname) = newname.clone() {
            let existing_auth: Option<Auth> = ok_return!(db.find_one(doc! { "name" : newname }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
            if existing_auth.is_some() {
                return Response::EXIST("Auth with the new name", "already").json();
            }
        }
        let old_auth: Option<Auth> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        let old_auth: Auth = some_return!(old_auth, Response::EXIST("Auth", "doesn't").json());
        if !auth.permission.can_admin() && old_auth.permission.can_manage() {
            return Response::PERMISSIONS_TOO_LOW().json();
        }
        let hashed;
        match password.clone() {
            None => hashed = None,
            Some(p) => hashed = Some(ok_return!(bcrypt::hash(p, bcrypt::DEFAULT_COST), Response::COULD_NOT("encrypt", "password").json()))
        }
        let res = db
            .update_one(
                doc! { "_id" : old_auth._id },
                doc! {
                    "$set": {
                        "name": newname.clone().unwrap_or(old_auth.name.clone()),
                        "password": hashed.clone().unwrap_or(old_auth.password.clone()),
                        "permission": permission.unwrap_or(old_auth.permission)
                    }
                },
                None)
            .await;
        match res {
            Ok(m) if m.modified_count > 0 => {
                if newname.is_none() && password.is_none() && permission.is_none() {
                    return Response::NOTHING_CHANGED().json();
                }
                let mut str = "".to_string();
                if let Some(newname) = newname {
                    str += &format!("name '{}' -> '{}'", old_auth.name, newname);
                }
                if password.is_some() {
                    add_and!(str);
                    str += &format!("password changed");
                }
                if let Some(permission) = permission {
                    add_and!(str);
                    str += &format!("permission '{}' -> '{}'", old_auth.permission, permission);
                }
                return Response::new(true, &format!("Edited auth, {}", str)).json();
            }
            Ok(_) => Response::NOTHING_CHANGED().json(),
            Err(_) => Response::COULD_NOT("edit", "redirect").json()
        }
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    };
}

#[delete("/delete?<name>")]
async fn delete_auth(name: Option<String>, auth: Auth) -> Json<Response> {
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let db = connect().await.collection::<Auth>(AUTH_COLLECTION);
    let del_auth = ok_return!(db.find_one(doc! { "name": name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    match del_auth {
        Some(del_auth) => {
            if auth.permission.can_admin() || (auth.permission.can_manage() && !del_auth.permission.can_manage()) {
                let res = db.delete_one(doc! { "_id": del_auth._id }, None).await;
                match res {
                    Ok(r) if r.deleted_count > 0 => Response::new(true, &format!("Deleted auth named '{}'", name)).json(),
                    Ok(_) => Response::NOTHING_DELETED().json(),
                    Err(_) => Response::COULD_NOT("delete", "auth").json()
                }
            } else {
                return Response::PERMISSIONS_TOO_LOW().json()
            }
        },
        None => Response::COULD_NOT("find", "redirect").json()
    }
}


//////////
// AUTH
//////////

#[rocket::async_trait]
impl<'a> FromRequest<'a> for Auth {
    type Error = AuthError;

    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        let name: &str = some_return!(request.headers().get_one("name"), Outcome::Failure((Status::BadRequest, AuthError::MissingName)));
        let password: &str = some_return!(request.headers().get_one("password"), Outcome::Failure((Status::BadRequest, AuthError::MissingPassword)));
        // connect to database and select auth collection
        let col = connect().await.collection::<Auth>(AUTH_COLLECTION);
        let found: Option<Auth> = ok_return!(col.find_one(doc! {"name": name}, None).await, Outcome::Failure((Status::InternalServerError, AuthError::ServerError)));
        let auth: Auth = some_return!(found, Outcome::Failure((Status::Unauthorized, AuthError::NotFound)));
        let ver: bool = ok_return!(bcrypt::verify(password, &auth.password), Outcome::Failure((Status::Unauthorized, AuthError::Unknown)));
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
    const EXIST: fn(&str, &str) -> Response = |thing: &str, action: &str| Response::new(false, &format!("{} {} exist.", thing, action));
    const COULD_NOT: fn(&str, &str) -> Response = |action: &str, thing: &str| Response::new(false, &format!("Could not {} {}.", action, thing));
    const NOTHING_CHANGED: fn() -> Response = || Response::new(false, "Nothing changed.");
    const NOTHING_DELETED: fn() -> Response = || Response::new(false, "Nothing deleted.");
}

/////////////
// ERRORS
/////////////

#[derive(Debug)]
pub(crate) enum AuthError {
    MissingPassword,
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