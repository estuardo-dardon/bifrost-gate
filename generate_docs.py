import os
import docx
from docx import Document
from docx.shared import Inches, Pt, RGBColor
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.oxml import OxmlElement, parse_xml
from docx.oxml.ns import nsdecls, qn

def create_element(name):
    return OxmlElement(name)

def set_cell_background(cell, fill_hex):
    shading_elm = parse_xml(f'<w:shd {nsdecls("w")} w:fill="{fill_hex}"/>')
    cell._tc.get_or_add_tcPr().append(shading_elm)

def set_cell_margins(cell, top=100, bottom=100, left=150, right=150):
    tcPr = cell._tc.get_or_add_tcPr()
    tcMar = OxmlElement('w:tcMar')
    for m, val in [('top', top), ('bottom', bottom), ('left', left), ('right', right)]:
        node = OxmlElement(f'w:{m}')
        node.set(qn('w:w'), str(val))
        node.set(qn('w:type'), 'dxa')
        tcMar.append(node)
    tcPr.append(tcMar)

def add_callout(doc, text, title="NOTA IMPORTANTE", border_color="003366", bg_color="F0F4F8"):
    tbl = doc.add_table(rows=1, cols=1)
    tbl.alignment = WD_TABLE_ALIGNMENT.CENTER
    cell = tbl.cell(0, 0)
    set_cell_background(cell, bg_color)
    set_cell_margins(cell, top=120, bottom=120, left=200, right=200)
    
    # Left border only
    tcPr = cell._tc.get_or_add_tcPr()
    tcBorders = parse_xml(f'''
        <w:tcBorders {nsdecls("w")}>
            <w:top w:val="none"/>
            <w:left w:val="single" w:sz="36" w:space="0" w:color="{border_color}"/>
            <w:bottom w:val="none"/>
            <w:right w:val="none"/>
        </w:tcBorders>
    ''')
    tcPr.append(tcBorders)
    
    p = cell.paragraphs[0]
    p.paragraph_format.space_before = Pt(2)
    p.paragraph_format.space_after = Pt(4)
    run_t = p.add_run(f"📌 {title}\n")
    run_t.bold = True
    run_t.font.name = 'Calibri'
    run_t.font.size = Pt(10.5)
    run_t.font.color.rgb = RGBColor(0x00, 0x33, 0x66)
    
    run_b = p.add_run(text)
    run_b.font.name = 'Calibri'
    run_b.font.size = Pt(10)
    run_b.font.color.rgb = RGBColor(0x33, 0x33, 0x33)

def add_code_block(doc, code_text):
    tbl = doc.add_table(rows=1, cols=1)
    tbl.alignment = WD_TABLE_ALIGNMENT.CENTER
    cell = tbl.cell(0, 0)
    set_cell_background(cell, "F8F9FA")
    set_cell_margins(cell, top=100, bottom=100, left=150, right=150)
    
    tcPr = cell._tc.get_or_add_tcPr()
    tcBorders = parse_xml(f'''
        <w:tcBorders {nsdecls("w")}>
            <w:top w:val="single" w:sz="4" w:space="0" w:color="E0E0E0"/>
            <w:left w:val="single" w:sz="4" w:space="0" w:color="E0E0E0"/>
            <w:bottom w:val="single" w:sz="4" w:space="0" w:color="E0E0E0"/>
            <w:right w:val="single" w:sz="4" w:space="0" w:color="E0E0E0"/>
        </w:tcBorders>
    ''')
    tcPr.append(tcBorders)
    
    p = cell.paragraphs[0]
    p.paragraph_format.space_before = Pt(2)
    p.paragraph_format.space_after = Pt(2)
    run = p.add_run(code_text)
    run.font.name = 'Consolas'
    run.font.size = Pt(9.5)
    run.font.color.rgb = RGBColor(0x24, 0x29, 0x2E)

