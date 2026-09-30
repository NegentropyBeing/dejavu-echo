use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("não consegui ler o arquivo de segredos: {0}")]
    Io(#[from] std::io::Error),
    #[error("o arquivo de segredos está corrompido")]
    Parse,
}

const FILE_NAME: &str = ".secret_env";

/// Segredos ficam num arquivo oculto na pasta de configuração do aplicativo,
/// nunca dentro da pasta do projeto, para não irem para o Git por acidente.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Secrets {
    pub discord_webhook_url: Option<String>,
}

pub fn path_in(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

pub fn read(dir: &Path) -> Result<Secrets, SecretsError> {
    let p = path_in(dir);
    let raw = match std::fs::read_to_string(&p) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Secrets::default()),
        Err(e) => return Err(e.into()),
    };

    let mut webhook = None;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() == "DISCORD_WEBHOOK_URL" {
            let v = v.trim();
            if !v.is_empty() {
                webhook = Some(v.to_string());
            }
        }
    }
    Ok(Secrets {
        discord_webhook_url: webhook,
    })
}

pub fn write(dir: &Path, secrets: &Secrets) -> Result<(), SecretsError> {
    std::fs::create_dir_all(dir)?;
    let mut body = String::new();
    if let Some(url) = &secrets.discord_webhook_url {
        body.push_str(&format!("DISCORD_WEBHOOK_URL={url}\n"));
    }

    let p = path_in(dir);
    std::fs::write(&p, body)?;
    restrict(&p);
    Ok(())
}

pub fn clear(dir: &Path) {
    let _ = std::fs::remove_file(path_in(dir));
}

/// Esconde o arquivo e o deixa legível só pelo dono.
fn restrict(p: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
    }

    #[cfg(windows)]
    {
        // std não expõe o atributo de oculto; usamos o comando do sistema
        let _ = std::process::Command::new("attrib")
            .arg("+h")
            .arg(p)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!("dejavu-test-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_reads_as_empty() {
        let t = TempDir::new("missing");
        let s = read(t.path()).unwrap();
        assert!(s.discord_webhook_url.is_none());
    }

    #[test]
    fn round_trips_webhook() {
        let t = TempDir::new("roundtrip");
        let url = "https://discord.com/api/webhooks/123/tok";
        write(
            t.path(),
            &Secrets {
                discord_webhook_url: Some(url.into()),
            },
        )
        .unwrap();
        assert_eq!(
            read(t.path()).unwrap().discord_webhook_url.as_deref(),
            Some(url)
        );
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let t = TempDir::new("comments");
        std::fs::write(
            path_in(t.path()),
            "# comentário\n\nDISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/1/t\n",
        )
        .unwrap();
        assert_eq!(
            read(t.path()).unwrap().discord_webhook_url.as_deref(),
            Some("https://discord.com/api/webhooks/1/t")
        );
    }

    #[test]
    fn clear_removes_the_file() {
        let t = TempDir::new("clear");
        write(
            t.path(),
            &Secrets {
                discord_webhook_url: Some("https://discord.com/api/webhooks/1/t".into()),
            },
        )
        .unwrap();
        assert!(path_in(t.path()).exists());
        clear(t.path());
        assert!(!path_in(t.path()).exists());
        assert!(read(t.path()).unwrap().discord_webhook_url.is_none());
    }

    #[test]
    fn file_is_written_outside_the_project_folder() {
        let t = TempDir::new("location");
        write(t.path(), &Secrets::default()).unwrap();
        let p = path_in(t.path());
        assert!(p.starts_with(t.path()));
        assert!(p.file_name().unwrap().to_string_lossy().starts_with('.'));
    }
}
