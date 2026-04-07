use axum::{
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse},
    Json,
};

use crate::api::types::{
    ResponseCodeCreateRequest, ResponseCodeListQuery, ResponseCodePdfQuery,
    ResponseCodeTranslationUpsertRequest, ResponseCodeUpdateRequest, ResponseCodeWhoAmIResponse,
};

#[utoipa::path(
    get,
    path = "/api/response_codes/whoami",
    responses(
        (status = 200, description = "Usuario docs autenticado y permisos", body = ResponseCodeWhoAmIResponse)
    )
)]
pub async fn response_codes_whoami_handler(
    Extension(ctx): Extension<crate::middleware::DocsAuthContext>,
) -> impl IntoResponse {
    let permission = if ctx.can_manage_responses { "manage" } else { "view" };
    (
        axum::http::StatusCode::OK,
        Json(ResponseCodeWhoAmIResponse {
            username: ctx.username,
            responses_permission: permission.to_string(),
            can_manage_responses: ctx.can_manage_responses,
        }),
    )
}

#[utoipa::path(
    get,
    path = "/api/response_codes/manager",
    responses(
        (status = 200, description = "UI web para administrar response codes", content_type = "text/html")
    )
)]
pub async fn response_codes_ui_handler() -> impl IntoResponse {
        Html(RESPONSE_CODES_UI_HTML)
}