def style_heading(p, font_size, color_rgb, space_before, space_after, bold=True):
    p.paragraph_format.space_before = Pt(space_before)
    p.paragraph_format.space_after = Pt(space_after)
    for run in p.runs:
        run.font.name = 'Calibri'
        run.font.size = Pt(font_size)
        run.font.color.rgb = color_rgb
        run.bold = bold

def main():
    doc = Document()
    
    # Page setup
    sections = doc.sections
    for section in sections:
        section.top_margin = Inches(1)
        section.bottom_margin = Inches(1)
        section.left_margin = Inches(1)
        section.right_margin = Inches(1)

    PRIMARY_COLOR = RGBColor(0x00, 0x33, 0x66)    # Dark Navy
    SECONDARY_COLOR = RGBColor(0x46, 0x82, 0xB4)  # Steel Blue
    TEXT_COLOR = RGBColor(0x33, 0x33, 0x33)       # Charcoal
    
    # Base Normal Style
    normal_style = doc.styles['Normal']
    normal_style.font.name = 'Calibri'
    normal_style.font.size = Pt(11)
    normal_style.font.color.rgb = TEXT_COLOR
    
    # --- COVER / TITLE BLOCK ---
    title_p = doc.add_paragraph()
    title_p.paragraph_format.space_before = Pt(36)
    title_p.paragraph_format.space_after = Pt(6)
    run_title = title_p.add_run("BIFRÖST-GATE VPN ORCHESTRATOR")
    run_title.font.name = 'Calibri'
    run_title.font.size = Pt(26)
    run_title.bold = True
    run_title.font.color.rgb = PRIMARY_COLOR
    
    subtitle_p = doc.add_paragraph()
    subtitle_p.paragraph_format.space_after = Pt(18)
    run_sub = subtitle_p.add_run("Especificación Técnica de Arquitectura, Integración y Guía de API RESTful")
    run_sub.font.name = 'Calibri'
    run_sub.font.size = Pt(16)
    run_sub.font.color.rgb = SECONDARY_COLOR
    
    meta_p = doc.add_paragraph()
    meta_p.paragraph_format.space_after = Pt(24)
    run_meta = meta_p.add_run("Versión del Documento: 1.2.0 | Nivel de Confidencialidad: Uso Interno / Partners\nFecha: Julio 2026 | Autor: Arquitectura de Software & Seguridad Enterprise")
    run_meta.font.name = 'Calibri'
    run_meta.font.size = Pt(9.5)
    run_meta.font.color.rgb = RGBColor(0x66, 0x66, 0x66)
    
    doc.add_paragraph().paragraph_format.space_after = Pt(12)

    # --- TABLA DE CONTENIDOS (RESUMEN) ---
    h1 = doc.add_heading("1. RESUMEN EJECUTIVO Y PROPÓSITO", level=1)
    style_heading(h1, 18, PRIMARY_COLOR, 16, 6)
    
    p = doc.add_paragraph(
        "Bifröst-Gate es una plataforma enterprise desarrollada en Rust sobre el framework Axum para la orquestación, "
        "gestión automatizada y monitoreo en tiempo real de infraestructuras VPN basadas en StrongSwan (IPsec / IKEv2). "
        "El sistema provee una capa de abstracción de alto nivel mediante una API RESTful moderna, OpenAPI/Swagger incorporado, "
        "internacionalización de mensajes (i18n), autenticación híbrida granular y almacenamiento persistente transaccional en SQLite."
    )
    p.paragraph_format.space_after = Pt(8)

    add_callout(doc, 
        "Bifröst-Gate actúa como el plano de control (Control Plane) para la gestión de túneles IPsec. "
        "Toda interacción directa con el stack nativo de StrongSwan se efectúa mediante comandos atómicos y seguros (swanctl/VICI), "
        "garantizando cero tiempo de inactividad (zero-downtime) y aislamiento completo de fallos.",
        title="VISIÓN ARQUITECTÓNICA DEL SISTEMA"
    )

    doc.add_paragraph().paragraph_format.space_after = Pt(8)

    # --- ARQUITECTURA DE SEGURIDAD ---
    h2 = doc.add_heading("2. ARQUITECTURA DE SEGURIDAD Y AUTENTICACIÓN HÍBRIDA", level=1)
    style_heading(h2, 18, PRIMARY_COLOR, 16, 6)
    
    p = doc.add_paragraph(
        "Bifröst-Gate implementa un modelo de seguridad por capas (Defense-in-Depth) "
        "que aísla las interfaces administrativas, los endpoints de ingesta de métricas y la API operativa principal:"
    )
    p.paragraph_format.space_after = Pt(8)

    # Tabla de autenticacion
    table = doc.add_table(rows=4, cols=3)
    table.alignment = WD_TABLE_ALIGNMENT.CENTER
    headers = ["Capa / Endpoint", "Mecanismo de Autenticación", "Propósito & Alcance"]
    
    # Style Header
    hdr_cells = table.rows[0].cells
    for i, title in enumerate(headers):
        hdr_cells[i].text = title
        set_cell_background(hdr_cells[i], "003366")
        set_cell_margins(hdr_cells[i], 100, 100, 150, 150)
        p_hdr = hdr_cells[i].paragraphs[0]
        for run in p_hdr.runs:
            run.font.name = 'Calibri'
            run.font.bold = True
            run.font.color.rgb = RGBColor(0xFF, 0xFF, 0xFF)

    data = [
        ("/api/* (Endpoints Operativos)", "Seguridad Híbrida: API Key + JWT (RBAC Scopes)", "Gestión de Conexiones, Secretos, Certificados y Control de Peers IPsec."),
        ("/metrics (Métricas Prometheus)", "HTTP Basic Auth (Tabla api_users)", "Ingesta automatizada para Prometheus / Grafana sin exponer keys operativas."),
        ("/api/docs, /howto, /tryme", "HTTP Basic Auth (Tabla docs_users)", "Acceso a documentación Swagger, Redoc y herramientas de prueba interactivas.")
    ]

    for row_idx, row_data in enumerate(data, start=1):
        row_cells = table.rows[row_idx].cells
        bg_color = "F9FBFD" if row_idx % 2 == 1 else "FFFFFF"
        for col_idx, cell_value in enumerate(row_data):
            row_cells[col_idx].text = cell_value
            set_cell_background(row_cells[col_idx], bg_color)
            set_cell_margins(row_cells[col_idx], 80, 80, 120, 120)
            p_cell = row_cells[col_idx].paragraphs[0]
            for run in p_cell.runs:
                run.font.name = 'Calibri'
                run.font.size = Pt(10)

    doc.add_paragraph().paragraph_format.space_after = Pt(12)

    # --- FLUJOS DE INTEGRACION ---
    h3 = doc.add_heading("3. DIAGRAMAS DE FLUJO Y PROCESO DE AUTENTICACIÓN", level=1)
    style_heading(h3, 18, PRIMARY_COLOR, 16, 6)

    doc.add_paragraph("A continuación se ilustra el flujo de validación del Middleware de Seguridad Híbrida:")

    diagram_text = (
        "+-----------------------------------------------------------------------+\n"
        "|                 Petición de Cliente HTTP (REST API)                   |\n"
        "+-----------------------------------------------------------------------+\n"
        "                                    |\n"
        "                                    v\n"
        "                [ Cabecera 'x-api-key' presente? ]\n"
        "                       /                 \\\n"
        "                     (No)               (Sí)\n"
        "                     /                     \\\n"
        "                    v                       v\n"
        "         HTTP 401 Unauthorized     [ API Key activa en DB? ]\n"
        "         (Code: 30000/30001)             /            \\\n"
        "                                       (No)           (Sí)\n"
        "                                       /                \\\n"
        "                                      v                  v\n"
        "                          HTTP 401 Unauthorized    [ Bearer JWT Token Válido? ]\n"
        "                          (Code: 30002)                 /            \\\n"
        "                                                      (No)           (Sí)\n"
        "                                                      /                \\\n"
        "                                                     v                  v\n"
        "                                         HTTP 401 Unauthorized  [ Evaluar Intersección ]\n"
        "                                         (Code: 30003)          [    de Permisos       ]\n"
        "                                                                       |\n"
        "                                                                [ Scope Autorizado? ]\n"
        "                                                                 /              \\\n"
        "                                                               (No)             (Sí)\n"
        "                                                               /                  \\\n"
        "                                                              v                    v\n"
        "                                                    HTTP 403 Forbidden     HTTP 200 / Exec\n"
        "                                                    (Code: 40301)          (Controlador)\n"
    )
    add_code_block(doc, diagram_text)

    doc.add_paragraph().paragraph_format.space_after = Pt(12)

    # --- CASOS DE USO Y EJEMPLOS DE INTEGRACION ---
    h4 = doc.add_heading("4. CASOS DE USO INTEGRALES Y EJEMPLOS DE PAYLOADS", level=1)
    style_heading(h4, 18, PRIMARY_COLOR, 16, 6)

    # Caso 1
    sub1 = doc.add_heading("Caso 1: Configuración de Conexión Sitio a Sitio (Peer-to-Peer PSK)", level=2)
    style_heading(sub1, 14, SECONDARY_COLOR, 12, 4)

    doc.add_paragraph("Para establecer un túnel IPsec PSK entre dos sedes corporativas:")
    
    code_c1_secret = (
        "// 1. Crear Secreto Pre-Shared Key (POST /api/secrets)\n"
        "{\n"
        '  "name": "psk-site-b",\n'
        '  "secret_type": "ike",\n'
        '  "config": {\n'
        '    "secret": "SuperSecretEnterpriseKey2026!",\n'
        '    "owners": ["192.168.1.1", "192.168.2.1"]\n'
        "  }\n"
        "}"
    )
    add_code_block(doc, code_c1_secret)

    code_c1_conn = (
        "// 2. Crear Conexión VPN IPsec (POST /api/connections)\n"
        "{\n"
        '  "name": "conn-site-b",\n'
        '  "config": {\n'
        '    "version": 2,\n'
        '    "local_addrs": "192.168.1.1",\n'
        '    "remote_addrs": "192.168.2.1",\n'
        '    "proposals": "aes256-sha256-modp2048",\n'
        '    "children": {\n'
        '      "net-to-net": {\n'
        '        "local_ts": "10.10.0.0/16",\n'
        '        "remote_ts": "10.20.0.0/16",\n'
        '        "start_action": "trap"\n'
        "      }\n"
        "    }\n"
        "  }\n"
        "}"
    )
    add_code_block(doc, code_c1_conn)

    # Caso 2
    sub2 = doc.add_heading("Caso 2: Gestión de Certificados X.509 y PKI (Roadwarrior VPN)", level=2)
    style_heading(sub2, 14, SECONDARY_COLOR, 12, 4)

    doc.add_paragraph("Creación de una Entidad Emisora (CA) y asignación de certificado para cliente VPN:")

    code_c2_ca = (
        "// 1. Emitir Root CA (POST /api/certificates/ca)\n"
        "{\n"
        '  "name": "corp-root-ca",\n'
        '  "common_name": "Enterprise Corporate Root CA",\n'
        '  "organization": "Bifrost Network Systems",\n'
        '  "country": "GT",\n'
        '  "days": 3650,\n'
        '  "key_size": 4096\n'
        "}"
    )
    add_code_block(doc, code_c2_ca)

    # --- ADMINISTRACION VIA BIFROSTCTL ---
    h5 = doc.add_heading("5. ADMINISTRACIÓN EN CONSOLA CON BIFROSTCTL", level=1)
    style_heading(h5, 18, PRIMARY_COLOR, 16, 6)

    doc.add_paragraph(
        "Para tareas de infraestructura y operaciones en servidor, la herramienta CLI 'bifrostctl' "
        "permite gestionar API Keys y Usuarios Administrativos directamente sobre la base SQLite:"
    )

    cli_examples = (
        "# Gestión de Usuarios para Métricas Prometheus (/metrics)\n"
        "sudo bifrostctl api-user create prometheus_collector MySecurePass123!\n"
        "sudo bifrostctl api-user list\n"
        "sudo bifrostctl api-user passwd prometheus_collector NewPass456!\n\n"
        "# Gestión de API Keys Operativas\n"
        "sudo bifrostctl apikey create partner_integration_app\n"
        "sudo bifrostctl apikey list\n"
        "sudo bifrostctl apikey disable bfg_xxxxxx"
    )
    add_code_block(doc, cli_examples)

    doc.add_paragraph().paragraph_format.space_after = Pt(12)

    # --- CATÁLOGO DE RESPUESTAS E I18N ---
    h6 = doc.add_heading("6. CATÁLOGO ESTÁNDAR DE CÓDIGOS DE RESPUESTA E I18N", level=1)
    style_heading(h6, 18, PRIMARY_COLOR, 16, 6)

    doc.add_paragraph(
        "Bifröst-Gate incluye un catálogo internacionalizado (i18n) de códigos de respuesta. "
        "El idioma del mensaje devuelto en el JSON se resuelve dinámicamente mediante la cabecera HTTP 'Accept-Language':"
    )

    # Tabla de codigos
    t_code = doc.add_table(rows=6, cols=3)
    t_code.alignment = WD_TABLE_ALIGNMENT.CENTER
    
    headers_c = ["Código HTTP / Interno", "Tipo / Módulo", "Descripción & Mensaje por Defecto"]
    hdr_c_cells = t_code.rows[0].cells
    for i, title in enumerate(headers_c):
        hdr_c_cells[i].text = title
        set_cell_background(hdr_c_cells[i], "003366")
        set_cell_margins(hdr_c_cells[i], 100, 100, 150, 150)
        p_hdr = hdr_c_cells[i].paragraphs[0]
        for run in p_hdr.runs:
            run.font.name = 'Calibri'
            run.font.bold = True
            run.font.color.rgb = RGBColor(0xFF, 0xFF, 0xFF)

    data_c = [
        ("20000", "Global", "Request executed successfully / Solicitud ejecutada exitosamente."),
        ("30000", "Auth", "API Key Required / API Key requerida."),
        ("30003", "Auth", "Missing or invalid JWT token / Token JWT ausente o inválido."),
        ("40301", "Auth", "Insufficient permissions to perform this action / Permisos insuficientes."),
        ("50000", "Fatal", "General Error / Error interno del servidor.")
    ]

    for row_idx, row_data in enumerate(data_c, start=1):
        row_cells = t_code.rows[row_idx].cells
        bg_color = "F9FBFD" if row_idx % 2 == 1 else "FFFFFF"
        for col_idx, cell_value in enumerate(row_data):
            row_cells[col_idx].text = cell_value
            set_cell_background(row_cells[col_idx], bg_color)
            set_cell_margins(row_cells[col_idx], 80, 80, 120, 120)
            p_cell = row_cells[col_idx].paragraphs[0]
            for run in p_cell.runs:
                run.font.name = 'Calibri'
                run.font.size = Pt(10)

    doc.add_paragraph().paragraph_format.space_after = Pt(18)

    # Final Notes
    add_callout(doc, 
        "Para probar de forma interactiva la API, puedes utilizar el Swagger UI incorporado en "
        "http://<server-ip>:2704/api/docs o importar la colección oficial de Postman 'postman/Bifrost-Gate.postman_collection.json'.",
        title="RECURSOS ADICIONALES Y HERRAMIENTAS"
    )

    output_filename = "Bifrost-Gate_Especificacion_Tecnica_API.docx"
    doc.save(output_filename)
    print(f"Documento generado exitosamente: {output_filename}")

if __name__ == "__main__":
    main()
