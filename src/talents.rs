//! Arbre de talents : les eclats gagnes en combat s'echangent contre des ameliorations durables.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Talent {
    Vigor,
    Reach,
    Reflex,
    Edge,
    Breath,
    Magnet,
}

impl Talent {
    /// Un prerequis precede toujours ses talents dependants : la validation s'appuie sur cet ordre.
    pub const ALL: [Self; 6] = [
        Self::Vigor,
        Self::Reach,
        Self::Reflex,
        Self::Edge,
        Self::Breath,
        Self::Magnet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Vigor => "VIGUEUR",
            Self::Reach => "ALLONGE",
            Self::Reflex => "REFLEXES",
            Self::Edge => "TRANCHANT",
            Self::Breath => "SOUFFLE",
            Self::Magnet => "AIMANT",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Vigor => "+1 coeur maximum",
            Self::Reach => "+15% de portee des armes de melee",
            Self::Reflex => "+0,5 s d'invulnerabilite apres un coup",
            Self::Edge => "+1 degat pour toutes les armes",
            Self::Breath => "Recharge de la ruee reduite de 25%",
            Self::Magnet => "+50% d'eclats gagnes",
        }
    }

    pub fn cost(self) -> u32 {
        match self {
            Self::Vigor => 10,
            Self::Reach => 8,
            Self::Reflex => 10,
            Self::Edge => 16,
            Self::Breath => 12,
            Self::Magnet => 12,
        }
    }

    pub fn requires(self) -> Option<Self> {
        match self {
            Self::Edge => Some(Self::Reach),
            Self::Breath => Some(Self::Reflex),
            Self::Magnet => Some(Self::Vigor),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalentState {
    Owned,
    Available,
    Locked,
    TooExpensive,
}

/// Eclats gagnes par ennemi ordinaire, avant le bonus de l'Aimant.
pub const SHARDS_PER_KILL: u32 = 2;
pub const SHARDS_PER_BOSS: u32 = 10;
pub const MAX_SHARDS: u32 = 9999;

/// Applique le bonus de l'Aimant, arrondi a l'entier superieur.
pub fn shards_gained(base: u32, magnet: bool) -> u32 {
    if magnet {
        (base * 3).div_ceil(2)
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prerequisites_come_first_and_costs_are_positive() {
        for (index, talent) in Talent::ALL.iter().enumerate() {
            assert!(talent.cost() > 0 && !talent.name().is_empty());
            if let Some(required) = talent.requires() {
                let position = Talent::ALL.iter().position(|other| *other == required);
                assert!(position.is_some_and(|position| position < index));
            }
        }
        let json = serde_json::to_string(&Talent::Magnet).unwrap();
        assert_eq!(json, "\"magnet\"");
    }

    #[test]
    fn magnet_adds_half_rounded_up() {
        assert_eq!(shards_gained(2, false), 2);
        assert_eq!(shards_gained(2, true), 3);
        assert_eq!(shards_gained(10, true), 15);
        assert_eq!(shards_gained(1, true), 2);
    }
}
