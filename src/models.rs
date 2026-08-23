use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub unit_value: f64,
}

pub struct UserRecord {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
}

#[derive(Clone)]
pub struct HoldingPosition {
    pub id: i64,
    pub quantity: f64,
    pub asset_id: i64,
    pub name: String,
    pub unit_value: f64,
}

impl HoldingPosition {
    pub fn subtotal(&self) -> f64 {
        self.quantity * self.unit_value
    }
}
