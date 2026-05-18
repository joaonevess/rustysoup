use crate::dom::Document;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::sync::Arc;

pub type SharedDocument = Arc<RwLock<Document>>;

pub fn shared_document(document: Document) -> SharedDocument {
    Arc::new(RwLock::new(document))
}

pub fn read_document(document: &SharedDocument) -> RwLockReadGuard<'_, Document> {
    document.read()
}

pub fn write_document(document: &SharedDocument) -> RwLockWriteGuard<'_, Document> {
    document.write()
}
