// Autostart Order
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub async fn toggle_auto_launch(
    app: tauri::AppHandle,
    enable: bool,
) -> Result<(), String> {
    let manager = app.autolaunch();
    
    if enable {
        manager.enable().map_err(|e| format!("EnableautomaticStartFailed: {}", e))?;
        crate::modules::logger::log_info("已EnableAutomatically start upStart");
    } else {
        match manager.disable() {
            Ok(_) => {
                crate::modules::logger::log_info("已DisableAutomatically start upStart");
            },
            Err(e) => {
                let err_msg = e.to_string();
                // 在 Windows 上，IfRegistertable entryDoes not exist，disable() 会Return "SystemNot founddesignatedFile" (os error 2)
                // this situationShouldregarded asSuccess，BecauseTarget（Disable）Alreadyachieve
                if err_msg.contains("os error 2") || err_msg.contains("Not founddesignatedFile") {
                    crate::modules::logger::log_info("The startup item has beenDoes not exist，regarded asDisableSuccess");
                } else {
                    return Err(format!("DisableautomaticStartFailed: {}", e));
                }
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
pub async fn is_auto_launch_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    let manager = app.autolaunch();
    manager.is_enabled().map_err(|e| e.to_string())
}
