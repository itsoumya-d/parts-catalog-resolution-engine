use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum CatalogError {
    #[error("Subsystem node '{0}' not found in assembly hierarchy")]
    SubsystemNotFound(String),
    #[error("No matching part identified for query '{0}'")]
    PartNotFound(String),
    #[error("Part '{0}' does not fit vehicle year {1}")]
    FitmentMismatch(String, u32),
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InterchangeGrade {
    DirectOem,
    Tier1Supplier,
    CertifiedAftermarket,
}

impl InterchangeGrade {
    pub fn base_confidence(&self) -> f64 {
        match self {
            Self::DirectOem => 1.0,
            Self::Tier1Supplier => 0.95,
            Self::CertifiedAftermarket => 0.88,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterchangeRecord {
    pub source_oem: String,
    pub replacement_sku: String,
    pub grade: InterchangeGrade,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolutionCandidate {
    pub resolved_sku: String,
    pub oem_reference: String,
    pub confidence_score: f64,
    pub rationale: String,
}

pub struct ResolutionEngine {
    parts: HashMap<String, PartRecord>,
    assembly_subsystems: HashMap<String, Vec<String>>,
    interchanges: HashMap<String, Vec<InterchangeRecord>>,
}

impl ResolutionEngine {
    pub fn new() -> Self {
        Self {
            parts: HashMap::new(),
            assembly_subsystems: HashMap::new(),
            interchanges: HashMap::new(),
        }
    }

    pub fn register_part(&mut self, record: PartRecord) {
        self.parts.insert(record.part_sku.clone(), record);
    }

    pub fn register_subsystem_mapping(&mut self, subsystem: &str, part_sku: &str) {
        self.assembly_subsystems
            .entry(subsystem.to_lowercase())
            .or_default()
            .push(part_sku.to_string());
    }

    pub fn register_interchange(&mut self, interchange: InterchangeRecord) {
        self.interchanges
            .entry(interchange.source_oem.clone())
            .or_default()
            .push(interchange);
    }

    pub fn resolve(
        &self,
        vehicle: &VehicleDescriptor,
        query_token: &str,
    ) -> Result<Vec<ResolutionCandidate>, CatalogError> {
        let normalized = query_token.trim().to_lowercase();
        let mut candidates = Vec::new();

        if let Some(direct_part) = self.parts.values().find(|p| {
            p.part_sku.to_lowercase() == normalized
                || p.oem_reference.to_lowercase() == normalized
        }) {
            if direct_part.fits_year(vehicle.year) {
                candidates.push(ResolutionCandidate {
                    resolved_sku: direct_part.part_sku.clone(),
                    oem_reference: direct_part.oem_reference.clone(),
                    confidence_score: 1.0,
                    rationale: format!(
                        "Direct OEM match for {} {} ({})",
                        vehicle.make, vehicle.model, vehicle.year
                    ),
                });
            } else {
                return Err(CatalogError::FitmentMismatch(
                    direct_part.part_sku.clone(),
                    vehicle.year,
                ));
            }

            if let Some(cross_refs) = self.interchanges.get(&direct_part.oem_reference) {
                for cr in cross_refs {
                    if let Some(target_part) = self.parts.get(&cr.replacement_sku) {
                        if target_part.fits_year(vehicle.year) {
                            candidates.push(ResolutionCandidate {
                                resolved_sku: target_part.part_sku.clone(),
                                oem_reference: direct_part.oem_reference.clone(),
                                confidence_score: cr.grade.base_confidence(),
                                rationale: format!("Interchange via {}: {}", cr.source_oem, cr.notes),
                            });
                        }
                    }
                }
            }
        } else if let Some(skus) = self.assembly_subsystems.get(&normalized) {
            for sku in skus {
                if let Some(part) = self.parts.get(sku) {
                    if part.fits_year(vehicle.year) {
                        candidates.push(ResolutionCandidate {
                            resolved_sku: part.part_sku.clone(),
                            oem_reference: part.oem_reference.clone(),
                            confidence_score: 0.92,
                            rationale: format!("Subsystem assembly resolution for '{}'", query_token),
                        });
                    }
                }
            }
        }

        if candidates.is_empty() {
            return Err(CatalogError::PartNotFound(query_token.to_string()));
        }

        candidates.sort_by(|a, b| {
            b.confidence_score
                .partial_cmp(&a.confidence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(candidates)
    }
}

impl Default for ResolutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vehicle() -> VehicleDescriptor {
        VehicleDescriptor {
            make: "Toyota".to_string(),
            model: "Corolla".to_string(),
            year: 2022,
            vin_prefix: Some("JTNB1".to_string()),
        }
    }

    #[test]
    fn test_direct_oem_resolution() {
        let mut engine = ResolutionEngine::new();
        let part = PartRecord {
            part_sku: "OEM-BRP-1001".to_string(),
            oem_reference: "47730-02200".to_string(),
            description: "Front Left Brake Caliper".to_string(),
            category: "Braking".to_string(),
            year_start: 2019,
            year_end: 2024,
        };
        engine.register_part(part);

        let vehicle = sample_vehicle();
        let res = engine.resolve(&vehicle, "47730-02200").unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].resolved_sku, "OEM-BRP-1001");
        assert_eq!(res[0].confidence_score, 1.0);
    }

    #[test]
    fn test_interchange_resolution_ranking() {
        let mut engine = ResolutionEngine::new();
        let oem_part = PartRecord {
            part_sku: "OEM-BRP-1001".to_string(),
            oem_reference: "47730-02200".to_string(),
            description: "Front Left Brake Caliper OEM".to_string(),
            category: "Braking".to_string(),
            year_start: 2019,
            year_end: 2024,
        };
        let aft_part = PartRecord {
            part_sku: "AFT-BRK-5520".to_string(),
            oem_reference: "AFT-47730".to_string(),
            description: "Performance Brake Caliper Aftermarket".to_string(),
            category: "Braking".to_string(),
            year_start: 2020,
            year_end: 2025,
        };
        engine.register_part(oem_part);
        engine.register_part(aft_part);

        engine.register_interchange(InterchangeRecord {
            source_oem: "47730-02200".to_string(),
            replacement_sku: "AFT-BRK-5520".to_string(),
            grade: InterchangeGrade::CertifiedAftermarket,
            notes: "Direct bolt-on aftermarket replacement".to_string(),
        });

        let vehicle = sample_vehicle();
        let res = engine.resolve(&vehicle, "47730-02200").unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].resolved_sku, "OEM-BRP-1001");
        assert_eq!(res[0].confidence_score, 1.0);
        assert_eq!(res[1].resolved_sku, "AFT-BRK-5520");
        assert_eq!(res[1].confidence_score, 0.88);
    }

    #[test]
    fn test_subsystem_fallback_resolution() {
        let mut engine = ResolutionEngine::new();
        let part = PartRecord {
            part_sku: "OEM-SUS-901".to_string(),
            oem_reference: "48820-47010".to_string(),
            description: "Front Stabilizer Link".to_string(),
            category: "Suspension".to_string(),
            year_start: 2018,
            year_end: 2024,
        };
        engine.register_part(part);
        engine.register_subsystem_mapping("front stabilizer", "OEM-SUS-901");

        let vehicle = sample_vehicle();
        let res = engine.resolve(&vehicle, "front stabilizer").unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].resolved_sku, "OEM-SUS-901");
        assert_eq!(res[0].confidence_score, 0.92);
    }

    #[test]
    fn test_fitment_mismatch_error() {
        let mut engine = ResolutionEngine::new();
        let old_part = PartRecord {
            part_sku: "OEM-OLD-001".to_string(),
            oem_reference: "47730-01000".to_string(),
            description: "Classic Caliper".to_string(),
            category: "Braking".to_string(),
            year_start: 2005,
            year_end: 2012,
        };
        engine.register_part(old_part);

        let vehicle = sample_vehicle(); // year 2022
        let err = engine.resolve(&vehicle, "47730-01000").unwrap_err();
        assert_eq!(
            err,
            CatalogError::FitmentMismatch("OEM-OLD-001".to_string(), 2022)
        );
    }
}
