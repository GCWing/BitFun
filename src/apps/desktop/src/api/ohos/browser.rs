use bitfun_core::util::JS_THREADSAFE_FUNCTION;
use log::{info,error};
#[tauri::command]
pub async fn open_browser(url: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("open_browser").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(url)).await;
    match res {
        Ok(res) => match res.await{
            Ok(_) => {
                info!("open_browser successfully");
                Ok(())
            },
            Err(err) => {
                error!("open_browser failed: {}", err);
                Err(err.to_string())
            }
        },
        Err(err) => Err(err.to_string()),
    }
}