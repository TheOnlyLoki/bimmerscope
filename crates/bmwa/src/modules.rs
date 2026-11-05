//! Modules asked for fault memory, in scan order. `id` is the group file
//! name without `.grp` in the ECU directory. Missing group files are skipped.

pub struct Module {
    pub id: &'static str,
    pub name_de: &'static str,
    pub name_en: &'static str,
}

pub static MODULES: &[Module] = &[
    Module { id: "d_motor", name_de: "Motor", name_en: "Engine" },
    Module { id: "d_egs", name_de: "Getriebe", name_en: "Gearbox" },
    Module { id: "d_dsc", name_de: "Bremsregelung DSC", name_en: "Brakes DSC" },
    Module { id: "d_mrs", name_de: "Airbag", name_en: "Airbag" },
    Module { id: "d_kombi", name_de: "Kombiinstrument", name_en: "Instrument cluster" },
    Module { id: "d_cas", name_de: "Zugang/Wegfahrsperre CAS", name_en: "Access & immobiliser" },
    Module { id: "d_zgm", name_de: "Zentrales Gateway JBBF", name_en: "Body gateway" },
    Module { id: "d_klima", name_de: "Klimaanlage", name_en: "Air conditioning" },
    Module { id: "d_szl", name_de: "Lenkstockschalter", name_en: "Steering column switch" },
    Module { id: "d_lm", name_de: "Lichtmodul", name_en: "Light module" },
    Module { id: "d_pdc", name_de: "Parkhilfe", name_en: "Park distance" },
    Module { id: "d_ekp", name_de: "Kraftstoffpumpe", name_en: "Fuel pump" },
    Module { id: "d_ehc", name_de: "Niveauregelung", name_en: "Ride height" },
    Module { id: "d_cdc", name_de: "CD-Wechsler", name_en: "CD changer" },
    Module { id: "d_cid", name_de: "Display", name_en: "Display" },
    Module { id: "d_ccc", name_de: "Navigation", name_en: "Navigation" },
];

pub fn find(id: &str) -> Option<&'static Module> {
    MODULES.iter().find(|m| m.id == id)
}

/// Scan-order index, used to keep found modules in list order.
pub fn position(id: &str) -> usize {
    MODULES.iter().position(|m| m.id == id).unwrap_or(usize::MAX)
}
