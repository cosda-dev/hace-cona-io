use alloc::{collections::BTreeSet, string::String};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmoHandle {
    pub amo_id: String,
    pub loaded: bool,
}

#[derive(Debug, Default)]
pub struct AmoLoader {
    loaded: BTreeSet<String>,
}

impl AmoLoader {
    pub fn ensure_loaded(&mut self, amo_id: &str) -> AmoHandle {
        if !self.loaded.contains(amo_id) {
            self.load_from_disk(amo_id);
        }

        AmoHandle {
            amo_id: amo_id.to_string(),
            loaded: self.loaded.contains(amo_id),
        }
    }

    pub fn verify_hooks(&self, _amo_id: &str) -> Result<(), &'static str> {
        verify_fah()?;
        verify_zes()?;
        verify_version()?;
        Ok(())
    }

    fn load_from_disk(&mut self, amo_id: &str) {
        self.loaded.insert(amo_id.to_string());
    }
}

fn verify_fah() -> Result<(), &'static str> {
    Ok(())
}

fn verify_zes() -> Result<(), &'static str> {
    Ok(())
}

fn verify_version() -> Result<(), &'static str> {
    Ok(())
}
