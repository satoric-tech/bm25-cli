use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tantivy::schema::{
    IndexRecordOption, Schema, SchemaBuilder, TextFieldIndexing, TextOptions, FAST, STORED, STRING,
};
use tantivy::Index;

use crate::tokenizer::{self, TOKENIZER_NAME};

pub struct BM25Index {
    pub bm25_dir: PathBuf,
    pub index_dir: PathBuf,
    pub mtimes_path: PathBuf,
    pub sources_path: PathBuf,
    pub schema: Schema,
    pub field_path: tantivy::schema::Field,
    pub field_content: tantivy::schema::Field,
    pub field_mtime: tantivy::schema::Field,
}

impl BM25Index {
    pub fn open_global() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let bm25_dir = PathBuf::from(home).join(".bm25");
        Self::open_at(&bm25_dir)
    }

    pub fn open_at(bm25_dir: &Path) -> Result<Self> {
        let index_dir = bm25_dir.join("index");
        let mtimes_path = bm25_dir.join("mtimes.json");
        let sources_path = bm25_dir.join("sources.json");

        std::fs::create_dir_all(&index_dir)
            .with_context(|| format!("failed to create {}", index_dir.display()))?;

        let content_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer(TOKENIZER_NAME)
                    .set_index_option(IndexRecordOption::WithFreqs),
            )
            .set_stored();

        let mut builder = SchemaBuilder::new();
        let field_path = builder.add_text_field("path", STRING | STORED);
        let field_content = builder.add_text_field("content", content_options);
        let field_mtime = builder.add_u64_field("mtime", FAST);
        let schema = builder.build();

        Ok(Self {
            bm25_dir: bm25_dir.to_path_buf(),
            index_dir,
            mtimes_path,
            sources_path,
            schema,
            field_path,
            field_content,
            field_mtime,
        })
    }

    pub fn open_or_create_tantivy(&self) -> Result<Index> {
        let index = if self.index_dir.join("meta.json").exists() {
            match Index::open_in_dir(&self.index_dir) {
                Ok(index) if index.schema() == self.schema => index,
                _ => {
                    std::fs::remove_dir_all(&self.index_dir)
                        .context("failed to remove stale index")?;
                    std::fs::create_dir_all(&self.index_dir)
                        .context("failed to recreate index dir")?;
                    let _ = std::fs::remove_file(&self.mtimes_path);
                    Index::create_in_dir(&self.index_dir, self.schema.clone())
                        .context("failed to create tantivy index")?
                }
            }
        } else {
            Index::create_in_dir(&self.index_dir, self.schema.clone())
                .context("failed to create tantivy index")?
        };

        index
            .tokenizers()
            .register(TOKENIZER_NAME, tokenizer::build());
        Ok(index)
    }

    pub fn load_mtimes(&self) -> Result<HashMap<String, u64>> {
        if !self.mtimes_path.exists() {
            return Ok(HashMap::new());
        }
        let data = std::fs::read_to_string(&self.mtimes_path).context("failed to read mtimes")?;
        serde_json::from_str(&data).context("failed to parse mtimes")
    }

    pub fn save_mtimes(&self, mtimes: &HashMap<String, u64>) -> Result<()> {
        let data = serde_json::to_string(mtimes)?;
        std::fs::write(&self.mtimes_path, data).context("failed to write mtimes")
    }
}

pub fn file_mtime(path: &Path) -> Option<u64> {
    path.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}
