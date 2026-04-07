/*
 * Bifrostctl: CLI de administración para Bifröst-Gate.
 */

#[path = "../db.rs"]
mod db;

use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("bifrostctl {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if !is_running_as_root() {
        eprintln!("bifrostctl requiere privilegios de root. Ejecute con sudo.");
        std::process::exit(1);
    }

    if args.len() < 2 {
        print_usage();
        return;
    }

    let pool = db::init_db().await;

    match args[1].as_str() {
        "apikey" => handle_apikey_commands(&pool, &args).await,
        "docs-user" => handle_docs_user_commands(&pool, &args).await,
        "response" => handle_response_commands(&pool, &args).await,
        "version" => println!("bifrostctl {}", env!("CARGO_PKG_VERSION")),
        "help" | "--help" | "-h" => print_usage(),
        other => {
            eprintln!("Comando desconocido '{}'.", other);
            print_usage();
        }
    }
}

#[cfg(unix)]
fn is_running_as_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn is_running_as_root() -> bool {
    false
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
                    println!("ID | USERNAME | STATUS | RESPONSES_PERM | CREATED_AT");
                    for user in users {
                        let status = if user.is_active { "active" } else { "disabled" };
                        let perm = if user.can_manage_responses { "manage" } else { "view" };
                        println!("{} | {} | {} | {} | {}", user.id, user.username, status, perm, user.created_at);
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
        "grant-responses-manage" => {
            if args.len() < 4 {
                eprintln!("Uso: bifrostctl docs-user grant-responses-manage <username>");
                return;
            }
            let username = args[3].trim();
            match db::set_docs_user_manage_responses(pool, username, true).await {
                Ok(0) => eprintln!("No se encontró el usuario indicado."),
                Ok(_) => println!("Permiso de administración de response codes concedido a '{}'.", username),
                Err(err) => eprintln!("Error actualizando permisos: {}", err),
            }
        }
        "revoke-responses-manage" => {
            if args.len() < 4 {
                eprintln!("Uso: bifrostctl docs-user revoke-responses-manage <username>");
                return;
            }
            let username = args[3].trim();
            match db::set_docs_user_manage_responses(pool, username, false).await {
                Ok(0) => eprintln!("No se encontró el usuario indicado."),
                Ok(_) => println!("Permiso de administración de response codes revocado para '{}'.", username),
                Err(err) => eprintln!("Error actualizando permisos: {}", err),
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

async fn handle_response_commands(pool: &sqlx::SqlitePool, args: &[String]) {
    if args.len() < 3 {
        print_usage();
        return;
    }

    match args[2].as_str() {
        "list" => {
            let lang = args.get(3).map(|s| s.as_str());
            match db::list_response_messages(pool, lang).await {
                Ok(rows) => {
                    if rows.is_empty() {
                        println!("No hay códigos de respuesta registrados.");
                    } else {
                        println!("CODE | TYPE | LANG | MESSAGE");
                        for row in rows {
                            println!("{} | {} | {} | {}", row.code, row.kind, row.lang, row.message);
                        }
                    }
                }
                Err(err) => eprintln!("Error listando catálogo de respuestas: {}", err),
            }
        }
        "add" => {
            if args.len() < 6 {
                eprintln!("Uso: bifrostctl response add <code> <type> <message_en>");
                return;
            }

            let code = match parse_code(&args[3]) {
                Some(v) => v,
                None => {
                    eprintln!("code inválido: '{}'", args[3]);
                    return;
                }
            };
            let kind = args[4].trim();
            let message_en = join_args(args, 5);

            if kind.is_empty() || message_en.trim().is_empty() {
                eprintln!("type y message_en son requeridos");
                return;
            }

            match db::upsert_response_code(pool, code, kind, message_en.trim()).await {
                Ok(_) => println!("Código {} guardado/actualizado.", code),
                Err(err) => eprintln!("Error guardando código {}: {}", code, err),
            }
        }
        "set-en" => {
            if args.len() < 5 {
                eprintln!("Uso: bifrostctl response set-en <code> <message_en>");
                return;
            }

            let code = match parse_code(&args[3]) {
                Some(v) => v,
                None => {
                    eprintln!("code inválido: '{}'", args[3]);
                    return;
                }
            };
            let message_en = join_args(args, 4);

            if message_en.trim().is_empty() {
                eprintln!("message_en no puede estar vacío");
                return;
            }

            match db::set_response_code_message_en(pool, code, message_en.trim()).await {
                Ok(0) => eprintln!("No existe el code {}.", code),
                Ok(_) => println!("Mensaje base (en) actualizado para code {}.", code),
                Err(err) => eprintln!("Error actualizando message_en: {}", err),
            }
        }
        "set-type" => {
            if args.len() < 5 {
                eprintln!("Uso: bifrostctl response set-type <code> <type>");
                return;
            }

            let code = match parse_code(&args[3]) {
                Some(v) => v,
                None => {
                    eprintln!("code inválido: '{}'", args[3]);
                    return;
                }
            };
            let kind = args[4].trim();
            if kind.is_empty() {
                eprintln!("type no puede estar vacío");
                return;
            }

            match db::set_response_code_type(pool, code, kind).await {
                Ok(0) => eprintln!("No existe el code {}.", code),
                Ok(_) => println!("Type actualizado para code {}.", code),
                Err(err) => eprintln!("Error actualizando type: {}", err),
            }
        }
        "set-lang" => {
            if args.len() < 6 {
                eprintln!("Uso: bifrostctl response set-lang <code> <lang> <message>");
                return;
            }

            let code = match parse_code(&args[3]) {
                Some(v) => v,
                None => {
                    eprintln!("code inválido: '{}'", args[3]);
                    return;
                }
            };
            let lang = args[4].trim().to_ascii_lowercase();
            let message = join_args(args, 5);

            if lang.is_empty() || lang == "en" {
                eprintln!("lang debe ser distinto de 'en' para traducciones (usa set-en para inglés)");
                return;
            }
            if message.trim().is_empty() {
                eprintln!("message no puede estar vacío");
                return;
            }

            match db::upsert_response_translation(pool, code, &lang, message.trim()).await {
                Ok(_) => println!("Traducción {} guardada para code {}.", lang, code),
                Err(err) => eprintln!("Error guardando traducción: {}", err),
            }
        }
        "del-lang" => {
            if args.len() < 5 {
                eprintln!("Uso: bifrostctl response del-lang <code> <lang>");
                return;
            }

            let code = match parse_code(&args[3]) {
                Some(v) => v,
                None => {
                    eprintln!("code inválido: '{}'", args[3]);
                    return;
                }
            };
            let lang = args[4].trim().to_ascii_lowercase();
            if lang.is_empty() || lang == "en" {
                eprintln!("lang debe ser distinto de 'en'");
                return;
            }

            match db::delete_response_translation(pool, code, &lang).await {
                Ok(0) => eprintln!("No existe traducción {} para code {}.", lang, code),
                Ok(_) => println!("Traducción {} eliminada para code {}.", lang, code),
                Err(err) => eprintln!("Error eliminando traducción: {}", err),
            }
        }
        "delete" => {
            if args.len() < 4 {
                eprintln!("Uso: bifrostctl response delete <code>");
                return;
            }

            let code = match parse_code(&args[3]) {
                Some(v) => v,
                None => {
                    eprintln!("code inválido: '{}'", args[3]);
                    return;
                }
            };

            match db::delete_response_code(pool, code).await {
                Ok(0) => eprintln!("No existe el code {}.", code),
                Ok(_) => println!("Code {} eliminado.", code),
                Err(err) => eprintln!("Error eliminando code {}: {}", code, err),
            }
        }
        "help" | "--help" | "-h" => print_usage(),
        other => {
            eprintln!("Subcomando response desconocido '{}'.", other);
            print_usage();
        }
    }
}

fn parse_code(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok()
}

fn join_args(args: &[String], start: usize) -> String {
    if args.len() <= start {
        return String::new();
    }
    args[start..].join(" ")
}

fn print_usage() {
    println!("Uso:");
    println!("  bifrostctl --version");
    println!("  bifrostctl version");
    println!("");
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
    println!("  bifrostctl docs-user grant-responses-manage <username>");
    println!("  bifrostctl docs-user revoke-responses-manage <username>");
    println!("");
    println!("  bifrostctl response list [lang]");
    println!("  bifrostctl response add <code> <type> <message_en>");
    println!("  bifrostctl response set-en <code> <message_en>");
    println!("  bifrostctl response set-type <code> <type>");
    println!("  bifrostctl response set-lang <code> <lang> <message>");
    println!("  bifrostctl response del-lang <code> <lang>");
    println!("  bifrostctl response delete <code>");
}
