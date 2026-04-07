use std::collections::BTreeMap;

use axum::{
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use chrono_tz::Tz;

use crate::api::types::{
    ResponseCodeAdminResponse, ResponseCodeCreateRequest, ResponseCodeItem, ResponseCodeListResponse,
    ResponseCodeTranslationItem, ResponseCodeTranslationUpsertRequest, ResponseCodeUpdateRequest,
};

fn normalize_lang(value: &str) -> String {
    crate::i18n::normalize_language_tag(value)
}

async fn build_response_code_items(
    pool: &sqlx::SqlitePool,
    lang_filter: Option<&str>,
) -> Result<Vec<ResponseCodeItem>, String> {
    let base = crate::db::list_response_codes_base(pool)
        .await
        .map_err(|e| format!("No se pudo listar response_codes: {}", e))?;
    let translations = crate::db::list_response_translations(pool)
        .await
        .map_err(|e| format!("No se pudo listar response_translations: {}", e))?;

    let mut by_code: BTreeMap<i64, Vec<(String, String)>> = BTreeMap::new();
    for tr in translations {
        by_code
            .entry(tr.code)
            .or_default()
            .push((tr.lang.to_ascii_lowercase(), tr.message));
    }

    let mut items = Vec::new();
    let filter = lang_filter.map(normalize_lang);

    for row in base {
        let mut item_translations = Vec::new();
        match filter.as_deref() {
            None => {
                item_translations.push(ResponseCodeTranslationItem {
                    lang: "en".to_string(),
                    message: row.message_en.clone(),
                });

                if let Some(extra) = by_code.get(&row.code) {
                    for (lang, message) in extra {
                        item_translations.push(ResponseCodeTranslationItem {
                            lang: lang.clone(),
                            message: message.clone(),
                        });
                    }
                }
            }
            Some("en") => {
                item_translations.push(ResponseCodeTranslationItem {
                    lang: "en".to_string(),
                    message: row.message_en.clone(),
                });
            }
            Some(selected) => {
                if let Some(extra) = by_code.get(&row.code) {
                    if let Some((lang, message)) = extra.iter().find(|(lang, _)| lang == selected) {
                        item_translations.push(ResponseCodeTranslationItem {
                            lang: lang.clone(),
                            message: message.clone(),
                        });
                    }
                }
            }
        }

        if !item_translations.is_empty() {
            items.push(ResponseCodeItem {
                code: row.code,
                kind: row.kind,
                translations: item_translations,
            });
        }
    }

    Ok(items)
}

fn escape_pdf_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

#[derive(Clone)]
struct PdfRow {
    code: String,
    kind: String,
    lang: String,
    message_lines: Vec<String>,
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();

    for word in text.split_whitespace() {
        if line.is_empty() {
            line.push_str(word);
            continue;
        }

        if line.len() + 1 + word.len() <= max_chars {
            line.push(' ');
            line.push_str(word);
        } else {
            out.push(line);
            line = word.to_string();
        }
    }

    if !line.is_empty() {
        out.push(line);
    }

    if out.is_empty() {
        out.push(String::new());
    }

    out
}

fn rows_from_items(items: &[ResponseCodeItem]) -> Vec<PdfRow> {
    let mut rows = Vec::new();
    for item in items {
        for (idx, tr) in item.translations.iter().enumerate() {
            rows.push(PdfRow {
                code: if idx == 0 { item.code.to_string() } else { String::new() },
                kind: if idx == 0 { item.kind.clone() } else { String::new() },
                lang: tr.lang.clone(),
                message_lines: wrap_text(&tr.message, 64),
            });
        }
    }
    rows
}

fn row_height(row: &PdfRow) -> f32 {
    let lines = row.message_lines.len().max(1) as f32;
    (lines * 12.0 + 6.0).max(18.0)
}

fn paginate_rows(rows: &[PdfRow]) -> Vec<Vec<PdfRow>> {
    let mut pages: Vec<Vec<PdfRow>> = Vec::new();
    let mut current: Vec<PdfRow> = Vec::new();

    let table_top: f32 = 760.0;
    let header_height: f32 = 18.0;
    let min_bottom_y: f32 = 70.0;
    let mut y = table_top - header_height;

    for row in rows {
        let h = row_height(row);
        if y - h < min_bottom_y && !current.is_empty() {
            pages.push(current);
            current = Vec::new();
            y = table_top - header_height;
        }
        current.push(row.clone());
        y -= h;
    }

    if current.is_empty() {
        pages.push(Vec::new());
    } else {
        pages.push(current);
    }

    pages
}

fn draw_text(content: &mut String, font: &str, size: i32, x: f32, y: f32, text: &str) {
    content.push_str(&format!(
        "BT\n/{} {} Tf\n{} {} Td\n({}) Tj\nET\n",
        font,
        size,
        x,
        y,
        escape_pdf_text(text)
    ));
}

fn render_page_content(rows: &[PdfRow], page: usize, total_pages: usize, generated_at: &str) -> String {
    let mut content = String::new();

    let x0: f32 = 40.0;
    let x1: f32 = 100.0;
    let x2: f32 = 210.0;
    let x3: f32 = 270.0;
    let x4: f32 = 555.0;

    let table_top: f32 = 760.0;
    let table_header_h: f32 = 18.0;

    draw_text(&mut content, "F2", 18, 40.0, 807.0, "Bifrost Gateway Service");
    draw_text(&mut content, "F1", 10, 40.0, 790.0, "Response Codes Report");
    content.push_str("0.75 G\n40 784 m 555 784 l S\n0 G\n");

    content.push_str("0.93 g\n");
    content.push_str(&format!("{} {} {} {} re f\n", x0, table_top - table_header_h, x4 - x0, table_header_h));
    content.push_str("0 g\n");
    content.push_str("0.85 G\n");
    content.push_str(&format!("{} {} m {} {} l S\n", x0, table_top, x4, table_top));
    content.push_str(&format!("{} {} m {} {} l S\n", x0, table_top - table_header_h, x4, table_top - table_header_h));
    content.push_str(&format!("{} {} m {} {} l S\n", x0, table_top, x0, table_top - table_header_h));
    content.push_str(&format!("{} {} m {} {} l S\n", x1, table_top, x1, table_top - table_header_h));
    content.push_str(&format!("{} {} m {} {} l S\n", x2, table_top, x2, table_top - table_header_h));
    content.push_str(&format!("{} {} m {} {} l S\n", x3, table_top, x3, table_top - table_header_h));
    content.push_str(&format!("{} {} m {} {} l S\n", x4, table_top, x4, table_top - table_header_h));
    content.push_str("0 G\n");

    draw_text(&mut content, "F2", 10, x0 + 4.0, table_top - 13.0, "CODE");
    draw_text(&mut content, "F2", 10, x1 + 4.0, table_top - 13.0, "TYPE");
    draw_text(&mut content, "F2", 10, x2 + 4.0, table_top - 13.0, "LANG");
    draw_text(&mut content, "F2", 10, x3 + 4.0, table_top - 13.0, "MESSAGE");

    let mut y = table_top - table_header_h;
    for row in rows {
        let h = row_height(row);
        let next_y = y - h;

        content.push_str("0.9 G\n");
        content.push_str(&format!("{} {} m {} {} l S\n", x0, next_y, x4, next_y));
        content.push_str(&format!("{} {} m {} {} l S\n", x0, y, x0, next_y));
        content.push_str(&format!("{} {} m {} {} l S\n", x1, y, x1, next_y));
        content.push_str(&format!("{} {} m {} {} l S\n", x2, y, x2, next_y));
        content.push_str(&format!("{} {} m {} {} l S\n", x3, y, x3, next_y));
        content.push_str(&format!("{} {} m {} {} l S\n", x4, y, x4, next_y));
        content.push_str("0 G\n");

        let base_text_y = y - 12.0;
        draw_text(&mut content, "F1", 10, x0 + 4.0, base_text_y, &row.code);
        draw_text(&mut content, "F1", 10, x1 + 4.0, base_text_y, &row.kind);
        draw_text(&mut content, "F1", 10, x2 + 4.0, base_text_y, &row.lang);
        for (idx, msg_line) in row.message_lines.iter().enumerate() {
            draw_text(
                &mut content,
                "F1",
                10,
                x3 + 4.0,
                base_text_y - (idx as f32 * 12.0),
                msg_line,
            );
        }

        y = next_y;
    }

    content.push_str("0.75 G\n40 44 m 555 44 l S\n0 G\n");
    draw_text(
        &mut content,
        "F1",
        9,
        40.0,
        30.0,
        &format!("Generated at {}", generated_at),
    );
    draw_text(
        &mut content,
        "F1",
        9,
        470.0,
        30.0,
        &format!("Page {} of {}", page, total_pages),
    );

    content
}

fn build_simple_pdf(items: &[ResponseCodeItem], generated_at: &str) -> Vec<u8> {
    let rows = rows_from_items(items);
    let pages = paginate_rows(&rows);
    let total_pages = pages.len();

    let mut objects: Vec<String> = Vec::new();
    objects.push(String::new());
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());

    let page_start_id = 5usize;
    let max_id = 4 + (total_pages * 2);

    let mut kids = String::new();
    for i in 0..total_pages {
        let page_id = page_start_id + (i * 2);
        kids.push_str(&format!("{} 0 R ", page_id));
    }
    objects.push(format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.trim_end(), total_pages));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".to_string());

    for i in 0..total_pages {
        let page_id = page_start_id + (i * 2);
        let content_id = page_id + 1;
        let stream = render_page_content(&pages[i], i + 1, total_pages, generated_at);

        if objects.len() <= page_id {
            objects.resize(page_id + 1, String::new());
        }
        objects[page_id] = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            content_id
        );

        if objects.len() <= content_id {
            objects.resize(content_id + 1, String::new());
        }
        objects[content_id] = format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        );
    }

    if objects.len() <= max_id {
        objects.resize(max_id + 1, String::new());
    }

    let mut pdf = String::new();
    pdf.push_str("%PDF-1.4\n");

    let mut offsets = vec![0usize; max_id + 1];
    for id in 1..=max_id {
        offsets[id] = pdf.len();
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", id, objects[id]));
    }

    let xref_pos = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", max_id + 1));
    pdf.push_str("0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        pdf.push_str(&format!("{:010} 00000 n \n", off));
    }
    pdf.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\n", max_id + 1));
    pdf.push_str(&format!("startxref\n{}\n%%EOF\n", xref_pos));

    pdf.into_bytes()
}

