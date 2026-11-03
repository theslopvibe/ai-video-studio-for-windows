use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::sync::Mutex;

pub struct Db(pub Mutex<Connection>);

#[derive(Serialize, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    /// Folder chosen by the user ("" = <output folder>/<project name>)
    pub folder: String,
    /// Style text appended to every prompt of this project
    pub suffix: String,
    /// Write a .txt with the prompt next to every video
    pub sidecar: bool,
    /// Resolved folder, filled in by the app (not stored)
    #[serde(default)]
    pub dir: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Job {
    pub id: String,
    pub project_id: String,
    pub workflow_id: String,
    pub prompt: String,
    pub params: String,
    /// queued | routing | generating | downloading | done | failed | cancelled
    pub status: String,
    pub message: String,
    pub error: String,
    pub file_path: String,
    pub created_at: i64,
    pub started_at: i64,
    pub finished_at: i64,
    pub est_cost: f64,
    pub gpu: String,
}

const JOB_COLS: &str = "id,project_id,workflow_id,prompt,params,status,message,error,file_path,created_at,started_at,finished_at,est_cost,gpu";

fn row_to_job(r: &rusqlite::Row) -> rusqlite::Result<Job> {
    Ok(Job {
        id: r.get(0)?,
        project_id: r.get(1)?,
        workflow_id: r.get(2)?,
        prompt: r.get(3)?,
        params: r.get(4)?,
        status: r.get(5)?,
        message: r.get(6)?,
        error: r.get(7)?,
        file_path: r.get(8)?,
        created_at: r.get(9)?,
        started_at: r.get(10)?,
        finished_at: r.get(11)?,
        est_cost: r.get(12)?,
        gpu: r.get(13)?,
    })
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl Db {
    pub fn open(path: &std::path::Path) -> Result<Db> {
        let c = Connection::open(path)?;
        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL,
               folder TEXT NOT NULL DEFAULT '', suffix TEXT NOT NULL DEFAULT '', sidecar INTEGER NOT NULL DEFAULT 1);
             CREATE TABLE IF NOT EXISTS jobs(
               id TEXT PRIMARY KEY, project_id TEXT NOT NULL, workflow_id TEXT NOT NULL,
               prompt TEXT NOT NULL, params TEXT NOT NULL DEFAULT '{}',
               status TEXT NOT NULL, message TEXT NOT NULL DEFAULT '', error TEXT NOT NULL DEFAULT '',
               file_path TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL,
               started_at INTEGER NOT NULL DEFAULT 0, finished_at INTEGER NOT NULL DEFAULT 0,
               est_cost REAL NOT NULL DEFAULT 0, gpu TEXT NOT NULL DEFAULT '');",
        )?;
        // Databases created by v0.1.0 lack the per-project columns.
        for col in [
            "ALTER TABLE projects ADD COLUMN folder TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE projects ADD COLUMN suffix TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE projects ADD COLUMN sidecar INTEGER NOT NULL DEFAULT 1",
        ] {
            let _ = c.execute(col, []);
        }
        // Jobs interrupted by an app restart go back to the queue.
        c.execute(
            "UPDATE jobs SET status='queued', message='' WHERE status IN ('routing','generating','downloading')",
            [],
        )?;
        let db = Db(Mutex::new(c));
        if db.projects()?.is_empty() {
            db.create_project("Default", "")?;
        }
        Ok(db)
    }

    pub fn kv_get(&self, key: &str) -> Option<String> {
        let c = self.0.lock().unwrap();
        c.query_row("SELECT value FROM kv WHERE key=?1", [key], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let c = self.0.lock().unwrap();
        c.execute(
            "INSERT INTO kv(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        let c = self.0.lock().unwrap();
        let mut s = c.prepare("SELECT id,name,created_at,folder,suffix,sidecar FROM projects ORDER BY created_at, rowid")?;
        let v = s
            .query_map([], |r| {
                Ok(Project {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    created_at: r.get(2)?,
                    folder: r.get(3)?,
                    suffix: r.get(4)?,
                    sidecar: r.get::<_, i64>(5)? != 0,
                    dir: String::new(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn create_project(&self, name: &str, folder: &str) -> Result<Project> {
        let p = Project {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            created_at: now(),
            folder: folder.trim().to_string(),
            suffix: String::new(),
            sidecar: true,
            dir: String::new(),
        };
        let c = self.0.lock().unwrap();
        c.execute(
            "INSERT INTO projects(id,name,created_at,folder,suffix,sidecar) VALUES(?1,?2,?3,?4,?5,1)",
            params![p.id, p.name, p.created_at, p.folder, p.suffix],
        )?;
        Ok(p)
    }

    pub fn update_project(&self, id: &str, name: &str, folder: &str, suffix: &str, sidecar: bool) -> Result<()> {
        self.0.lock().unwrap().execute(
            "UPDATE projects SET name=?2, folder=?3, suffix=?4, sidecar=?5 WHERE id=?1",
            params![id, name.trim(), folder.trim(), suffix.trim(), sidecar as i64],
        )?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> Result<()> {
        let c = self.0.lock().unwrap();
        c.execute("DELETE FROM jobs WHERE project_id=?1", [id])?;
        c.execute("DELETE FROM projects WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn insert_job(&self, j: &Job) -> Result<()> {
        let c = self.0.lock().unwrap();
        c.execute(
            &format!("INSERT INTO jobs({JOB_COLS}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)"),
            params![
                j.id, j.project_id, j.workflow_id, j.prompt, j.params, j.status, j.message, j.error,
                j.file_path, j.created_at, j.started_at, j.finished_at, j.est_cost, j.gpu
            ],
        )?;
        Ok(())
    }

    pub fn job(&self, id: &str) -> Option<Job> {
        let c = self.0.lock().unwrap();
        c.query_row(&format!("SELECT {JOB_COLS} FROM jobs WHERE id=?1"), [id], row_to_job)
            .optional()
            .ok()
            .flatten()
    }

    pub fn jobs(&self, project_id: Option<&str>) -> Result<Vec<Job>> {
        let c = self.0.lock().unwrap();
        let (sql, p): (String, Vec<String>) = match project_id {
            Some(p) => (
                format!("SELECT {JOB_COLS} FROM jobs WHERE project_id=?1 ORDER BY created_at DESC"),
                vec![p.to_string()],
            ),
            None => (format!("SELECT {JOB_COLS} FROM jobs ORDER BY created_at DESC"), vec![]),
        };
        let mut s = c.prepare(&sql)?;
        let v = s
            .query_map(rusqlite::params_from_iter(p.iter()), row_to_job)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn next_queued(&self) -> Option<Job> {
        let c = self.0.lock().unwrap();
        c.query_row(
            &format!("SELECT {JOB_COLS} FROM jobs WHERE status='queued' ORDER BY created_at, rowid LIMIT 1"),
            [],
            row_to_job,
        )
        .optional()
        .ok()
        .flatten()
    }

    /// All waiting jobs, oldest first.
    pub fn queued_all(&self) -> Vec<Job> {
        let c = self.0.lock().unwrap();
        let Ok(mut s) = c.prepare(&format!("SELECT {JOB_COLS} FROM jobs WHERE status='queued' ORDER BY created_at, rowid")) else { return vec![] };
        s.query_map([], row_to_job).map(|r| r.filter_map(|x| x.ok()).collect()).unwrap_or_default()
    }

    pub fn update_job(&self, id: &str, status: &str, message: &str, error: &str) -> Result<()> {
        let c = self.0.lock().unwrap();
        c.execute(
            "UPDATE jobs SET status=?2, message=?3, error=?4 WHERE id=?1",
            [id, status, message, error],
        )?;
        Ok(())
    }

    pub fn set_job_times(&self, id: &str, started: Option<i64>, finished: Option<i64>) -> Result<()> {
        let c = self.0.lock().unwrap();
        if let Some(s) = started {
            c.execute("UPDATE jobs SET started_at=?2 WHERE id=?1", params![id, s])?;
        }
        if let Some(f) = finished {
            c.execute("UPDATE jobs SET finished_at=?2 WHERE id=?1", params![id, f])?;
        }
        Ok(())
    }

    pub fn set_job_file(&self, id: &str, path: &str) -> Result<()> {
        self.0.lock().unwrap().execute("UPDATE jobs SET file_path=?2 WHERE id=?1", [id, path])?;
        Ok(())
    }

    pub fn delete_job(&self, id: &str) -> Result<()> {
        self.0.lock().unwrap().execute("DELETE FROM jobs WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn requeue_job(&self, id: &str) -> Result<()> {
        self.0.lock().unwrap().execute(
            "UPDATE jobs SET status='queued', message='', error='', created_at=?2 WHERE id=?1",
            params![id, now()],
        )?;
        Ok(())
    }
}
