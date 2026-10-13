//! The game's own `Options.ini` (graphics and mouse settings), which lives in the user's Documents folder.
//!
//! Only the keys the options window knows are touched; every other line (volumes, network addresses, keys added by
//! mods or tools) is written back exactly as it was.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::game::Game;

/// `<Documents>/Command and Conquer Generals Zero Hour Data/Options.ini` (or the Generals folder).
pub fn path_for(documents: &Path, game: Game) -> PathBuf {
    let folder = match game {
        Game::Zh => "Command and Conquer Generals Zero Hour Data",
        Game::Generals => "Command and Conquer Generals Data",
    };
    documents.join(folder).join("Options.ini")
}

/// The lines of an ini file, edited in place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ini {
    lines: Vec<String>,
    crlf: bool,
}

impl Ini {
    pub fn parse(text: &str) -> Self {
        let text = text.trim_start_matches('\u{feff}');
        Ini { crlf: text.contains("\r\n"), lines: text.lines().map(str::to_string).collect() }
    }

    fn key_of(line: &str) -> Option<&str> {
        let (k, _) = line.split_once('=')?;
        let k = k.trim();
        (!k.is_empty()).then_some(k)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().find(|l| Self::key_of(l).is_some_and(|k| k.eq_ignore_ascii_case(key))).and_then(|l| l.split_once('=')).map(|(_, v)| v.trim())
    }

    /// Change the value of `key` (keeping its place in the file), or add it at the end.
    pub fn set(&mut self, key: &str, value: &str) {
        let line = format!("{key} = {value}");
        match self.lines.iter_mut().find(|l| Self::key_of(l).is_some_and(|k| k.eq_ignore_ascii_case(key))) {
            Some(l) => *l = line,
            None => self.lines.push(line),
        }
    }

    pub fn render(&self) -> String {
        let nl = if self.crlf { "\r\n" } else { "\n" };
        let mut s = self.lines.join(nl);
        s.push_str(nl);
        s
    }
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

/// What the options window edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Graphics {
    pub width: u32,
    pub height: u32,
    /// 0 = full size textures, 1 = half, 2 = quarter.
    pub texture_reduction: u8,
    pub max_particle_count: u32,
    pub shadow_volumes: bool,
    pub shadow_decals: bool,
    pub building_occlusion: bool,
    pub show_trees: bool,
    pub cloud_map: bool,
    pub light_map: bool,
    pub soft_water_edge: bool,
    pub heat_effects: bool,
    pub extra_animations: bool,
    pub dynamic_lod: bool,
    pub alternate_mouse: bool,
}

impl Default for Graphics {
    /// What the game itself starts with.
    fn default() -> Self {
        Graphics {
            width: 800,
            height: 600,
            texture_reduction: 0,
            max_particle_count: 2500,
            shadow_volumes: true,
            shadow_decals: true,
            building_occlusion: true,
            show_trees: true,
            cloud_map: true,
            light_map: true,
            soft_water_edge: true,
            heat_effects: true,
            extra_animations: true,
            dynamic_lod: true,
            alternate_mouse: false,
        }
    }
}

impl Graphics {
    /// Light settings that run well everywhere, at the screen's own size (what 1.x offered on first start).
    pub fn recommended(width: u32, height: u32) -> Self {
        Graphics {
            width,
            height,
            texture_reduction: 0,
            max_particle_count: 2500,
            shadow_volumes: false,
            shadow_decals: true,
            building_occlusion: true,
            show_trees: true,
            cloud_map: false,
            light_map: true,
            soft_water_edge: true,
            heat_effects: false,
            extra_animations: true,
            dynamic_lod: true,
            alternate_mouse: false,
        }
    }

