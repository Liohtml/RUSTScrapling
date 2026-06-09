use std::path::PathBuf;

pub struct CheckpointManager {
    checkpoint_dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CheckpointData {
    pub pending_urls: Vec<String>,
    pub seen_fingerprints: Vec<String>,
    pub items_count: u64,
}

impl CheckpointManager {
    pub fn new(dir: &str) -> Result<Self, std::io::Error> {
        let path = PathBuf::from(dir);
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            checkpoint_dir: path,
        })
    }

    pub fn save(&self, data: &CheckpointData) -> Result<(), std::io::Error> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let json = serde_json::to_string_pretty(data).map_err(std::io::Error::other)?;
        // Write atomically: write to a .tmp file then rename so a crash
        // mid-write cannot corrupt or zero-out an existing checkpoint.
        let tmp_path = self.checkpoint_dir.join("checkpoint.json.tmp");
        std::fs::write(&tmp_path, &json)?;
        std::fs::rename(&tmp_path, &file_path)
    }

    pub fn restore(&self) -> Option<CheckpointData> {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let data = std::fs::read_to_string(&file_path).ok()?;
        serde_json::from_str(&data).ok()
    }

    pub fn cleanup(&self) {
        let file_path = self.checkpoint_dir.join("checkpoint.json");
        let _ = std::fs::remove_file(file_path);
    }

    pub fn exists(&self) -> bool {
        self.checkpoint_dir.join("checkpoint.json").exists()
    }
}
