/*
 * Bifröst-Gate: Agente de monitoreo para StrongSwan.
 * Copyright (C) 2026 Estuardo Dardón.
 * * Este programa es software libre: puedes redistribuirlo y/o modificarlo
 * bajo los términos de la Licencia Pública General Affero de GNU tal como
 * fue publicada por la Free Software Foundation, ya sea la versión 3 de
 * la Licencia, o (a tu elección) cualquier versión posterior.
 */
 
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum NodeType {
    Gateway,
    RemoteEndpoint,
    Subnet,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum VpnStatus {
    Up,
    Down,
    Connecting,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NetworkNode {
    pub id: String,
    pub name: String,
    pub node_type: NodeType,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VpnEdge {
    pub from_node: String, // ID of the source NetworkNode
    pub to_node: String,   // ID of the target NetworkNode
    pub status: VpnStatus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BifrostTopology {
    pub nodes: Vec<NetworkNode>,
    pub edges: Vec<VpnEdge>,
}
