use crate::AppState;
use bitfun_core::util::JS_THREADSAFE_FUNCTIONS;
use log::error;
use napi_ohos::threadsafe_function::ThreadsafeFunctionCallMode;
use std::sync::mpsc::channel;
use tauri::State;

#[tauri::command]
pub fn handle_min_window() -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTIONS.read();
        lock.get("handle_min_window").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    function.call(Ok("".to_string()),ThreadsafeFunctionCallMode::NonBlocking);
    Ok(())
}
#[tauri::command]
pub fn handle_max_window() -> Result<(),String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTIONS.read();

        lock.get("handle_max_window").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    function.call(Ok("".to_string()), ThreadsafeFunctionCallMode::NonBlocking);
    Ok(())
}
#[tauri::command]
pub fn handle_restore_window() -> Result<(),String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTIONS.read();
        lock.get("handle_restore_window").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    function.call(Ok("".to_string()), ThreadsafeFunctionCallMode::NonBlocking);
    Ok(())
}
#[tauri::command]
pub async fn window_is_minimized() -> Result<bool, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTIONS.read();
        lock.get("window_is_minimized").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("str".to_string())).await;
    match res {
        Ok(err) => match err.await{
            Ok(result) => {
                if result.eq("true") {
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
            Err(err) => Err(err.to_string()),
        }
        Err(err) => Err(err.to_string()),
    }
}
#[tauri::command]
pub fn window_start_dragging() -> Result<(),String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTIONS.read();
        lock.get("window_start_dragging").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    function.call(Ok("".to_string()), ThreadsafeFunctionCallMode::NonBlocking);
    Ok(())
}
#[tauri::command]
pub fn close_window() -> Result<(),String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTIONS.read();
        lock.get("close_window").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    function.call(Ok("".to_string()), ThreadsafeFunctionCallMode::NonBlocking);
    Ok(())
}
#[tauri::command]
pub async fn window_is_maximized() -> Result<bool, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTIONS.read();
        lock.get("window_is_maximized").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("str".to_string())).await;
    match res {
        Ok(err) => match err.await {
            Ok(result) => {
                if result.eq("true") {
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
            Err(err) => Err(err.to_string()),
        }
        Err(err) => Err(err.to_string()),
    }
}