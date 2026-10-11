use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::held::Hold;
use crate::config::store::{self, Failure, RUNNING};

const SCHEMA: u32 = 1;

#[derive(Deserialize)]
struct Header {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    written_by: String,
}

#[derive(Serialize, Deserialize)]
struct Stamped<T> {
    schema: u32,
    written_by: String,
    #[serde(flatten)]
    body: T,
}

pub(super) fn read<T: DeserializeOwned + Default>(path: &Path) -> Result<T, Failure> {
    text_at(path)?.map_or_else(|| Ok(T::default()), |text| body(path, &text))
}

pub(super) fn changed<T, R>(
    path: &Path,
    change: impl FnOnce(&mut T) -> Result<R, Failure>,
) -> Result<R, Failure>
where
    T: Serialize + DeserializeOwned + Default,
{
    let _hold = Hold::taken(path).map_err(|why| Failure::NotWritten {
        path: path.to_path_buf(),
        reason: why.to_string(),
    })?;
    let mut body: T = match text_at(path)? {
        Some(text) => {
            let header: Header = serde_json::from_str(&text).map_err(|why| unread(path, &why))?;
            if header.schema > SCHEMA
                || crate::version::Version::is_newer(&header.written_by, RUNNING)
            {
                return Err(Failure::TooNew {
                    path: path.to_path_buf(),
                    wrote: header.written_by,
                    running: RUNNING.to_owned(),
                });
            }
            self::body(path, &text)?
        }
        None => T::default(),
    };
    let before = serde_json::to_string(&body).map_err(|why| unwritten(path, &why))?;
    let answer = change(&mut body)?;
    let after = serde_json::to_string(&body).map_err(|why| unwritten(path, &why))?;
    if before == after {
        return Ok(answer);
    }
    let stamped = Stamped {
        schema: SCHEMA,
        written_by: RUNNING.to_owned(),
        body,
    };
    let written = serde_json::to_string_pretty(&stamped).map_err(|why| unwritten(path, &why))?;
    store::write(path, &written)?;
    Ok(answer)
}

fn body<T: DeserializeOwned>(path: &Path, text: &str) -> Result<T, Failure> {
    serde_json::from_str::<Stamped<T>>(text)
        .map(|stamped| stamped.body)
        .map_err(|why| unread(path, &why))
}

fn text_at(path: &Path) -> Result<Option<String>, Failure> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(Failure::Unreadable {
            path: path.to_path_buf(),
            reason: why.to_string(),
        }),
    }
}

fn unread(path: &Path, why: &serde_json::Error) -> Failure {
    Failure::Unreadable {
        path: path.to_path_buf(),
        reason: format!(
            "line {} column {} is not what this store holds",
            why.line(),
            why.column()
        ),
    }
}

fn unwritten(path: &Path, why: &serde_json::Error) -> Failure {
    Failure::NotWritten {
        path: path.to_path_buf(),
        reason: why.to_string(),
    }
}