pub async fn list_response_codes_handler(
    state: crate::AppState,
    lang: Option<String>,
) -> impl IntoResponse {
    match build_response_code_items(&state.pool, lang.as_deref()).await {
        Ok(items) => (StatusCode::OK, Json(ResponseCodeListResponse { items })).into_response(),
        Err(err) => {
            state.logger.error(&err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "list".to_string(),
                    success: false,
                    message: err,
                }),
            )
                .into_response()
        }
    }
}

pub async fn create_response_code_handler(
    state: crate::AppState,
    payload: ResponseCodeCreateRequest,
) -> impl IntoResponse {
    if payload.kind.trim().is_empty() || payload.message_en.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ResponseCodeAdminResponse {
                response_code: crate::i18n::CODE_INVALID_INPUT,
                action: "create".to_string(),
                success: false,
                message: "type y message_en son requeridos".to_string(),
            }),
        )
            .into_response();
    }

    match crate::db::upsert_response_code(&state.pool, payload.code, &payload.kind, &payload.message_en).await {
        Ok(_) => (
            StatusCode::CREATED,
            Json(ResponseCodeAdminResponse {
                response_code: payload.code,
                action: "create".to_string(),
                success: true,
                message: "Response code guardado".to_string(),
            }),
        )
            .into_response(),
        Err(err) => {
            state.logger.error(&format!("Error creando response code: {}", err));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "create".to_string(),
                    success: false,
                    message: format!("No se pudo guardar response code: {}", err),
                }),
            )
                .into_response()
        }
    }
}

