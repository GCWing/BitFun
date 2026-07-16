use bitfun_core::util::JS_THREADSAFE_FUNCTION;
use log::info;

pub async fn check_app_update_ohos() -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("check_app_update_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(String::new())).await;
    match res {
        Ok(res) => match res.await {
            Ok(result) => {
                info!("check_app_update_ohos successfully");
                Ok(result)
            }
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}
