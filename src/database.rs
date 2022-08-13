use std::process;
use mongodb::{Client, Database};
use mongodb::bson::oid::ObjectId;
use mongodb::error::ErrorKind;
use mongodb::options::ClientOptions;
use rocket::Config;
use rocket::tokio::join;
use serde::{Serialize, Deserialize};
use crate::{AUTH_COLLECTION, DATABASE_NAME, DOMAINS_COLLECTION};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Domain {
    pub(crate) _id: ObjectId,
    pub(crate) name: String,
    pub(crate) domain: String,
    pub(crate) owner: ObjectId,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Auth {
    pub(crate) _id: ObjectId,
    pub(crate) name: String,
    pub(crate) token: String,
    pub(crate) permission: Permission,
}

#[derive(Deserialize, Clone)]
struct MoConfig {
    db_host: String,
    db_port: u16,
    db_user: String,
    db_password: String,
}

impl Default for MoConfig {
    fn default() -> Self {
        Self {
            db_host: "localhost".to_string(),
            db_port: 27017,
            db_user: "admin".to_string(),
            db_password: "".to_string(),
        }
    }
}

pub(crate) async fn connect() -> Database {
    let conf = match Config::figment().extract::<MoConfig>() {
        Ok(conf) => conf,
        Err(_) => {
            println!("Database config not found. Using default values");
            MoConfig::default()
        }
    };
    let client = re_conn(conf, 3).await;
    client.database(DATABASE_NAME)
}

#[async_recursion::async_recursion]
async fn re_conn(config: MoConfig, tries: u8) -> Client {
    match connect_to_database(config.clone()).await {
        Ok(c) => c,
        Err(e) => {
            if tries == 0 {
                println!("Could not connect to the database: {:?} \n\x1b[31mTerminating process\x1b[0m", *e.kind);
                process::exit(1);
            }
            println!("Could not connect to the database: {:?} \x1b[34m(Remaining tries: {})\x1b[0m", *e.kind, tries);
            re_conn(config, tries - 1).await
        }
    }
}

async fn connect_to_database(config: MoConfig) -> mongodb::error::Result<Client> {
    // TODO: ability to use url
    let mut client_options = ClientOptions::parse(
        format!("mongodb://{}:{}@{}:{}/",
                config.db_user, config.db_password, config.db_host, config.db_port)
    ).await?;
    client_options.app_name = Some("RustRedirect".to_string());
    let client = Client::with_options(client_options)?;
    Ok(client)
}

#[async_recursion::async_recursion]
async fn create_collection_unless(db: &Database, name: &str, tries: u8) {
    match db.create_collection(name, None).await {
        Ok(_) => println!("Created collection {}", name),
        Err(e) => {
            match *e.kind {
                ErrorKind::Command(c) if c.code == 48 => {
                    println!("Collection '{}' already exists", name)
                }
                _ => {
                    if tries == 0 {
                        println!("Could not create collection: {:?} \n\x1b[31mTerminating process\x1b[0m", *e.kind);
                        process::exit(1);
                    }
                    println!("Could not create collection: {:?} \x1b[34m(Remaining tries: {})\x1b[0m", *e.kind, tries);
                    create_collection_unless(db, name, tries - 1).await;
                }
            }
        }
    }
}

pub(crate) async fn manage_database() {
    let db = connect().await;

    let create_domains = create_collection_unless(&db, DOMAINS_COLLECTION, 3);
    let create_auths = create_collection_unless(&db, AUTH_COLLECTION, 3);
    join!(create_domains, create_auths);

    // add default auth if not found any
    let a_col = db.collection::<Auth>(AUTH_COLLECTION);
    if let Ok(count) = a_col.count_documents(None, None).await {
        if count == 0 {
            let h = bcrypt::hash("pass", bcrypt::DEFAULT_COST).expect("Could not hash");
            a_col.insert_one(Auth {
                _id: Default::default(),
                name: "admin".to_string(),
                token: h,
                permission: Permission(1, 0, 0, 0, 0, 0),
            }, None).await.expect("Could not create default user");
            println!("No auth found, created new auth");
        }
    }
}

// Permission(0, 0, 0, 0, 0)
// 0 - full admin
// 1 - add/remove/edit auths lower than this and list all auths except admin
// 2 - edit/delete all redirects
// 3 - list all redirects
// 4 - create/edit/delete/list own redirects
// 5 - create random named redirects

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Permission(u8, u8, u8, u8, u8, u8);

impl Permission {
    // can do anything they want
    pub(crate) fn can_admin(&self) -> bool {
        self.0 == 1
    }

    // can add/remove/edit auths lower than admin and list all auths except admin
    pub(crate) fn can_manage(&self) -> bool {
        self.1 == 1 || self.can_admin()
    }

    // can edit/delete all redirects
    pub(crate) fn can_mod(&self) -> bool {
        self.2 == 1 || self.can_admin()
    }

    // can list all redirects
    pub(crate) fn can_list(&self) -> bool {
        self.3 == 1 || self.can_admin()
    }

    // can create/edit/delete/list own redirects
    pub(crate) fn can_own(&self) -> bool {
        self.4 == 1 || self.can_admin()
    }

    // can create random named redirects
    pub(crate) fn can_random(&self) -> bool {
        self.4 == 1 || self.can_admin()
    }

    pub(crate) fn from_arr(nums: [u8; 6]) -> Permission {
        Permission(nums[0], nums[1], nums[2], nums[3], nums[4], nums[5])
    }
}