const RESPONSE_CODES_UI_HTML: &str = r#"<!doctype html>
<html lang="es">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Bifrost Response Codes Admin</title>
    <style>
        :root { --bg:#f5f7fb; --card:#fff; --ink:#1f2937; --muted:#6b7280; --line:#e5e7eb; --brand:#0f766e; --warn:#b91c1c; }
        body { margin:0; font-family:ui-sans-serif,system-ui,-apple-system,Segoe UI,Roboto; background:var(--bg); color:var(--ink); }
        .wrap { max-width:1200px; margin:24px auto; padding:0 16px; }
        .card { background:var(--card); border:1px solid var(--line); border-radius:12px; padding:16px; margin-bottom:14px; }
        h1 { margin:0 0 12px; font-size:22px; }
        .row { display:flex; gap:10px; flex-wrap:wrap; align-items:center; }
        input, select, button { padding:8px 10px; border:1px solid var(--line); border-radius:8px; font-size:14px; }
        button { background:var(--brand); color:white; border:none; cursor:pointer; }
        button.secondary { background:#374151; }
        button.danger { background:var(--warn); }
        button:disabled { opacity:.45; cursor:not-allowed; }
        table { width:100%; border-collapse:collapse; }
        th, td { border-bottom:1px solid var(--line); padding:8px; text-align:left; vertical-align:top; }
        th { background:#f9fafb; }
        .muted { color:var(--muted); font-size:12px; }
        .ok { color:#065f46; }
        .err { color:#991b1b; white-space:pre-wrap; }
    </style>
</head>
<body>
    <div class="wrap">
        <div class="card">
            <h1>Response Codes Admin</h1>
            <div id="whoami" class="muted">Cargando perfil...</div>
        </div>

        <div class="card">
            <div class="row">
                <label>Filtro idioma:</label>
                <input id="filterLang" placeholder="es, en, fr..." />
                <button onclick="loadCodes()">Refrescar</button>
                <button class="secondary" onclick="downloadPdf()">Descargar PDF</button>
            </div>
            <div id="status" class="muted" style="margin-top:8px"></div>
        </div>

        <div class="card" id="manageBlock">
            <div class="row">
                <input id="newCode" type="number" placeholder="code" />
                <input id="newType" placeholder="type" />
                <input id="newMessageEn" style="min-width:340px" placeholder="message_en" />
                <button id="btnCreate" onclick="createCode()">Crear/Actualizar code</button>
            </div>
            <div class="row" style="margin-top:10px">
                <input id="updCode" type="number" placeholder="code" />
                <input id="updType" placeholder="nuevo type (opcional)" />
                <input id="updMessageEn" style="min-width:340px" placeholder="nuevo message_en (opcional)" />
                <button id="btnUpdate" onclick="updateCode()">Actualizar code</button>
                <button id="btnDelete" class="danger" onclick="deleteCode()">Eliminar code</button>
            </div>
            <div class="row" style="margin-top:10px">
                <input id="trCode" type="number" placeholder="code" />
                <input id="trLang" placeholder="lang (ej: es)" />
                <input id="trMessage" style="min-width:340px" placeholder="message" />
                <button id="btnSetTr" onclick="setTranslation()">Guardar traducción</button>
                <button id="btnDelTr" class="danger" onclick="deleteTranslation()">Eliminar traducción</button>
            </div>
        </div>

        <div class="card">
            <table>
                <thead>
                    <tr><th>Code</th><th>Type</th><th>Lang</th><th>Message</th></tr>
                </thead>
                <tbody id="tbody"></tbody>
            </table>
        </div>
    </div>

    <script>
        const apiBase = '/api/response_codes';
        let canManage = false;

        function setStatus(text, ok=true) {
            const el = document.getElementById('status');
            el.className = ok ? 'ok' : 'err';
            el.textContent = text;
        }

        async function req(url, options={}) {
            const res = await fetch(url, {
                headers: { 'content-type': 'application/json', ...(options.headers || {}) },
                ...options
            });
            const ct = res.headers.get('content-type') || '';
            let body = null;
            if (ct.includes('application/json')) body = await res.json();
            return { res, body };
        }

        function renderTable(items) {
            const tbody = document.getElementById('tbody');
            tbody.innerHTML = '';
            for (const item of items || []) {
                item.translations.forEach((tr, idx) => {
                    const row = document.createElement('tr');
                    row.innerHTML = `
                        <td>${idx === 0 ? item.code : ''}</td>
                        <td>${idx === 0 ? item.type || item.kind : ''}</td>
                        <td>${tr.lang}</td>
                        <td>${tr.message}</td>
                    `;
                    tbody.appendChild(row);
                });
            }
        }

        async function loadWhoAmI() {
            const { res, body } = await req(`${apiBase}/whoami`);
            if (!res.ok) {
                document.getElementById('whoami').textContent = 'No autenticado o sin acceso.';
                return;
            }
            canManage = !!body.can_manage_responses;
            document.getElementById('whoami').textContent = `Usuario: ${body.username} | Permiso: ${body.responses_permission}`;
            ['btnCreate','btnUpdate','btnDelete','btnSetTr','btnDelTr'].forEach(id => {
                document.getElementById(id).disabled = !canManage;
            });
            if (!canManage) setStatus('Modo solo lectura (view).', true);
        }

        async function loadCodes() {
            const lang = document.getElementById('filterLang').value.trim();
            const q = lang ? `?lang=${encodeURIComponent(lang)}` : '';
            const { res, body } = await req(`${apiBase}${q}`);
            if (!res.ok) {
                setStatus(body?.message || 'Error cargando códigos', false);
                return;
            }
            renderTable(body.items || []);
            setStatus(`Listado actualizado (${(body.items||[]).length} códigos).`);
        }

        async function createCode() {
            const payload = {
                code: Number(document.getElementById('newCode').value),
                type: document.getElementById('newType').value,
                message_en: document.getElementById('newMessageEn').value,
            };
            const { res, body } = await req(apiBase, { method:'POST', body: JSON.stringify(payload) });
            setStatus(body?.message || `HTTP ${res.status}`, res.ok);
            if (res.ok) loadCodes();
        }

        async function updateCode() {
            const code = Number(document.getElementById('updCode').value);
            const t = document.getElementById('updType').value.trim();
            const m = document.getElementById('updMessageEn').value.trim();
            const payload = {};
            if (t) payload.type = t;
            if (m) payload.message_en = m;
            const { res, body } = await req(`${apiBase}/${code}`, { method:'PUT', body: JSON.stringify(payload) });
            setStatus(body?.message || `HTTP ${res.status}`, res.ok);
            if (res.ok) loadCodes();
        }

        async function deleteCode() {
            const code = Number(document.getElementById('updCode').value);
            const { res, body } = await req(`${apiBase}/${code}`, { method:'DELETE' });
            setStatus(body?.message || `HTTP ${res.status}`, res.ok);
            if (res.ok) loadCodes();
        }

        async function setTranslation() {
            const code = Number(document.getElementById('trCode').value);
            const lang = document.getElementById('trLang').value.trim();
            const message = document.getElementById('trMessage').value;
            const { res, body } = await req(`${apiBase}/${code}/lang/${encodeURIComponent(lang)}`, {
                method:'PUT', body: JSON.stringify({ message })
            });
            setStatus(body?.message || `HTTP ${res.status}`, res.ok);
            if (res.ok) loadCodes();
        }

        async function deleteTranslation() {
            const code = Number(document.getElementById('trCode').value);
            const lang = document.getElementById('trLang').value.trim();
            const { res, body } = await req(`${apiBase}/${code}/lang/${encodeURIComponent(lang)}`, { method:'DELETE' });
            setStatus(body?.message || `HTTP ${res.status}`, res.ok);
            if (res.ok) loadCodes();
        }

        function downloadPdf() {
            const lang = document.getElementById('filterLang').value.trim();
            const tz = Intl.DateTimeFormat().resolvedOptions().timeZone || '';
            const params = new URLSearchParams();
            if (lang) params.set('lang', lang);
            if (tz) params.set('tz', tz);
            const q = params.toString() ? `?${params.toString()}` : '';
            window.location.href = `${apiBase}/pdf${q}`;
        }

        (async () => {
            await loadWhoAmI();
            await loadCodes();
        })();
    </script>
</body>
</html>
"#;

#[utoipa::path(
    get,
    path = "/api/response_codes",
    params(ResponseCodeListQuery),
    responses(
        (status = 200, description = "Listado de códigos de respuesta", body = crate::api::types::ResponseCodeListResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ResponseCodeAdminResponse)
    )
)]
pub async fn list_response_codes_handler(
    State(state): State<crate::AppState>,
    Query(query): Query<ResponseCodeListQuery>,
) -> impl IntoResponse {
    crate::api::service::response_codes::list_response_codes_handler(state, query.lang).await
}

#[utoipa::path(
    post,
    path = "/api/response_codes",
    request_body = ResponseCodeCreateRequest,
    responses(
        (status = 201, description = "Código guardado", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 400, description = "Solicitud inválida", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ResponseCodeAdminResponse)
    )
)]
pub async fn create_response_code_handler(
    State(state): State<crate::AppState>,
    Json(payload): Json<ResponseCodeCreateRequest>,
) -> impl IntoResponse {
    crate::api::service::response_codes::create_response_code_handler(state, payload).await
}

#[utoipa::path(
    put,
    path = "/api/response_codes/{code}",
    request_body = ResponseCodeUpdateRequest,
    params(("code" = i64, Path, description = "Código de respuesta")),
    responses(
        (status = 200, description = "Código actualizado", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 400, description = "Solicitud inválida", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ResponseCodeAdminResponse)
    )
)]
pub async fn update_response_code_handler(
    State(state): State<crate::AppState>,
    Path(code): Path<i64>,
    Json(payload): Json<ResponseCodeUpdateRequest>,
) -> impl IntoResponse {
    crate::api::service::response_codes::update_response_code_handler(state, code, payload).await
}

