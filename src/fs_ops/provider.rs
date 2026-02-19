#![allow(dead_code)]
use crate::fs_ops::scanner;
use async_trait::async_trait;
use gpui::Result;
use gpui::*;
use std::path::PathBuf;

use chrono::{DateTime, Local};
use humansize::{format_size, DECIMAL};

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: std::time::SystemTime,
    pub formatted_size: String,
    pub formatted_date: String,
}

#[async_trait]
pub trait FileSystemProvider: Send + Sync {
    async fn list_directory(
        &self,
        executor: BackgroundExecutor,
        path: PathBuf,
        show_hidden: bool,
    ) -> Result<Vec<FileEntry>>;
    async fn open(
        &self,
        executor: BackgroundExecutor,
        path: PathBuf,
        command: Option<String>,
    ) -> Result<()>;
    async fn delete(&self, executor: BackgroundExecutor, path: PathBuf) -> Result<()>;
    async fn rename(&self, executor: BackgroundExecutor, from: PathBuf, to: PathBuf) -> Result<()>;
    async fn copy(&self, executor: BackgroundExecutor, from: PathBuf, to: PathBuf) -> Result<()>;
    async fn create_dir(&self, executor: BackgroundExecutor, path: PathBuf) -> Result<()>;
    async fn search(
        &self,
        executor: BackgroundExecutor,
        path: PathBuf,
        query: String,
        options: scanner::SearchOptions,
    ) -> Result<Vec<FileEntry>>;
}

pub struct LocalFs;

#[async_trait]
impl FileSystemProvider for LocalFs {
    async fn list_directory(
        &self,
        executor: BackgroundExecutor,
        path: PathBuf,
        show_hidden: bool,
    ) -> Result<Vec<FileEntry>> {
        let scan_result = executor
            .spawn(async move { scanner::scan_dir(path, show_hidden) })
            .await;

        let entries = scan_result
            .files
            .into_iter()
            .map(|f| {
                let name = f
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Pre-compute formatted strings
                let formatted_date = DateTime::<Local>::from(f.modified)
                    .format("%Y-%m-%d %H:%M")
                    .to_string();

                let formatted_size = if f.is_dir {
                    "--".to_string()
                } else {
                    format_size(f.size, DECIMAL)
                };

                FileEntry {
                    path: f.path,
                    name,
                    is_dir: f.is_dir,
                    size: f.size,
                    modified: f.modified,
                    formatted_size,
                    formatted_date,
                }
            })
            .collect();
        Ok(entries)
    }

    async fn open(
        &self,
        executor: BackgroundExecutor,
        path: PathBuf,
        command: Option<String>,
    ) -> Result<()> {
        let cmd = command.unwrap_or_else(|| "xdg-open".to_string());
        let path_clone = path.clone();

        executor
            .spawn(async move {
                let _ = std::process::Command::new(cmd).arg(&path_clone).spawn();
            })
            .await;

        Ok(())
    }

    async fn delete(&self, executor: BackgroundExecutor, path: PathBuf) -> Result<()> {
        executor
            .spawn(async move { trash::delete(&path).map_err(|e| anyhow::anyhow!(e)) })
            .await
            .map_err(|e| anyhow::anyhow!(e).into())
    }

    async fn rename(&self, executor: BackgroundExecutor, from: PathBuf, to: PathBuf) -> Result<()> {
        executor
            .spawn(async move { std::fs::rename(from, to).map_err(|e| anyhow::anyhow!(e)) })
            .await
            .map_err(|e| anyhow::anyhow!(e).into())
    }

    async fn copy(&self, executor: BackgroundExecutor, from: PathBuf, to: PathBuf) -> Result<()> {
        executor
            .spawn(async move {
                if from.is_dir() {
                    let mut options = fs_extra::dir::CopyOptions::new();
                    options.copy_inside = true;
                    // fs_extra dir copy logic
                    fs_extra::dir::copy(&from, &to, &options).map_err(|e| anyhow::anyhow!(e))?;
                    Ok(())
                } else {
                    std::fs::copy(from, to)
                        .map(|_| ())
                        .map_err(|e| anyhow::anyhow!(e))
                }
            })
            .await
            .map_err(|e| anyhow::anyhow!(e).into())
    }

    async fn create_dir(&self, executor: BackgroundExecutor, path: PathBuf) -> Result<()> {
        executor
            .spawn(async move { std::fs::create_dir_all(path).map_err(|e| anyhow::anyhow!(e)) })
            .await
            .map_err(|e| anyhow::anyhow!(e).into())
    }

    async fn search(
        &self,
        executor: BackgroundExecutor,
        path: PathBuf,
        query: String,
        options: scanner::SearchOptions,
    ) -> Result<Vec<FileEntry>> {
        let scan_result = executor
            .spawn(async move { scanner::scan_recursive(path, query, options) })
            .await;

        let entries = scan_result
            .files
            .into_iter()
            .map(|f| {
                let name = f
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Pre-compute formatted strings
                let formatted_date = DateTime::<Local>::from(f.modified)
                    .format("%Y-%m-%d %H:%M")
                    .to_string();

                let formatted_size = if f.is_dir {
                    "--".to_string()
                } else {
                    format_size(f.size, DECIMAL)
                };

                FileEntry {
                    path: f.path,
                    name,
                    is_dir: f.is_dir,
                    size: f.size,
                    modified: f.modified,
                    formatted_size,
                    formatted_date,
                }
            })
            .collect();
        Ok(entries)
    }
}
