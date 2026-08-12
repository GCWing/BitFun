use bitfun_core::util::JS_THREADSAFE_FUNCTION;
use napi_ohos::threadsafe_function::ThreadsafeFunctionCallMode;

#[tauri::command]
pub fn handle_min_window() -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
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
        let lock = JS_THREADSAFE_FUNCTION.read();

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
        let lock = JS_THREADSAFE_FUNCTION.read();
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
        let lock = JS_THREADSAFE_FUNCTION.read();
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
        let lock = JS_THREADSAFE_FUNCTION.read();
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
        let lock = JS_THREADSAFE_FUNCTION.read();
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
        let lock = JS_THREADSAFE_FUNCTION.read();
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

#[tauri::command]
pub async fn set_always_on_top_ohos(arg: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_always_on_top_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(arg)).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn set_decorations_ohos(arg: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_decorations_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(arg)).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn set_skip_taskbar_ohos(arg: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_skip_taskbar_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(arg)).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn set_window_size_ohos(arg: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_window_size_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(arg)).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn set_window_position_ohos(arg: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_window_position_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(arg)).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn outer_position_ohos() -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("outer_position_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn outer_size_ohos() -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("outer_size_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn inner_size_ohos() -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("inner_size_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn current_monitor_ohos() -> Result<String, String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("current_monitor_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(result) => Ok(result),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn unmaximize_ohos() -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("unmaximize_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn set_min_size_ohos(arg: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_min_size_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(arg)).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn set_focus_ohos() -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_focus_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn set_resizable_ohos(arg: String) -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("set_resizable_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok(arg)).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn maximize_ohos() -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("maximize_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn center_ohos() -> Result<(), String> {
    let function = {
        let lock = JS_THREADSAFE_FUNCTION.read();
        lock.get("center_ohos").cloned()
    };
    let Some(function) = function else {
        return Err("The Arkts has not register the function".to_owned());
    };
    let res = function.call_async(Ok("".to_string())).await;
    match res {
        Ok(fut) => match fut.await {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string()),
        },
        Err(err) => Err(err.to_string()),
    }
}