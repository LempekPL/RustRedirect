use mongodb::{Client, Database};
use mongodb::error::ErrorKind;
use mongodb::options::ClientOptions;
use rocket::Config;
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

    create_collection_unless(&db, DOMAINS_COLLECTION);
    create_collection_unless(&db, AUTH_COLLECTION);

    // add default auth if not found any
    let a_col = db.collection::<Auth>(AUTH_COLLECTION);
    if let Ok(count) = a_col.count_documents(None, None).await {
        if count == 0 {
            a_col.insert_one(Auth {
                name: "admin".to_string(),
                token: "pass".to_string()
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