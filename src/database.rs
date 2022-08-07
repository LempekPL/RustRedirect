use mongodb::{Client, Database};
use mongodb::error::ErrorKind;
use mongodb::options::ClientOptions;
use rocket::Config;
use rocket::tokio::join;
use serde::{Serialize, Deserialize};
use crate::{AUTH_COLLECTION, DATABASE_NAME, DOMAINS_COLLECTION};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Domain {
    pub(crate) name: String,
    pub(crate) domain: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Auth {
    pub(crate) name: String,
    pub(crate) token: String,
    pub(crate) permission: Permission,
}

#[derive(Deserialize)]
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

pub(crate) async fn connect() -> Database {
    let conf = match Config::figment().extract::<MoConfig>() {
        Ok(conf) => conf,
        Err(_) => {
            println!("Database config not found. Using default values");
            MoConfig::default()
        }
    };
    let client = match connect_to_database(conf).await {
        Ok(c) => c,
        Err(e) => {
            panic!("Could not connect to the database: {:?}", *e.kind);
        }
    };
    client.database(DATABASE_NAME)
}

pub(crate) async fn manage_database() {
    let db = connect().await;

    let create_domains = create_collection_unless(&db, DOMAINS_COLLECTION);
    let create_auths = create_collection_unless(&db, AUTH_COLLECTION);
    join!(create_domains, create_auths);

    // add default auth if not found any
    let a_col = db.collection::<Auth>(AUTH_COLLECTION);
    if let Ok(count) = a_col.count_documents(None, None).await {
        if count == 0 {
            let h = bcrypt::hash("pass", bcrypt::DEFAULT_COST).expect("Could not hash");
            a_col.insert_one(Auth {
                name: "admin".to_string(),
                token: h,
                permission: Permission(1, 0, 0, 0, 0),
            }, None).await.expect("Could not create default user");
            println!("No auth found, created new auth");
        }
    }
}

async fn create_collection_unless(db: &Database, name: &str) {
    match db.create_collection(name, None).await {
        Ok(_) => println!("Created collection {}", name),
        Err(e) => {
            match *e.kind {
                ErrorKind::Command(c) if c.code == 48 => {
                    println!("Collection '{}' already exists", name)
                }
                _ => {
                    panic!("Could not create collection: {:?}", *e.kind)
                }
            }
        }
    }
}

// Permission(0, 0, 0, 0, 0)
// 0 - full admin
// 1 - edit(only name and password) own auth
// 2 - edit/delete/list other redirects
// 3 - create/edit/delete/list own redirects
// 4 - create random named redirects

#[derive(Deserialize, Serialize, Debug)]
pub(crate) struct Permission(u8, u8, u8, u8, u8);

impl Permission {
    pub(crate) fn to_u8(self) -> u8 {
        self.0 * 16 + self.1 * 8 + self.2 * 4 + self.3 * 2 + self.4
    }

    // can do anything they want
    pub(crate) fn can_admin(&self) -> bool {
        self.0 == 1
    }

    // can edit self auth (only name and password)
    pub(crate) fn can_self(&self) -> bool {
        self.1 == 1 || self.can_admin()
    }

    // can edit/delete other redirects
    pub(crate) fn can_other(&self) -> bool {
        self.2 == 1 || self.can_admin()
    }

    // can create/edit/delete/list own redirects
    pub(crate) fn can_own(&self) -> bool {
        self.3 == 1 || self.can_admin()
    }

    // can create random named redirects
    pub(crate) fn can_random(&self) -> bool {
        self.4 == 1 || self.can_admin()
    }

    pub(crate) fn from_u8(mut num: u8) -> Permission {
        let mut arr: [u8; 5] = [0; 5];
        for n in (0..5).rev() {
            let b = 2_u8.pow(n as u32);
            (arr[4 - n], num) = {
                let b = num % b;
                if b != num {
                    (1, b)
                } else {
                    (0, b)
                }
            }
        }
        Permission(arr[0], arr[1], arr[2], arr[3], arr[4])
    }

    pub(crate) fn from_arr(nums: [u8; 5]) -> Permission {
        Permission(nums[0], nums[1], nums[2], nums[3], nums[4])
    }
}