pub async fn update_response_code_handler(
    state: crate::AppState,
    code: i64,
    payload: ResponseCodeUpdateRequest,
) -> impl IntoResponse {
    if payload.kind.is_none() && payload.message_en.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ResponseCodeAdminResponse {
                response_code: crate::i18n::CODE_INVALID_INPUT,
                action: "update".to_string(),
                success: false,
                message: "Debe enviar al menos type o message_en".to_string(),
            }),
        )
            .into_response();
    }

    if let Some(kind) = payload.kind {
        if kind.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INVALID_INPUT,
                    action: "update".to_string(),
                    success: false,
                    message: "type no puede estar vacío".to_string(),
                }),
            )
                .into_response();
        }

        if let Err(err) = crate::db::set_response_code_type(&state.pool, code, &kind).await {
            state.logger.error(&format!("Error actualizando type: {}", err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "update".to_string(),
                    success: false,
                    message: format!("No se pudo actualizar type: {}", err),
                }),
            )
                .into_response();
        }
    }

    if let Some(message_en) = payload.message_en {
        if message_en.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INVALID_INPUT,
                    action: "update".to_string(),
                    success: false,
                    message: "message_en no puede estar vacío".to_string(),
                }),
            )
                .into_response();
        }

        if let Err(err) = crate::db::set_response_code_message_en(&state.pool, code, &message_en).await {
            state.logger.error(&format!("Error actualizando message_en: {}", err));
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "update".to_string(),
                    success: false,
                    message: format!("No se pudo actualizar message_en: {}", err),
                }),
            )
                .into_response();
        }
    }

    (
        StatusCode::OK,
        Json(ResponseCodeAdminResponse {
            response_code: code,
            action: "update".to_string(),
            success: true,
            message: "Response code actualizado".to_string(),
        }),
    )
        .into_response()
}

