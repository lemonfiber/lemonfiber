use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::undoing::Undone;
use crate::config::store::Failure;

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Settings {
    #[serde(default)]
    values: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    filed: bool,
}

impl Settings {
    pub(crate) fn at(path: &Path) -> Result<Self, Failure> {
        super::kept::read(path)
    }

    pub(crate) fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub(crate) const fn is_filed(&self) -> bool {
        self.filed
    }

    fn resettled(&mut self, name: &str, value: Option<&str>, wrote: Option<&str>) -> Undone {
        let holds = self.value(name);
        if holds == value {
            return Undone::Done;
        }
        if holds != wrote {
            return Undone::TheirsNow;
        }
        match value {
            Some(value) => self.values.insert(name.to_owned(), value.to_owned()),
            None => self.values.remove(name),
        };
        Undone::Done
    }
}

pub(crate) fn marked_filed(path: &Path) -> Result<(), Failure> {
    super::kept::changed(path, &mut |settings: &mut Settings| {
        settings.filed = true;
        Ok(())
    })
}

pub(crate) fn forgotten(path: &Path, namespace: &str) -> Result<(), Failure> {
    super::kept::changed(path, &mut |settings: &mut Settings| {
        settings
            .values
            .retain(|name, _| !super::in_namespace(name, namespace));
        Ok(())
    })
}

pub(crate) fn resettled(
    path: &Path,
    name: &str,
    value: Option<&str>,
    wrote: Option<&str>,
) -> Result<Undone, Failure> {
    let mut undone = Undone::Done;
    super::kept::changed(path, &mut |settings: &mut Settings| {
        undone = settings.resettled(name, value, wrote);
        Ok(())
    })?;
    Ok(undone)
}
