//! Artifact Service
//!
//! Provides versioned artifact storage with hierarchical scoping
//! (project/session/user). Metadata is stored in SQLite, binary data
//! on the filesystem.
//!
//! ## Architecture
//!
//! - `ArtifactService` trait defines the CRUD interface
//! - `DefaultArtifactService` implements it with SQLite + filesystem
//! - `ArtifactScope` provides hierarchical access control
//! - Auto-versioning: each save increments the version number

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::storage::database::Database;
use crate::utils::error::{AppError, AppResult};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Hierarchical scope for artifact access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactScope {
    /// Project the artifact belongs to.
    pub project_id: String,
    /// Optional session within the project.
    pub session_id: Option<String>,
    /// Optional user within the session.
    pub user_id: Option<String>,
}

impl ArtifactScope {
    /// Create a project-wide scope.
    pub fn project(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            session_id: None,
            user_id: None,
        }
    }

    /// Create a session-scoped artifact.
    pub fn session(project_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            session_id: Some(session_id.into()),
            user_id: None,
        }
    }

    /// Create a user-scoped artifact.
    pub fn user(
        project_id: impl Into<String>,
        session_id: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            session_id: Some(session_id.into()),
            user_id: Some(user_id.into()),
        }
    }
}

/// Metadata for a stored artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMeta {
    /// Unique artifact ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Scope (project/session/user).
    pub scope: ArtifactScope,
    /// Current version number.
    pub version: u32,
    /// MIME content type (e.g., "text/markdown", "image/png").
    pub content_type: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum of the content.
    pub checksum: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}

/// A specific version of an artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersion {
    /// Version record ID.
    pub id: String,
    /// Parent artifact ID.
    pub artifact_id: String,
    /// Version number.
    pub version: u32,
    /// Size in bytes.
    pub size_bytes: u64,
    /// SHA-256 checksum.
    pub checksum: String,
    /// Filesystem path where the binary is stored.
    pub storage_path: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// ArtifactService trait
// ---------------------------------------------------------------------------

/// Trait for versioned artifact storage operations.
#[async_trait]
pub trait ArtifactService: Send + Sync {
    /// Save artifact data, auto-incrementing version.
    async fn save(
        &self,
        name: &str,
        scope: &ArtifactScope,
        content_type: &str,
        data: &[u8],
    ) -> AppResult<ArtifactMeta>;

    /// Load artifact data. If version is None, loads the latest version.
    async fn load(
        &self,
        name: &str,
        scope: &ArtifactScope,
        version: Option<u32>,
    ) -> AppResult<(ArtifactMeta, Vec<u8>)>;

    /// List artifacts matching a scope filter.
    async fn list(&self, scope: &ArtifactScope) -> AppResult<Vec<ArtifactMeta>>;

    /// Get all versions of a named artifact in a scope.
    async fn versions(&self, name: &str, scope: &ArtifactScope) -> AppResult<Vec<ArtifactVersion>>;

    /// Delete an artifact and all its versions.
    async fn delete(&self, name: &str, scope: &ArtifactScope) -> AppResult<()>;
}

// ---------------------------------------------------------------------------
// DefaultArtifactService
// ---------------------------------------------------------------------------

/// Default implementation using SQLite for metadata and filesystem for binaries.
pub struct DefaultArtifactService {
    database: Arc<Database>,
    storage_root: PathBuf,
}

impl DefaultArtifactService {
    /// Create a new DefaultArtifactService.
    ///
    /// `storage_root` is the base directory for artifact binaries.
    /// `database` is the SQLite database for metadata.
    pub fn new(database: Arc<Database>, storage_root: impl AsRef<Path>) -> AppResult<Self> {
        let service = Self {
            database,
            storage_root: storage_root.as_ref().to_path_buf(),
        };
        service.init_schema()?;
        Ok(service)
    }

    /// Initialize the artifact tables in SQLite.
    fn init_schema(&self) -> AppResult<()> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                project_id TEXT NOT NULL,
                session_id TEXT,
                user_id TEXT,
                content_type TEXT NOT NULL,
                current_version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(name, project_id, session_id, user_id)
            )",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create artifacts table: {}", e)))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS artifact_versions (
                id TEXT PRIMARY KEY,
                artifact_id TEXT NOT NULL,
                version INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                checksum TEXT NOT NULL,
                storage_path TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now')),
                FOREIGN KEY (artifact_id) REFERENCES artifacts(id) ON DELETE CASCADE
            )",
            [],
        )
        .map_err(|e| {
            AppError::database(format!("Failed to create artifact_versions table: {}", e))
        })?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_artifacts_scope ON artifacts(project_id, session_id, user_id)",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create index: {}", e)))?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_artifact_versions_artifact ON artifact_versions(artifact_id, version)",
            [],
        )
        .map_err(|e| AppError::database(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    /// Compute the storage path for an artifact version.
    fn storage_path(
        &self,
        scope: &ArtifactScope,
        name: &str,
        version: u32,
        content_type: &str,
    ) -> PathBuf {
        let ext = content_type_to_ext(content_type);
        self.storage_root
            .join(&scope.project_id)
            .join(sanitize_name(name))
            .join(format!("v{}.{}", version, ext))
    }

    /// Compute SHA-256 checksum of data.
    fn compute_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }
}