pub async fn delete_response_code_handler(
    state: crate::AppState,
    code: i64,
) -> impl IntoResponse {
    match crate::db::delete_response_code(&state.pool, code).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(ResponseCodeAdminResponse {
                response_code: crate::i18n::CODE_NOT_FOUND,
                action: "delete".to_string(),
                success: false,
                message: "Response code no encontrado".to_string(),
            }),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::OK,
            Json(ResponseCodeAdminResponse {
                response_code: code,
                action: "delete".to_string(),
                success: true,
                message: "Response code eliminado".to_string(),
            }),
        )
            .into_response(),
        Err(err) => {
            state.logger.error(&format!("Error eliminando response code: {}", err));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "delete".to_string(),
                    success: false,
                    message: format!("No se pudo eliminar response code: {}", err),
                }),
            )
                .into_response()
        }
    }
}

pub async fn upsert_response_translation_handler(
    state: crate::AppState,
    code: i64,
    lang: String,
    payload: ResponseCodeTranslationUpsertRequest,
) -> impl IntoResponse {
    let lang = normalize_lang(&lang);
    if lang == "en" {
        return (
            StatusCode::BAD_REQUEST,
            Json(ResponseCodeAdminResponse {
                response_code: crate::i18n::CODE_INVALID_INPUT,
                action: "set-lang".to_string(),
                success: false,
                message: "Para inglés use message_en del código base".to_string(),
            }),
        )
            .into_response();
    }

    if payload.message.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ResponseCodeAdminResponse {
                response_code: crate::i18n::CODE_INVALID_INPUT,
                action: "set-lang".to_string(),
                success: false,
                message: "message no puede estar vacío".to_string(),
            }),
        )
            .into_response();
    }

    match crate::db::upsert_response_translation(&state.pool, code, &lang, &payload.message).await {
        Ok(_) => (
            StatusCode::OK,
            Json(ResponseCodeAdminResponse {
                response_code: code,
                action: "set-lang".to_string(),
                success: true,
                message: format!("Traducción '{}' guardada", lang),
            }),
        )
            .into_response(),
        Err(err) => {
            state.logger.error(&format!("Error guardando traducción: {}", err));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "set-lang".to_string(),
                    success: false,
                    message: format!("No se pudo guardar traducción: {}", err),
                }),
            )
                .into_response()
        }
    }
}

pub async fn delete_response_translation_handler(
    state: crate::AppState,
    code: i64,
    lang: String,
) -> impl IntoResponse {
    let lang = normalize_lang(&lang);
    if lang == "en" {
        return (
            StatusCode::BAD_REQUEST,
            Json(ResponseCodeAdminResponse {
                response_code: crate::i18n::CODE_INVALID_INPUT,
                action: "del-lang".to_string(),
                success: false,
                message: "No se puede eliminar inglés del código base".to_string(),
            }),
        )
            .into_response();
    }

    match crate::db::delete_response_translation(&state.pool, code, &lang).await {
        Ok(0) => (
            StatusCode::NOT_FOUND,
            Json(ResponseCodeAdminResponse {
                response_code: crate::i18n::CODE_NOT_FOUND,
                action: "del-lang".to_string(),
                success: false,
                message: format!("No existe traducción '{}' para ese code", lang),
            }),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::OK,
            Json(ResponseCodeAdminResponse {
                response_code: code,
                action: "del-lang".to_string(),
                success: true,
                message: format!("Traducción '{}' eliminada", lang),
            }),
        )
            .into_response(),
        Err(err) => {
            state.logger.error(&format!("Error eliminando traducción: {}", err));
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "del-lang".to_string(),
                    success: false,
                    message: format!("No se pudo eliminar traducción: {}", err),
                }),
            )
                .into_response()
        }
    }
}

pub async fn download_response_codes_pdf_handler(
    state: crate::AppState,
    headers: HeaderMap,
    lang: Option<String>,
    timezone: Option<String>,
) -> impl IntoResponse {
    let selected_lang = lang.or_else(|| {
        headers
            .get("x-lang")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    });

    let items = match build_response_code_items(&state.pool, selected_lang.as_deref()).await {
        Ok(v) => v,
        Err(err) => {
            state.logger.error(&err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ResponseCodeAdminResponse {
                    response_code: crate::i18n::CODE_INTERNAL_ERROR,
                    action: "pdf".to_string(),
                    success: false,
                    message: err,
                }),
            )
                .into_response();
        }
    };

    let selected_tz = timezone
        .or_else(|| headers.get("x-timezone").and_then(|v| v.to_str().ok()).map(|v| v.to_string()))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let generated_at = if let Some(tz_name) = selected_tz {
        match tz_name.parse::<Tz>() {
            Ok(tz) => Utc::now()
                .with_timezone(&tz)
                .format("%Y-%m-%d %H:%M:%S %Z")
                .to_string(),
            Err(_) => Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        }
    } else {
        Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
    };

    let pdf = build_simple_pdf(&items, &generated_at);

    let filename = if let Some(lang) = selected_lang {
        format!("response_codes_{}.pdf", normalize_lang(&lang))
    } else {
        "response_codes_all.pdf".to_string()
    };

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        pdf,
    )
        .into_response()
}
