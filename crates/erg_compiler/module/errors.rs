use std::path::Path;

use erg_common::pathutil::NormalizedPathBuf;
use erg_common::shared::Shared;
use erg_common::traits::Stream;

use crate::error::{CompileError, CompileErrors};

#[derive(Debug, Clone, Default)]
pub struct SharedCompileErrors(Shared<CompileErrors>);

impl SharedCompileErrors {
    pub fn new() -> Self {
        Self(Shared::new(CompileErrors::empty()))
    }

    pub fn push(&self, error: CompileError) {
        self.0.borrow_mut().push(error);
    }

    pub fn extend(&self, errors: CompileErrors) {
        self.0.borrow_mut().extend(errors);
    }

    pub fn take(&self) -> CompileErrors {
        self.0.borrow_mut().take_all().into()
    }

    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub fn remove(&self, path: &Path) {
        let path = NormalizedPathBuf::from(path);
        self.0
            .borrow_mut()
            .retain(|e| NormalizedPathBuf::from(e.input.path()) != path);
    }

    pub fn raw_iter(&self) -> impl Iterator<Item = &CompileError> {
        let _ref = self.0.borrow();
        let ref_ = unsafe { self.0.as_ptr().as_ref().unwrap() };
        ref_.iter()
    }
}

pub type SharedCompileWarnings = SharedCompileErrors;