    /// Read what the file says; anything missing or unreadable keeps the game's default.
    pub fn from_ini(ini: &Ini) -> Self {
        let d = Graphics::default();
        let flag = |key: &str, fallback: bool| match ini.get(key).map(str::to_ascii_lowercase).as_deref() {
            Some("yes" | "true" | "1") => true,
            Some("no" | "false" | "0") => false,
            _ => fallback,
        };
        let (width, height) = ini
            .get("Resolution")
            .and_then(|r| {
                let mut p = r.split_whitespace().map(|n| n.parse::<u32>().ok());
                Some((p.next()??, p.next()??))
            })
            .filter(|&(w, h)| w >= 320 && h >= 200)
            .unwrap_or((d.width, d.height));
        Graphics {
            width,
            height,
            texture_reduction: ini.get("TextureReduction").and_then(|v| v.parse::<u8>().ok()).filter(|v| *v <= 2).unwrap_or(d.texture_reduction),
            max_particle_count: ini.get("MaxParticleCount").and_then(|v| v.parse().ok()).unwrap_or(d.max_particle_count),
            shadow_volumes: flag("UseShadowVolumes", d.shadow_volumes),
            shadow_decals: flag("UseShadowDecals", d.shadow_decals),
            building_occlusion: flag("BuildingOcclusion", d.building_occlusion),
            show_trees: flag("ShowTrees", d.show_trees),
            cloud_map: flag("UseCloudMap", d.cloud_map),
            light_map: flag("UseLightMap", d.light_map),
            soft_water_edge: flag("ShowSoftWaterEdge", d.soft_water_edge),
            heat_effects: flag("HeatEffects", d.heat_effects),
            extra_animations: flag("ExtraAnimations", d.extra_animations),
            dynamic_lod: flag("DynamicLOD", d.dynamic_lod),
            alternate_mouse: flag("UseAlternateMouse", d.alternate_mouse),
        }
    }

    /// Check the values are ones the game can use.
    pub fn validate(&self) -> Result<()> {
        if !(320..=16384).contains(&self.width) || !(200..=16384).contains(&self.height) {
            return Err(Error::msg("The resolution must be between 320x200 and 16384x16384"));
        }
        if self.texture_reduction > 2 {
            return Err(Error::msg("Texture quality must be High, Medium or Low"));
        }
        if !(100..=20000).contains(&self.max_particle_count) {
            return Err(Error::msg("The particle limit must be between 100 and 20000"));
        }
        Ok(())
    }

    pub fn write_to(&self, ini: &mut Ini) {
        ini.set("Resolution", &format!("{} {}", self.width, self.height));
        ini.set("TextureReduction", &self.texture_reduction.to_string());
        ini.set("MaxParticleCount", &self.max_particle_count.to_string());
        ini.set("UseShadowVolumes", yes_no(self.shadow_volumes));
        ini.set("UseShadowDecals", yes_no(self.shadow_decals));
        ini.set("BuildingOcclusion", yes_no(self.building_occlusion));
        ini.set("ShowTrees", yes_no(self.show_trees));
        ini.set("UseCloudMap", yes_no(self.cloud_map));
        ini.set("UseLightMap", yes_no(self.light_map));
        ini.set("ShowSoftWaterEdge", yes_no(self.soft_water_edge));
        ini.set("HeatEffects", yes_no(self.heat_effects));
        ini.set("ExtraAnimations", yes_no(self.extra_animations));
        ini.set("DynamicLOD", yes_no(self.dynamic_lod));
        ini.set("UseAlternateMouse", yes_no(self.alternate_mouse));
    }
}

