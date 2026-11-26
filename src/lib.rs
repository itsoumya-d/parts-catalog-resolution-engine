use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum CatalogError {
    #[error("Subsystem node '{0}' not found in assembly hierarchy")]
    SubsystemNotFound(String),
    #[error("No matching part identified for token '{0}'")]
    PartNotFound(String),
    #[error("Invalid confidence score '{0}'")]
    InvalidConfidence(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VehicleDescriptor {
    pub make: String,
    pub model: String,
    pub year: u32,
    pub vin_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartRecord {
    pub part_sku: String,
    pub oem_reference: String,
    pub description: String,
    pub category: String,
    pub year_start: u32,
    pub year_end: u32,
}

impl PartRecord {
    pub fn fits_year(&self, year: u32) -> bool {
        year >= self.year_start && year <= self.year_end
    }
}

pub struct PartsCatalog {
    parts: HashMap<String, PartRecord>,
}

impl PartsCatalog {
    pub fn new() -> Self {
        Self {
            parts: HashMap::new(),
        }
    }

    pub fn insert(&mut self, record: PartRecord) {
        self.parts.insert(record.part_sku.clone(), record);
    }

    pub fn get(&self, sku: &str) -> Option<&PartRecord> {
        self.parts.get(sku)
    }

    pub fn count(&self) -> usize {
        self.parts.len()
    }
}

impl Default for PartsCatalog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_insertion_and_lookup() {
        let mut catalog = PartsCatalog::new();
        let part = PartRecord {
            part_sku: "OEM-BRP-1001".to_string(),
            oem_reference: "47730-02200".to_string(),
            description: "Front Brake Caliper Assembly Left".to_string(),
            category: "Braking".to_string(),
            year_start: 2018,
            year_end: 2024,
        };

        catalog.insert(part.clone());
        assert_eq!(catalog.count(), 1);
        assert_eq!(catalog.get("OEM-BRP-1001"), Some(&part));
        assert!(part.fits_year(2021));
        assert!(!part.fits_year(2015));
    }
}
