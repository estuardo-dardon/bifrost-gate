use axum::{
    routing::{delete, get, post, put},
    Router,
};

pub fn routes() -> Router<crate::AppState> {
    Router::new()
        .route(
            "/api/response_codes",
            get(crate::api::controller::response_codes::list_response_codes_handler),
        )
        .route(
            "/api/response_codes/whoami",
            get(crate::api::controller::response_codes::response_codes_whoami_handler),
        )
        .route(
            "/api/response_codes/manager",
            get(crate::api::controller::response_codes::response_codes_ui_handler),
        )
        .route(
            "/api/response_codes",
            post(crate::api::controller::response_codes::create_response_code_handler),
        )
        .route(
            "/api/response_codes/:code",
            put(crate::api::controller::response_codes::update_response_code_handler),
        )
        .route(
            "/api/response_codes/:code",
            delete(crate::api::controller::response_codes::delete_response_code_handler),
        )
        .route(
            "/api/response_codes/:code/lang/:lang",
            put(crate::api::controller::response_codes::upsert_response_translation_handler),
        )
        .route(
            "/api/response_codes/:code/lang/:lang",
            delete(crate::api::controller::response_codes::delete_response_translation_handler),
        )
        .route(
            "/api/response_codes/pdf",
            get(crate::api::controller::response_codes::download_response_codes_pdf_handler),
        )
}
