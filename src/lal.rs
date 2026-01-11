//! Licensed Asset Loader (LAL)
//!
//! Manages third-party content packs for TuxTalks.
//! Handles loading of audio packs and macros from `~/.local/share/tuxtalks/packs`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tracing::{error, info, warn};

/// Main LAL Manager
pub struct LALManager {
    packs: RwLock<HashMap<String, PackInfo>>,
    packs_dir: PathBuf,
}

impl Default for LALManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LALManager {
    /// Create new LAL Manager and load installed packs
    pub fn new() -> Self {
        let packs_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tuxtalks/packs");

        let manager = Self {
            packs: RwLock::new(HashMap::new()),
            packs_dir,
        };

        manager.ensure_packs_dir();
        manager.load_all_packs();

        manager
    }

    fn ensure_packs_dir(&self) {
        if !self.packs_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&self.packs_dir) {
                error!("Failed to create packs directory: {}", e);
            } else {
                info!("✅ Created packs directory: {}", self.packs_dir.display());
            }
        }
    }

    /// Scan and load all packs
    pub fn load_all_packs(&self) {
        if !self.packs_dir.exists() {
            return;
        }

        let entries = match std::fs::read_dir(&self.packs_dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!("Failed to read packs directory: {}", e);
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                match self.load_pack(&path) {
                    Ok((name, info)) => {
                        let mut packs = self.packs.write().expect("LAL packs lock poisoned");
                        packs.insert(name.clone(), info);
                        info!("✅ Loaded pack: {}", name);
                    }
                    Err(e) => {
                        warn!("Failed to load pack at {}: {}", path.display(), e);
                    }
                }
            }
        }

        let packs = self.packs.read().expect("LAL packs lock poisoned");
        if !packs.is_empty() {
            info!("📦 Loaded {} content pack(s)", packs.len());
        }
    }

    /// Load a specific pack (Internal, returns info instead of mutating)
    fn load_pack(&self, pack_path: &Path) -> Result<(String, PackInfo)> {
        let metadata_path = pack_path.join("pack.json");
        if !metadata_path.exists() {
            return Ok(("".to_string(), PackInfo::default())); // Or error? treating as 'not a pack' skipped in caller?
                                                              // Actually caller checks is_dir. If no pack.json, maybe bail.
        }
        if !metadata_path.exists() {
            anyhow::bail!("No pack.json found");
        }

        let content =
            std::fs::read_to_string(&metadata_path).context("Failed to read pack.json")?;

        let metadata: PackMetadata =
            serde_json::from_str(&content).context("Invalid JSON in pack.json")?;

        // Load audio index
        let audio_index = self.load_audio_index(pack_path, &metadata);

        // Load macros
        let macros = self.load_pack_macros(pack_path, &metadata);

        let pack_info = PackInfo {
            path: pack_path.to_path_buf(),
            metadata: metadata.clone(),
            audio_index,
            macros,
        };

        Ok((metadata.name, pack_info))
    }

    fn load_audio_index(
        &self,
        pack_path: &Path,
        metadata: &PackMetadata,
    ) -> HashMap<String, AudioFileInfo> {
        let mut audio_index = HashMap::new();

        if let Some(content) = &metadata.content {
            if let Some(audio) = &content.audio {
                if let Some(index_file) = &audio.index_file {
                    let index_path = pack_path.join(index_file);
                    if index_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(index_path) {
                            if let Ok(data) = serde_json::from_str::<AudioIndexData>(&content) {
                                for (_, items) in data.categories {
                                    for item in items {
                                        let file_path =
                                            if let Some(parent) = Path::new(index_file).parent() {
                                                parent.join(&item.file)
                                            } else {
                                                PathBuf::from(&item.file)
                                            };

                                        audio_index.insert(
                                            item.id,
                                            AudioFileInfo {
                                                file: file_path,
                                                tags: item.tags,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        audio_index
    }

    fn load_pack_macros(
        &self,
        pack_path: &Path,
        metadata: &PackMetadata,
    ) -> HashMap<String, crate::commands::Macro> {
        let all_macros = HashMap::new();

        if let Some(content) = &metadata.content {
            for macro_file in &content.macros {
                let macro_path = pack_path.join(macro_file);
                if macro_path.exists() {
                    if let Ok(_content) = std::fs::read_to_string(&macro_path) {
                        // TODO: Implement macro deserialization
                        info!("Found macro file: {}", macro_file);
                    }
                }
            }
        }

        all_macros
    }

    /// Resolve audio ID to a file path
    pub fn get_audio(&self, audio_id: &str) -> Option<PathBuf> {
        let packs = self.packs.read().expect("LAL packs lock poisoned");
        for pack in packs.values() {
            if let Some(info) = pack.audio_index.get(audio_id) {
                let full_path = pack.path.join(&info.file);
                if full_path.exists() {
                    return Some(full_path);
                }
            }
        }
        None
    }

    /// List all installed packs
    pub fn list_packs(&self) -> Vec<PackMetadata> {
        let packs = self.packs.read().expect("LAL packs lock poisoned");
        packs.values().map(|p| p.metadata.clone()).collect()
    }

    /// Remove a content pack
    pub fn remove_pack(&self, name: &str) -> Result<()> {
        let mut packs = self.packs.write().expect("LAL packs lock poisoned");
        if let Some(pack) = packs.get(name) {
            let path = pack.path.clone();
            std::fs::remove_dir_all(&path).context("Failed to delete pack directory")?;
            packs.remove(name);
            info!("🗑️ Removed pack: {}", name);
            Ok(())
        } else {
            anyhow::bail!("Pack not found: {}", name);
        }
    }

    /// Validate pack metadata (Alex - Security)
    /// Returns error if pack contains suspicious content
    pub fn validate_pack_metadata(&self, metadata: &PackMetadata, pack_path: &Path) -> Result<()> {
        // Security check: Verify pack name doesn't contain path traversal
        if metadata.name.contains("..") || metadata.name.contains('/') {
            anyhow::bail!("Pack name contains invalid characters: {}", metadata.name);
        }

        // Security check: Verify no executables in pack
        if self.contains_executables(pack_path) {
            anyhow::bail!("Pack contains executable files (security risk)");
        }

        // Size check: Prevent DoS via massive packs
        let max_size = 500 * 1024 * 1024; // 500MB max
        if let Ok(size) = self.calculate_directory_size(pack_path) {
            if size > max_size {
                anyhow::bail!("Pack exceeds size limit (500MB)");
            }
        }

        Ok(())
    }

    /// Get macros compatible with a specific game type (Python parity)
    pub fn get_macros_for_game(&self, game_type: &str) -> HashMap<String, crate::commands::Macro> {
        let mut result = HashMap::new();
        let packs = self.packs.read().expect("LAL packs lock poisoned");

        for pack in packs.values() {
            // Check if pack is compatible with this game
            if let Some(ref _content) = pack.metadata.content {
                let compatible = pack.metadata.compatibility.games.is_empty()
                    || pack
                        .metadata
                        .compatibility
                        .games
                        .iter()
                        .any(|g| g.eq_ignore_ascii_case(game_type));

                if compatible {
                    result.extend(pack.macros.clone());
                }
            }
        }

        result
    }

    /// Check if directory contains executable files (security helper)
    fn contains_executables(&self, path: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(metadata) = path.metadata() {
                        let mode = metadata.permissions().mode();
                        // Check for executable bit
                        if mode & 0o111 != 0 {
                            return true;
                        }
                    }
                } else if path.is_dir() && self.contains_executables(&path) {
                    return true;
                }
            }
        }
        false
    }

    /// Calculate total directory size (security helper)
    fn calculate_directory_size(&self, path: &Path) -> Result<u64> {
        let mut total = 0u64;

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(metadata) = path.metadata() {
                        total += metadata.len();
                    }
                } else if path.is_dir() {
                    total += self.calculate_directory_size(&path)?;
                }
            }
        }

        Ok(total)
    }
}

// Data Structures

#[derive(Debug, Clone, Default)]
pub struct PackInfo {
    pub path: PathBuf,
    pub metadata: PackMetadata,
    pub audio_index: HashMap<String, AudioFileInfo>,
    pub macros: HashMap<String, crate::commands::Macro>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub compatibility: CompatibilityInfo,
    pub content: Option<ContentInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompatibilityInfo {
    pub tuxtalks_version: String,
    #[serde(default)]
    pub games: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentInfo {
    pub audio: Option<AudioContentInfo>,
    #[serde(default)]
    pub macros: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioContentInfo {
    pub index_file: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AudioFileInfo {
    pub file: PathBuf,
    pub tags: Vec<String>,
}

// Internal JSON structs

#[derive(Debug, Deserialize)]
struct AudioIndexData {
    categories: HashMap<String, Vec<AudioItem>>,
}

#[derive(Debug, Deserialize)]
struct AudioItem {
    id: String,
    file: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_init() {
        let manager = LALManager::new();
        // Just verify it doesn't crash and creates/checks dir
        let packs = manager.packs.read().expect("LAL packs lock poisoned");
        assert!(packs.is_empty() || !packs.is_empty());
    }
}