#[utoipa::path(
    delete,
    path = "/api/response_codes/{code}",
    params(("code" = i64, Path, description = "Código de respuesta")),
    responses(
        (status = 200, description = "Código eliminado", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 404, description = "No encontrado", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ResponseCodeAdminResponse)
    )
)]
pub async fn delete_response_code_handler(
    State(state): State<crate::AppState>,
    Path(code): Path<i64>,
) -> impl IntoResponse {
    crate::api::service::response_codes::delete_response_code_handler(state, code).await
}

#[utoipa::path(
    put,
    path = "/api/response_codes/{code}/lang/{lang}",
    request_body = ResponseCodeTranslationUpsertRequest,
    params(
        ("code" = i64, Path, description = "Código de respuesta"),
        ("lang" = String, Path, description = "Idioma, por ejemplo es")
    ),
    responses(
        (status = 200, description = "Traducción guardada", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 400, description = "Solicitud inválida", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ResponseCodeAdminResponse)
    )
)]
pub async fn upsert_response_translation_handler(
    State(state): State<crate::AppState>,
    Path((code, lang)): Path<(i64, String)>,
    Json(payload): Json<ResponseCodeTranslationUpsertRequest>,
) -> impl IntoResponse {
    crate::api::service::response_codes::upsert_response_translation_handler(state, code, lang, payload).await
}

#[utoipa::path(
    delete,
    path = "/api/response_codes/{code}/lang/{lang}",
    params(
        ("code" = i64, Path, description = "Código de respuesta"),
        ("lang" = String, Path, description = "Idioma, por ejemplo es")
    ),
    responses(
        (status = 200, description = "Traducción eliminada", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 404, description = "No encontrada", body = crate::api::types::ResponseCodeAdminResponse),
        (status = 500, description = "Error interno", body = crate::api::types::ResponseCodeAdminResponse)
    )
)]
pub async fn delete_response_translation_handler(
    State(state): State<crate::AppState>,
    Path((code, lang)): Path<(i64, String)>,
) -> impl IntoResponse {
    crate::api::service::response_codes::delete_response_translation_handler(state, code, lang).await
}

#[utoipa::path(
    get,
    path = "/api/response_codes/pdf",
    params(ResponseCodePdfQuery),
    responses(
        (status = 200, description = "PDF de códigos de respuesta", content_type = "application/pdf"),
        (status = 500, description = "Error interno", body = crate::api::types::ResponseCodeAdminResponse)
    )
)]
pub async fn download_response_codes_pdf_handler(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<ResponseCodePdfQuery>,
) -> impl IntoResponse {
    crate::api::service::response_codes::download_response_codes_pdf_handler(
        state,
        headers,
        query.lang,
        query.tz,
    )
    .await
}
