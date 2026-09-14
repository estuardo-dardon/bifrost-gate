use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, Algorithm, Header, EncodingKey};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    scopes: Vec<String>,
    exp: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: gen_token <secret> <sub> <scopes...>");
        std::process::exit(1);
    }
    let secret = &args[1];
    let sub = &args[2];
    let scopes: Vec<String> = args[3..].to_vec();

    // Expiración en 1 hora
    let exp = (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize;

    let claims = Claims {
        sub: sub.clone(),
        scopes,
        exp,
    };

    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).unwrap();

    println!("{}", token);
}
