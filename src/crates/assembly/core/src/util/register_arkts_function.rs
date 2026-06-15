use lazy_static::lazy_static;
use napi_derive_ohos::napi;
use napi_ohos::bindgen_prelude::Promise;
use napi_ohos::threadsafe_function::ThreadsafeFunction;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
lazy_static! {
    pub static ref JS_THREADSAFE_FUNCTION: RwLock<HashMap<String, Arc<ThreadsafeFunction<String, Promise<String>>>>> =
        Default::default();
}
#[napi]
pub fn register_arkts_function(
    function_name: String,
    callback: ThreadsafeFunction<String, Promise<String>>,
) {
    JS_THREADSAFE_FUNCTION
        .write()
        .insert(function_name, Arc::new(callback));
}

pub async fn open_dialog_file() -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("open_dialog_file").cloned()
    };

    let Some(function) = function else {
        return Err("open_dialog_file has not register".to_owned());
    };

    // 3. 调用 JS 函数
    // ThreadsafeFunction 本身是 Send 的，可以安全地在异地任务中使用
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(err) => match err.await {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_string()),
        },

        Err(err) => Err(err.to_string()),
    }
}
