//! Browser draft persistence: keeps the active document in IndexedDB (and a fast synchronous
//! localStorage mirror) so refreshing, navigating, or closing the tab never loses the user's work.

use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{IdbDatabase, IdbRequest, IdbTransactionMode};

const DB_NAME: &str = "wordcraft_drafts";
const STORE: &str = "drafts";
const KEY_DOC: &str = "active_draft";
const KEY_TITLE: &str = "active_title";

const LS_DOC: &str = "wordcraft_active_draft_docx";
const LS_TITLE: &str = "wordcraft_active_draft_title";
const LS_TIME: &str = "wordcraft_active_draft_time";

/// Limit localStorage mirror to ~3.5 MB to avoid quota exceeded errors in browsers.
const MAX_LOCAL_STORAGE_BYTES: usize = 3_500_000;

fn js(e: JsValue) -> String {
    format!("{e:?}")
}

/// Wait for an IndexedDB request; its result.
async fn done(req: &IdbRequest) -> Result<JsValue, String> {
    let promise = js_sys::Promise::new(&mut |resolve: js_sys::Function, reject: js_sys::Function| {
        let ok_req = req.clone();
        let ok = Closure::once_into_js(move || {
            let res = match ok_req.result() {
                Ok(v) => v,
                Err(_) => JsValue::UNDEFINED,
            };
            let _ = resolve.call1(&JsValue::NULL, &res);
        });
        let err = Closure::once_into_js(move || {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("IndexedDB request failed"));
        });
        req.set_onsuccess(Some(ok.unchecked_ref()));
        req.set_onerror(Some(err.unchecked_ref()));
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.map_err(js)
}

/// The database, with its drafts store created on first use.
async fn open() -> Result<IdbDatabase, String> {
    let factory = web_sys::window().ok_or_else(|| "no window".to_string())?.indexed_db().map_err(js)?.ok_or_else(|| "IndexedDB unavailable".to_string())?;
    let req = factory.open_with_u32(DB_NAME, 1).map_err(js)?;
    let upgrade_req = req.clone();
    let upgrade = Closure::<dyn FnMut()>::new(move || {
        if let Ok(db) = upgrade_req.result().and_then(|r| r.dyn_into::<IdbDatabase>())
            && !db.object_store_names().contains(STORE)
            && let Err(e) = db.create_object_store(STORE)
        {
            log::error!("couldn't make the draft store: {e:?}");
        }
    });
    req.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    let db = done(&req).await;
    req.set_onupgradeneeded(None);
    drop(upgrade);
    db?.dyn_into::<IdbDatabase>().map_err(js)
}

/// Synchronous save to localStorage: fast fallback for beforeunload.
pub fn sync_save_local(bytes: &[u8], title: Option<&str>) {
    if bytes.len() > MAX_LOCAL_STORAGE_BYTES {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    let Ok(Some(storage)) = window.local_storage() else { return };
    let b64 = wordcraft_engine::cmd::insert::base64_encode(bytes);
    let _ = storage.set_item(LS_DOC, &b64);
    if let Some(t) = title {
        let _ = storage.set_item(LS_TITLE, t);
    } else {
        let _ = storage.remove_item(LS_TITLE);
    }
    let _ = storage.set_item(LS_TIME, &js_sys::Date::now().to_string());
}

/// Synchronous load from localStorage fallback.
pub fn sync_load_local() -> Option<(Vec<u8>, Option<String>)> {
    let window = web_sys::window()?;
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return None,
    };
    let b64 = match storage.get_item(LS_DOC) {
        Ok(Some(s)) if !s.is_empty() => s,
        _ => return None,
    };
    let bytes = wordcraft_engine::cmd::insert::base64_decode(&b64)?;
    if bytes.is_empty() {
        return None;
    }
    let title = storage.get_item(LS_TITLE).ok().flatten();
    Some((bytes, title))
}

/// Synchronous clear of localStorage draft.
pub fn sync_clear_local() {
    let Some(window) = web_sys::window() else { return };
    let Ok(Some(storage)) = window.local_storage() else { return };
    let _ = storage.remove_item(LS_DOC);
    let _ = storage.remove_item(LS_TITLE);
    let _ = storage.remove_item(LS_TIME);
}

/// Save the draft to IndexedDB and mirror to localStorage.
pub async fn save_draft(bytes: Vec<u8>, title: Option<String>) -> Result<(), String> {
    sync_save_local(&bytes, title.as_deref());
    let db = open().await?;
    let tx = db.transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite).map_err(js)?;
    let store = tx.object_store(STORE).map_err(js)?;
    let req_bytes = store.put_with_key(&js_sys::Uint8Array::from(bytes.as_slice()), &JsValue::from_str(KEY_DOC)).map_err(js)?;
    done(&req_bytes).await?;
    if let Some(t) = &title {
        let req_title = store.put_with_key(&JsValue::from_str(t), &JsValue::from_str(KEY_TITLE)).map_err(js)?;
        done(&req_title).await?;
    } else {
        let req_del = store.delete(&JsValue::from_str(KEY_TITLE)).map_err(js)?;
        done(&req_del).await?;
    }
    Ok(())
}

/// Clear the draft from both IndexedDB and localStorage.
pub async fn clear_draft() -> Result<(), String> {
    sync_clear_local();
    let db = open().await?;
    let tx = db.transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite).map_err(js)?;
    let store = tx.object_store(STORE).map_err(js)?;
    let req1 = store.delete(&JsValue::from_str(KEY_DOC)).map_err(js)?;
    let _ = done(&req1).await;
    let req2 = store.delete(&JsValue::from_str(KEY_TITLE)).map_err(js)?;
    let _ = done(&req2).await;
    Ok(())
}

/// Load the active draft. Tries IndexedDB first; falls back to localStorage.
pub async fn load_active_draft() -> Option<(Vec<u8>, Option<String>)> {
    let from_idb: Result<Option<(Vec<u8>, Option<String>)>, String> = async {
        let db = open().await?;
        let tx = db.transaction_with_str(STORE).map_err(js)?;
        let store = tx.object_store(STORE).map_err(js)?;
        let req_bytes = store.get(&JsValue::from_str(KEY_DOC)).map_err(js)?;
        let val_bytes = done(&req_bytes).await?;
        if val_bytes.is_undefined() || val_bytes.is_null() {
            return Ok(None);
        }
        let arr = val_bytes.dyn_into::<js_sys::Uint8Array>().map_err(js)?;
        let bytes = arr.to_vec();
        if bytes.is_empty() {
            return Ok(None);
        }
        let req_title = store.get(&JsValue::from_str(KEY_TITLE)).map_err(js)?;
        let val_title = done(&req_title).await.ok();
        let title = val_title.and_then(|v| v.as_string());
        Ok(Some((bytes, title)))
    }
    .await;

    match from_idb {
        Ok(Some(d)) => Some(d),
        _ => sync_load_local(),
    }
}