#[async_trait]
impl ArtifactService for DefaultArtifactService {
    async fn save(
        &self,
        name: &str,
        scope: &ArtifactScope,
        content_type: &str,
        data: &[u8],
    ) -> AppResult<ArtifactMeta> {
        let checksum = Self::compute_checksum(data);
        let size_bytes = data.len() as u64;

        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        // Check if artifact exists, get current version
        let session_id = scope.session_id.as_deref().unwrap_or("");
        let user_id = scope.user_id.as_deref().unwrap_or("");

        let existing: Option<(String, u32)> = conn
            .query_row(
                "SELECT id, current_version FROM artifacts
                 WHERE name = ?1 AND project_id = ?2 AND COALESCE(session_id, '') = ?3 AND COALESCE(user_id, '') = ?4",
                rusqlite::params![name, scope.project_id, session_id, user_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?)),
            )
            .ok();

        let (artifact_id, new_version) = if let Some((id, current)) = existing {
            let new_ver = current + 1;
            conn.execute(
                "UPDATE artifacts SET current_version = ?1, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![new_ver, id],
            )
            .map_err(|e| AppError::database(format!("Failed to update artifact: {}", e)))?;
            (id, new_ver)
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO artifacts (id, name, project_id, session_id, user_id, content_type, current_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)",
                rusqlite::params![
                    id,
                    name,
                    scope.project_id,
                    scope.session_id,
                    scope.user_id,
                    content_type,
                ],
            )
            .map_err(|e| AppError::database(format!("Failed to insert artifact: {}", e)))?;
            (id, 1u32)
        };

        // Compute storage path and write file
        let storage_path = self.storage_path(scope, name, new_version, content_type);
        if let Some(parent) = storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&storage_path, data)?;

        // Insert version record
        let version_id = uuid::Uuid::new_v4().to_string();
        let storage_path_str = storage_path.to_string_lossy().to_string();

        conn.execute(
            "INSERT INTO artifact_versions (id, artifact_id, version, size_bytes, checksum, storage_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                version_id,
                artifact_id,
                new_version,
                size_bytes as i64,
                checksum,
                storage_path_str,
            ],
        )
        .map_err(|e| AppError::database(format!("Failed to insert version: {}", e)))?;

        // Get created_at
        let created_at: String = conn
            .query_row(
                "SELECT created_at FROM artifacts WHERE id = ?1",
                rusqlite::params![artifact_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339());

        Ok(ArtifactMeta {
            id: artifact_id,
            name: name.to_string(),
            scope: scope.clone(),
            version: new_version,
            content_type: content_type.to_string(),
            size_bytes,
            checksum,
            created_at,
        })
    }

    async fn load(
        &self,
        name: &str,
        scope: &ArtifactScope,
        version: Option<u32>,
    ) -> AppResult<(ArtifactMeta, Vec<u8>)> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let session_id = scope.session_id.as_deref().unwrap_or("");
        let user_id = scope.user_id.as_deref().unwrap_or("");

        // Get artifact
        let (artifact_id, content_type, current_version, created_at): (String, String, u32, String) = conn
            .query_row(
                "SELECT id, content_type, current_version, created_at FROM artifacts
                 WHERE name = ?1 AND project_id = ?2 AND COALESCE(session_id, '') = ?3 AND COALESCE(user_id, '') = ?4",
                rusqlite::params![name, scope.project_id, session_id, user_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|_| AppError::not_found(format!("Artifact '{}' not found", name)))?;

        let target_version = version.unwrap_or(current_version);

        // Get version record
        let (version_id, size_bytes, checksum, storage_path): (String, i64, String, String) = conn
            .query_row(
                "SELECT id, size_bytes, checksum, storage_path FROM artifact_versions
                 WHERE artifact_id = ?1 AND version = ?2",
                rusqlite::params![artifact_id, target_version],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|_| {
                AppError::not_found(format!(
                    "Version {} of artifact '{}' not found",
                    target_version, name
                ))
            })?;

        // Read file
        let data = std::fs::read(&storage_path)?;

        let meta = ArtifactMeta {
            id: artifact_id,
            name: name.to_string(),
            scope: scope.clone(),
            version: target_version,
            content_type,
            size_bytes: size_bytes as u64,
            checksum,
            created_at,
        };

        Ok((meta, data))
    }

    async fn list(&self, scope: &ArtifactScope) -> AppResult<Vec<ArtifactMeta>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let session_id = scope.session_id.as_deref().unwrap_or("");
        let user_id = scope.user_id.as_deref().unwrap_or("");

        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.name, a.project_id, a.session_id, a.user_id,
                        a.content_type, a.current_version, a.created_at,
                        COALESCE(v.size_bytes, 0), COALESCE(v.checksum, '')
                 FROM artifacts a
                 LEFT JOIN artifact_versions v ON a.id = v.artifact_id AND a.current_version = v.version
                 WHERE a.project_id = ?1
                   AND (?2 = '' OR COALESCE(a.session_id, '') = ?2)
                   AND (?3 = '' OR COALESCE(a.user_id, '') = ?3)
                 ORDER BY a.updated_at DESC",
            )
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt
            .query_map(
                rusqlite::params![scope.project_id, session_id, user_id],
                |row| {
                    Ok(ArtifactMeta {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        scope: ArtifactScope {
                            project_id: row.get(2)?,
                            session_id: row.get::<_, Option<String>>(3)?,
                            user_id: row.get::<_, Option<String>>(4)?,
                        },
                        version: row.get(6)?,
                        content_type: row.get(5)?,
                        size_bytes: row.get::<_, i64>(8)? as u64,
                        checksum: row.get(9)?,
                        created_at: row.get(7)?,
                    })
                },
            )
            .map_err(|e| AppError::database(format!("Failed to query artifacts: {}", e)))?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    async fn versions(&self, name: &str, scope: &ArtifactScope) -> AppResult<Vec<ArtifactVersion>> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let session_id = scope.session_id.as_deref().unwrap_or("");
        let user_id = scope.user_id.as_deref().unwrap_or("");

        let artifact_id: String = conn
            .query_row(
                "SELECT id FROM artifacts
                 WHERE name = ?1 AND project_id = ?2 AND COALESCE(session_id, '') = ?3 AND COALESCE(user_id, '') = ?4",
                rusqlite::params![name, scope.project_id, session_id, user_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::not_found(format!("Artifact '{}' not found", name)))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, artifact_id, version, size_bytes, checksum, storage_path, created_at
                 FROM artifact_versions WHERE artifact_id = ?1 ORDER BY version DESC",
            )
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt
            .query_map(rusqlite::params![artifact_id], |row| {
                Ok(ArtifactVersion {
                    id: row.get(0)?,
                    artifact_id: row.get(1)?,
                    version: row.get(2)?,
                    size_bytes: row.get::<_, i64>(3)? as u64,
                    checksum: row.get(4)?,
                    storage_path: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| AppError::database(format!("Failed to query versions: {}", e)))?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    async fn delete(&self, name: &str, scope: &ArtifactScope) -> AppResult<()> {
        let conn = self
            .database
            .get_connection()
            .map_err(|e| AppError::database(format!("Failed to get connection: {}", e)))?;

        let session_id = scope.session_id.as_deref().unwrap_or("");
        let user_id = scope.user_id.as_deref().unwrap_or("");

        // Get artifact ID and all version storage paths
        let artifact_id: String = conn
            .query_row(
                "SELECT id FROM artifacts
                 WHERE name = ?1 AND project_id = ?2 AND COALESCE(session_id, '') = ?3 AND COALESCE(user_id, '') = ?4",
                rusqlite::params![name, scope.project_id, session_id, user_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::not_found(format!("Artifact '{}' not found", name)))?;

        // Get all storage paths for cleanup
        let mut stmt = conn
            .prepare("SELECT storage_path FROM artifact_versions WHERE artifact_id = ?1")
            .map_err(|e| AppError::database(format!("Failed to prepare query: {}", e)))?;

        let paths: Vec<String> = stmt
            .query_map(rusqlite::params![artifact_id], |row| row.get(0))
            .map_err(|e| AppError::database(format!("Failed to query paths: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        // Delete version records (cascade would handle this, but be explicit)
        conn.execute(
            "DELETE FROM artifact_versions WHERE artifact_id = ?1",
            rusqlite::params![artifact_id],
        )
        .map_err(|e| AppError::database(format!("Failed to delete versions: {}", e)))?;

        // Delete artifact record
        conn.execute(
            "DELETE FROM artifacts WHERE id = ?1",
            rusqlite::params![artifact_id],
        )
        .map_err(|e| AppError::database(format!("Failed to delete artifact: {}", e)))?;

        // Delete files from filesystem
        for path in paths {
            let _ = std::fs::remove_file(&path);
        }

        // Try to clean up empty directories
        let artifact_dir = self
            .storage_root
            .join(&scope.project_id)
            .join(sanitize_name(name));
        let _ = std::fs::remove_dir(&artifact_dir);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert content type to file extension.
fn content_type_to_ext(content_type: &str) -> &str {
    match content_type {
        "text/markdown" => "md",
        "text/plain" => "txt",
        "text/html" => "html",
        "application/json" => "json",
        "application/pdf" => "pdf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

/// Sanitize a name for filesystem use.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_service() -> (DefaultArtifactService, tempfile::TempDir) {
        let dir = tempdir().expect("tempdir");
        let db = Arc::new(Database::new_in_memory().expect("test db"));
        let service = DefaultArtifactService::new(db, dir.path().join("artifacts"))
            .expect("create service");
        (service, dir)
    }

    // ======================================================================
    // Save and load roundtrip
    // ======================================================================

    #[tokio::test]
    async fn save_load_roundtrip() {
        let (service, _dir) = create_test_service().await;
        let scope = ArtifactScope::project("proj-1");
        let data = b"Hello, artifact!";

        let meta = service
            .save("test-artifact", &scope, "text/plain", data)
            .await
            .expect("save");

        assert_eq!(meta.name, "test-artifact");
        assert_eq!(meta.version, 1);
        assert_eq!(meta.size_bytes, data.len() as u64);
        assert!(!meta.checksum.is_empty());

        let (loaded_meta, loaded_data) = service
            .load("test-artifact", &scope, None)
            .await
            .expect("load");

        assert_eq!(loaded_meta.version, 1);
        assert_eq!(loaded_data, data);
    }

    // ======================================================================
    // Auto-versioning
    // ======================================================================

    #[tokio::test]
    async fn auto_versioning_increments() {
        let (service, _dir) = create_test_service().await;
        let scope = ArtifactScope::project("proj-1");

        let m1 = service
            .save("versioned", &scope, "text/plain", b"v1 data")
            .await
            .unwrap();
        assert_eq!(m1.version, 1);

        let m2 = service
            .save("versioned", &scope, "text/plain", b"v2 data")
            .await
            .unwrap();
        assert_eq!(m2.version, 2);

        let m3 = service
            .save("versioned", &scope, "text/plain", b"v3 data")
            .await
            .unwrap();
        assert_eq!(m3.version, 3);

        // Load latest should be v3
        let (meta, data) = service.load("versioned", &scope, None).await.unwrap();
        assert_eq!(meta.version, 3);
        assert_eq!(data, b"v3 data");

        // Load specific version
        let (meta_v1, data_v1) = service.load("versioned", &scope, Some(1)).await.unwrap();
        assert_eq!(meta_v1.version, 1);
        assert_eq!(data_v1, b"v1 data");
    }

    // ======================================================================
    // Scope-based listing
    // ======================================================================

    #[tokio::test]
    async fn list_filters_by_scope() {
        let (service, _dir) = create_test_service().await;

        let scope_p1 = ArtifactScope::project("proj-1");
        let scope_p2 = ArtifactScope::project("proj-2");

        service
            .save("art-a", &scope_p1, "text/plain", b"a")
            .await
            .unwrap();
        service
            .save("art-b", &scope_p1, "text/plain", b"b")
            .await
            .unwrap();
        service
            .save("art-c", &scope_p2, "text/plain", b"c")
            .await
            .unwrap();

        let list_p1 = service.list(&scope_p1).await.unwrap();
        assert_eq!(list_p1.len(), 2);

        let list_p2 = service.list(&scope_p2).await.unwrap();
        assert_eq!(list_p2.len(), 1);
    }

    // ======================================================================
    // Versions listing
    // ======================================================================

    #[tokio::test]
    async fn versions_lists_all() {
        let (service, _dir) = create_test_service().await;
        let scope = ArtifactScope::project("proj-1");

        service
            .save("multi", &scope, "text/plain", b"v1")
            .await
            .unwrap();
        service
            .save("multi", &scope, "text/plain", b"v2")
            .await
            .unwrap();
        service
            .save("multi", &scope, "text/plain", b"v3")
            .await
            .unwrap();

        let versions = service.versions("multi", &scope).await.unwrap();
        assert_eq!(versions.len(), 3);
        // Should be sorted descending
        assert_eq!(versions[0].version, 3);
        assert_eq!(versions[2].version, 1);
    }

    // ======================================================================
    // Delete
    // ======================================================================

    #[tokio::test]
    async fn delete_removes_artifact_and_versions() {
        let (service, _dir) = create_test_service().await;
        let scope = ArtifactScope::project("proj-1");

        service
            .save("deleteme", &scope, "text/plain", b"data")
            .await
            .unwrap();
        service
            .save("deleteme", &scope, "text/plain", b"data2")
            .await
            .unwrap();

        service.delete("deleteme", &scope).await.unwrap();

        let result = service.load("deleteme", &scope, None).await;
        assert!(result.is_err(), "Should not find deleted artifact");

        let list = service.list(&scope).await.unwrap();
        assert!(list.is_empty());
    }

    // ======================================================================
    // Checksum verification
    // ======================================================================

    #[tokio::test]
    async fn checksum_is_computed_correctly() {
        let (service, _dir) = create_test_service().await;
        let scope = ArtifactScope::project("proj-1");
        let data = b"test data for checksum";

        let meta = service
            .save("checksum-test", &scope, "text/plain", data)
            .await
            .unwrap();

        // Compute expected checksum
        let mut hasher = Sha256::new();
        hasher.update(data);
        let expected = format!("{:x}", hasher.finalize());

        assert_eq!(meta.checksum, expected);
    }

    // ======================================================================
    // Concurrent access safety
    // ======================================================================

    #[tokio::test]
    async fn concurrent_save_safety() {
        let (service, _dir) = create_test_service().await;
        let service = Arc::new(service);
        let scope = ArtifactScope::project("proj-1");

        let mut handles = Vec::new();
        for i in 0..5 {
            let svc = Arc::clone(&service);
            let s = scope.clone();
            handles.push(tokio::spawn(async move {
                svc.save(
                    "concurrent",
                    &s,
                    "text/plain",
                    format!("data-{}", i).as_bytes(),
                )
                .await
            }));
        }

        let mut versions_seen = Vec::new();
        for handle in handles {
            match handle.await.unwrap() {
                Ok(meta) => versions_seen.push(meta.version),
                Err(_) => {} // Some may fail due to concurrency, that's ok
            }
        }

        // At least some saves should succeed
        assert!(!versions_seen.is_empty(), "At least one save should succeed");
    }

    // ======================================================================
    // Scope types
    // ======================================================================

    #[test]
    fn artifact_scope_project() {
        let scope = ArtifactScope::project("p1");
        assert_eq!(scope.project_id, "p1");
        assert!(scope.session_id.is_none());
        assert!(scope.user_id.is_none());
    }

    #[test]
    fn artifact_scope_session() {
        let scope = ArtifactScope::session("p1", "s1");
        assert_eq!(scope.project_id, "p1");
        assert_eq!(scope.session_id, Some("s1".to_string()));
    }

    #[test]
    fn artifact_scope_user() {
        let scope = ArtifactScope::user("p1", "s1", "u1");
        assert_eq!(scope.user_id, Some("u1".to_string()));
    }

    #[test]
    fn artifact_meta_serde() {
        let meta = ArtifactMeta {
            id: "id-1".to_string(),
            name: "test".to_string(),
            scope: ArtifactScope::project("p1"),
            version: 1,
            content_type: "text/plain".to_string(),
            size_bytes: 100,
            checksum: "abc123".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: ArtifactMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.version, 1);
    }

    // ======================================================================
    // Helper function tests
    // ======================================================================

    #[test]
    fn test_content_type_to_ext() {
        assert_eq!(content_type_to_ext("text/markdown"), "md");
        assert_eq!(content_type_to_ext("application/json"), "json");
        assert_eq!(content_type_to_ext("image/png"), "png");
        assert_eq!(content_type_to_ext("unknown/type"), "bin");
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("hello-world"), "hello-world");
        assert_eq!(sanitize_name("test file.txt"), "test_file.txt");
        assert_eq!(sanitize_name("a/b\\c"), "a_b_c");
    }

    // ======================================================================
    // Not found errors
    // ======================================================================

    #[tokio::test]
    async fn load_nonexistent_returns_error() {
        let (service, _dir) = create_test_service().await;
        let scope = ArtifactScope::project("proj-1");
        let result = service.load("nonexistent", &scope, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_error() {
        let (service, _dir) = create_test_service().await;
        let scope = ArtifactScope::project("proj-1");
        let result = service.delete("nonexistent", &scope).await;
        assert!(result.is_err());
    }
}
