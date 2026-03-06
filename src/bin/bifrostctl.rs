/*
 * Bifrostctl: CLI de administración para Bifröst-Gate.
 */

#[path = "../db.rs"]
mod db;

use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    let pool = db::init_db().await;

    match args[1].as_str() {
        "apikey" => handle_apikey_commands(&pool, &args).await,
        "docs-user" => handle_docs_user_commands(&pool, &args).await,
        "help" | "--help" | "-h" => print_usage(),
        other => {
            eprintln!("Comando desconocido '{}'.", other);
            print_usage();
        }
    }
}

async fn handle_apikey_commands(pool: &sqlx::SqlitePool, args: &[String]) {
    if args.len() < 3 {
        print_usage();
        return;
    }

    match args[2].as_str() {
        "list" => {
            match db::list_api_keys(pool).await {
                Ok(keys) => {
                    if keys.is_empty() {
                        println!("No hay API keys registradas.");
                    } else {
                        println!("ID | USER | STATUS | CREATED_AT | API_KEY");
                        for rec in keys {
                            let status = if rec.is_active { "active" } else { "disabled" };
                            println!(
                                "{} | {} | {} | {} | {}",
                                rec.id, rec.user_name, status, rec.created_at, rec.api_key
                            );
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error listando API keys: {}", err);
                }
            }
        }
        "create" => {
            if args.len() < 4 {
                eprintln!("Falta user_name. Uso: bifrostctl apikey create <user_name>");
                return;
            }
            let user_name = args[3].trim();
            if user_name.is_empty() {
                eprintln!("user_name no puede estar vacio");
                return;
            }

            match db::create_api_key_for_user(pool, user_name).await {
                Ok(api_key) => println!("API key creada para '{}': {}", user_name, api_key),
                Err(err) => eprintln!("Error creando API key: {}", err),
            }
        }
        "enable" => {
            if args.len() < 4 {
                eprintln!("Falta api_key. Uso: bifrostctl apikey enable <api_key>");
                return;
            }
            let api_key = args[3].trim();
            match db::set_api_key_active(pool, api_key, true).await {
                Ok(0) => eprintln!("No se encontro la API key indicada."),
                Ok(_) => println!("API key habilitada."),
                Err(err) => eprintln!("Error habilitando API key: {}", err),
            }
        }
        "disable" => {
            if args.len() < 4 {
                eprintln!("Falta api_key. Uso: bifrostctl apikey disable <api_key>");
                return;
            }
            let api_key = args[3].trim();
            match db::set_api_key_active(pool, api_key, false).await {
                Ok(0) => eprintln!("No se encontro la API key indicada."),
                Ok(_) => println!("API key deshabilitada."),
                Err(err) => eprintln!("Error deshabilitando API key: {}", err),
            }
        }
        "delete" => {
            if args.len() < 4 {
                eprintln!("Falta api_key. Uso: bifrostctl apikey delete <api_key>");
                return;
            }
            let api_key = args[3].trim();
            match db::delete_api_key(pool, api_key).await {
                Ok(0) => eprintln!("No se encontro la API key indicada."),
                Ok(_) => println!("API key eliminada."),
                Err(err) => eprintln!("Error eliminando API key: {}", err),
            }
        }
        other => {
            eprintln!("Subcomando apikey desconocido '{}'.", other);
            print_usage();
        }
    }
}

async fn handle_docs_user_commands(pool: &sqlx::SqlitePool, args: &[String]) {
    if args.len() < 3 {
        print_usage();
        return;
    }

    match args[2].as_str() {
        "list" => match db::list_docs_users(pool).await {
            Ok(users) => {
                if users.is_empty() {
                    println!("No hay usuarios de documentación registrados.");
                } else {
                    println!("ID | USERNAME | STATUS | CREATED_AT");
                    for user in users {
                        let status = if user.is_active { "active" } else { "disabled" };
                        println!("{} | {} | {} | {}", user.id, user.username, status, user.created_at);
                    }
                }
            }
            Err(err) => eprintln!("Error listando usuarios de documentación: {}", err),
        },
        "create" => {
            if args.len() < 5 {
                eprintln!("Uso: bifrostctl docs-user create <username> <password>");
                return;
            }
            let username = args[3].trim();
            let password = args[4].trim();
            if username.is_empty() || password.is_empty() {
                eprintln!("username y password no pueden estar vacíos");
                return;
            }

            match db::create_docs_user(pool, username, password).await {
                Ok(_) => println!("Usuario de documentación '{}' creado.", username),
                Err(err) => eprintln!("Error creando usuario de documentación: {}", err),
            }
        }
        "passwd" => {
            if args.len() < 5 {
                eprintln!("Uso: bifrostctl docs-user passwd <username> <new_password>");
                return;
            }
            let username = args[3].trim();
            let password = args[4].trim();
            if username.is_empty() || password.is_empty() {
                eprintln!("username y new_password no pueden estar vacíos");
                return;
            }

            match db::update_docs_user_password(pool, username, password).await {
                Ok(0) => eprintln!("No se encontró el usuario indicado."),
                Ok(_) => println!("Password actualizado para '{}'.", username),
                Err(err) => eprintln!("Error actualizando password: {}", err),
            }
        }
        "enable" => {
            if args.len() < 4 {
                eprintln!("Uso: bifrostctl docs-user enable <username>");
                return;
            }
            let username = args[3].trim();
            match db::set_docs_user_active(pool, username, true).await {
                Ok(0) => eprintln!("No se encontró el usuario indicado."),
                Ok(_) => println!("Usuario '{}' habilitado.", username),
                Err(err) => eprintln!("Error habilitando usuario: {}", err),
            }
        }
        "disable" => {
            if args.len() < 4 {
                eprintln!("Uso: bifrostctl docs-user disable <username>");
                return;
            }
            let username = args[3].trim();
            match db::set_docs_user_active(pool, username, false).await {
                Ok(0) => eprintln!("No se encontró el usuario indicado."),
                Ok(_) => println!("Usuario '{}' deshabilitado.", username),
                Err(err) => eprintln!("Error deshabilitando usuario: {}", err),
            }
        }
        "delete" => {
            if args.len() < 4 {
                eprintln!("Uso: bifrostctl docs-user delete <username>");
                return;
            }
            let username = args[3].trim();
            match db::delete_docs_user(pool, username).await {
                Ok(0) => eprintln!("No se encontró el usuario indicado."),
                Ok(_) => println!("Usuario '{}' eliminado.", username),
                Err(err) => eprintln!("Error eliminando usuario: {}", err),
            }
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        other => {
            eprintln!("Subcomando docs-user desconocido '{}'.", other);
            print_usage();
        }
    }
}

fn print_usage() {
    println!("Uso:");
    println!("  bifrostctl apikey ...");
    println!("  bifrostctl apikey list");
    println!("  bifrostctl apikey create <user_name>");
    println!("  bifrostctl apikey enable <api_key>");
    println!("  bifrostctl apikey disable <api_key>");
    println!("  bifrostctl apikey delete <api_key>");
    println!("");
    println!("  bifrostctl docs-user list");
    println!("  bifrostctl docs-user create <username> <password>");
    println!("  bifrostctl docs-user passwd <username> <new_password>");
    println!("  bifrostctl docs-user enable <username>");
    println!("  bifrostctl docs-user disable <username>");
    println!("  bifrostctl docs-user delete <username>");
}
