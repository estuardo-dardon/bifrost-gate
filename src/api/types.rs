use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PeerControlResponse {
    pub peer_name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

/// Parametros de query para controlar una fase especifica del peer.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct PeerControlQuery {
    /// Fase a operar: 1 (IKE/Fase 1), 2 (CHILD SA/Fase 2). Omitir para actuar sobre ambas.
    pub phase: Option<u8>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ServiceControlResponse {
    pub service_name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct FirewallRulesResponse {
    pub firewall: Vec<String>,
    pub filter: Vec<String>,
    pub nat: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PeerPhaseStatusResponse {
    pub state: String,
    pub active: bool,
    pub active_for_seconds: Option<u64>,
    pub packets_in: u64,
    pub packets_out: u64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ChildSaStatusResponse {
    pub name: String,
    pub state: String,
    pub active: bool,
    pub active_for_seconds: Option<u64>,
    pub packets_in: u64,
    pub packets_out: u64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PeerStatusResponse {
    pub peer_name: String,
    pub phase1: PeerPhaseStatusResponse,
    pub phase2: PeerPhaseStatusResponse,
    pub child_sas: Vec<ChildSaStatusResponse>,
    pub firewall_rules: FirewallRulesResponse,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PeerStatusListResponse {
    pub peers: Vec<PeerStatusResponse>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PeerStatusErrorResponse {
    pub peer_name: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ConnectionUpsertRequest {
    pub config: Value,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ConnectionCreateRequest {
    pub name: String,
    pub config: Value,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ConnectionResponse {
    pub name: String,
    pub config: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ConnectionListResponse {
    pub connections: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ConnectionCrudResponse {
    pub name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SecretType {
    Eap,
    Xauth,
    Ntlm,
    Ike,
    Ppk,
    Private,
    Rsa,
    Ecdsa,
    Pkcs8,
    Pkcs12,
    Token,
}

impl SecretType {
    pub fn as_str(self) -> &'static str {
        match self {
            SecretType::Eap => "eap",
            SecretType::Xauth => "xauth",
            SecretType::Ntlm => "ntlm",
            SecretType::Ike => "ike",
            SecretType::Ppk => "ppk",
            SecretType::Private => "private",
            SecretType::Rsa => "rsa",
            SecretType::Ecdsa => "ecdsa",
            SecretType::Pkcs8 => "pkcs8",
            SecretType::Pkcs12 => "pkcs12",
            SecretType::Token => "token",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "eap" => Some(SecretType::Eap),
            "xauth" => Some(SecretType::Xauth),
            "ntlm" => Some(SecretType::Ntlm),
            "ike" => Some(SecretType::Ike),
            "ppk" => Some(SecretType::Ppk),
            "private" => Some(SecretType::Private),
            "rsa" => Some(SecretType::Rsa),
            "ecdsa" => Some(SecretType::Ecdsa),
            "pkcs8" => Some(SecretType::Pkcs8),
            "pkcs12" => Some(SecretType::Pkcs12),
            "token" => Some(SecretType::Token),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SecretUpsertRequest {
    pub secret_type: SecretType,
    pub config: Value,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SecretCreateRequest {
    pub name: String,
    pub secret_type: SecretType,
    pub config: Value,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SecretResponse {
    pub name: String,
    pub secret_type: SecretType,
    pub config: Value,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SecretListResponse {
    pub secrets: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SecretCrudResponse {
    pub name: String,
    pub action: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CertificateKind {
    Ca,
    User,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CaCertificateCreateRequest {
    pub name: String,
    pub common_name: String,
    pub organization: Option<String>,
    pub country: Option<String>,
    pub days: Option<u32>,
    pub key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CaCertificateUpsertRequest {
    pub common_name: String,
    pub organization: Option<String>,
    pub country: Option<String>,
    pub days: Option<u32>,
    pub key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UserCertificateCreateRequest {
    pub name: String,
    pub ca_name: String,
    pub identity: String,
    pub san: Option<Vec<String>>,
    pub days: Option<u32>,
    pub key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UserCertificateUpsertRequest {
    pub ca_name: String,
    pub identity: String,
    pub san: Option<Vec<String>>,
    pub days: Option<u32>,
    pub key_size: Option<u32>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ConnectionCertificateAttachRequest {
    pub certificate_name: String,
    pub local_id: Option<String>,
    pub remote_ca_name: Option<String>,
    pub set_remote_auth_pubkey: Option<bool>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CertificateListResponse {
    pub certificates: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CertificateDetailsResponse {
    pub name: String,
    pub kind: CertificateKind,
    pub certificate_path: String,
    pub private_key_path: Option<String>,
    pub subject: Option<String>,
    pub issuer: Option<String>,
    pub not_after: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CertificateCrudResponse {
    pub name: String,
    pub kind: CertificateKind,
    pub action: String,
    pub success: bool,
    pub message: String,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub struct CaCertificateParams {
    pub common_name: String,
    pub organization: Option<String>,
    pub country: Option<String>,
    pub days: u32,
    pub key_size: u32,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub struct UserCertificateParams {
    pub ca_name: String,
    pub identity: String,
    pub san: Vec<String>,
    pub days: u32,
    pub key_size: u32,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub struct PeerRuntimeStatus {
    pub peer_name: String,
    pub phase1_state: String,
    pub phase1_active: bool,
    pub phase1_active_for_seconds: Option<u64>,
    pub phase1_packets_in: u64,
    pub phase1_packets_out: u64,
    pub child_sas: Vec<ChildRuntimeStatus>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone)]
pub struct ChildRuntimeStatus {
    pub name: String,
    pub state: String,
    pub active: bool,
    pub active_for_seconds: Option<u64>,
    pub packets_in: u64,
    pub packets_out: u64,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Default, Clone)]
pub struct FirewallRulesSnapshot {
    pub firewall: Vec<String>,
    pub filter: Vec<String>,
    pub nat: Vec<String>,
}
