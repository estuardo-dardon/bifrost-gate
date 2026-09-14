/*
 * Bifröst-Gate: Tipos y validaciones de JWT.
 * Copyright (C) 2026 Estuardo Dardón.
 */

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    pub sub: String,
    pub scopes: Vec<String>,
    pub exp: usize,
}

impl JwtClaims {
    pub fn has_scope(&self, required: &str) -> bool {
        if self.scopes.iter().any(|s| s == "*") {
            return true;
        }
        self.scopes.iter().any(|s| s == required)
    }
}
