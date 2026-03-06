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

    if args[1] != "apikey" {
        eprintln!("Comando desconocido '{}'.", args[1]);
        print_usage();
        return;
    }

    let pool = db::init_db().await;

    if args.len() < 3 {
        print_usage();
        return;
    }

    match args[2].as_str() {
        "list" => {
            match db::list_api_keys(&pool).await {
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

            match db::create_api_key_for_user(&pool, user_name).await {
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
            match db::set_api_key_active(&pool, api_key, true).await {
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
            match db::set_api_key_active(&pool, api_key, false).await {
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
            match db::delete_api_key(&pool, api_key).await {
                Ok(0) => eprintln!("No se encontro la API key indicada."),
                Ok(_) => println!("API key eliminada."),
                Err(err) => eprintln!("Error eliminando API key: {}", err),
            }
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        other => {
            eprintln!("Subcomando apikey desconocido '{}'.", other);
            print_usage();
        }
    }
}

fn print_usage() {
    println!("Uso:");
    println!("  bifrostctl apikey list");
    println!("  bifrostctl apikey create <user_name>");
    println!("  bifrostctl apikey enable <api_key>");
    println!("  bifrostctl apikey disable <api_key>");
    println!("  bifrostctl apikey delete <api_key>");
}