/// The options as they are on disk (the game's defaults when there is no file yet).
pub fn load(file: &Path) -> Result<(bool, Graphics)> {
    match fs::read_to_string(file) {
        Ok(text) => Ok((true, Graphics::from_ini(&Ini::parse(&text)))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((false, Graphics::default())),
        Err(e) => Err(e.into()),
    }
}

/// Write `graphics` into the file, creating it (and its folder) when the game has not run yet.
/// The previous file is kept once as `Options.ini.bak`.
pub fn save(file: &Path, graphics: &Graphics) -> Result<()> {
    graphics.validate()?;
    let mut ini = match fs::read_to_string(file) {
        Ok(text) => {
            let bak = file.with_extension("ini.bak");
            if !bak.exists() {
                let _ = fs::write(&bak, &text);
            }
            Ini::parse(&text)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ini::default(),
        Err(e) => return Err(e.into()),
    };
    graphics.write_to(&mut ini);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("ini.tmp");
    fs::write(&tmp, ini.render())?;
    fs::rename(&tmp, file)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "AntiAliasing = 14\r\nResolution = 1280 720\r\nMusicVolume = 0\r\nUseShadowVolumes = no\r\nTextureReduction = 1\r\nHeatEffects = yes\r\n";

    #[test]
    fn reads_what_the_file_says_and_defaults_the_rest() {
        let g = Graphics::from_ini(&Ini::parse(SAMPLE));
        assert_eq!((g.width, g.height), (1280, 720));
        assert_eq!(g.texture_reduction, 1);
        assert!(!g.shadow_volumes && g.heat_effects);
        assert!(g.show_trees, "missing keys keep the game's default");
        let broken = Graphics::from_ini(&Ini::parse("Resolution = wide\nTextureReduction = 9\nShowTrees = maybe\n"));
        assert_eq!(broken, Graphics::default());
    }

    #[test]
    fn writing_changes_only_our_keys_and_keeps_the_rest_untouched() {
        let mut ini = Ini::parse(SAMPLE);
        let g = Graphics { width: 1920, height: 1080, show_trees: false, ..Graphics::from_ini(&ini) };
        g.write_to(&mut ini);
        let out = ini.render();
        assert!(out.starts_with("AntiAliasing = 14\r\nResolution = 1920 1080\r\nMusicVolume = 0\r\n"), "{out:?}");
        assert!(out.contains("ShowTrees = no\r\n") && out.contains("UseShadowVolumes = no\r\n"));
        assert!(out.ends_with("\r\n") && !out.contains("\r\n\r\n"));
        assert_eq!(Graphics::from_ini(&Ini::parse(&out)), g);
    }

    #[test]
    fn keys_match_regardless_of_case_and_spacing() {
        let mut ini = Ini::parse("resolution=800 600\n");
        assert_eq!(ini.get("Resolution"), Some("800 600"));
        ini.set("Resolution", "1024 768");
        assert_eq!(ini.render(), "Resolution = 1024 768\n");
    }

    #[test]
    fn save_creates_the_file_and_backs_up_an_existing_one_once() {
        let d = tempfile::tempdir().unwrap();
        let file = path_for(d.path(), Game::Zh);
        assert_eq!(load(&file).unwrap(), (false, Graphics::default()));
        save(&file, &Graphics::recommended(1920, 1080)).unwrap();
        let (exists, g) = load(&file).unwrap();
        assert!(exists && g.width == 1920 && !g.cloud_map);

        let original = "MusicVolume = 7\n";
        fs::write(&file, original).unwrap();
        save(&file, &Graphics { show_trees: false, ..Graphics::default() }).unwrap();
        save(&file, &Graphics::default()).unwrap(); // the second save must not overwrite the backup
        assert_eq!(fs::read_to_string(file.with_extension("ini.bak")).unwrap(), original);
        assert!(fs::read_to_string(&file).unwrap().contains("MusicVolume = 7"));
        assert!(!file.with_extension("ini.tmp").exists());
    }

    #[test]
    fn nonsense_values_are_refused() {
        let d = tempfile::tempdir().unwrap();
        let file = path_for(d.path(), Game::Generals);
        assert!(file.to_string_lossy().contains("Generals Data"));
        assert!(save(&file, &Graphics { width: 10, ..Default::default() }).is_err());
        assert!(save(&file, &Graphics { texture_reduction: 3, ..Default::default() }).is_err());
        assert!(save(&file, &Graphics { max_particle_count: 5, ..Default::default() }).is_err());
        assert!(!file.exists());
    }
}
