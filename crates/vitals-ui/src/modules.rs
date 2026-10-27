//! The seven modules of the panel, in navigation order.

use crate::i18n::Locale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Errors,
    Disks,
    Runners,
    Sensors,
    Startup,
    Updates,
    Network,
}

impl Module {
    pub const ALL: [Self; 7] = [
        Self::Errors,
        Self::Disks,
        Self::Runners,
        Self::Sensors,
        Self::Startup,
        Self::Updates,
        Self::Network,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Errors => "errors",
            Self::Disks => "disks",
            Self::Runners => "runners",
            Self::Sensors => "sensors",
            Self::Startup => "startup",
            Self::Updates => "updates",
            Self::Network => "network",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|module| module.id() == id)
    }

    pub const fn title(self, locale: Locale) -> &'static str {
        let texts = &locale.texts().modules;
        match self {
            Self::Errors => texts.errors,
            Self::Disks => texts.disks,
            Self::Runners => texts.runners,
            Self::Sensors => texts.sensors,
            Self::Startup => texts.startup,
            Self::Updates => texts.updates,
            Self::Network => texts.network,
        }
    }

    /// The stage of the work plan (the brief, §8) that brings the module
    pub const fn stage(self) -> u32 {
        match self {
            Self::Errors => 2,
            Self::Disks => 3,
            Self::Updates => 4,
            Self::Runners => 5,
            Self::Network => 6,
            Self::Startup => 7,
            Self::Sensors => 9,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_reads_back() {
        for module in Module::ALL {
            assert_eq!(Module::parse(module.id()), Some(module));
        }
        assert_eq!(Module::parse("nope"), None);
    }
}
