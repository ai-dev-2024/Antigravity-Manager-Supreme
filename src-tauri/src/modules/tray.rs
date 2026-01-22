use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager, Runtime, Emitter, Listener,
};
use crate::modules;

pub fn create_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    // 1. LoadConfigGetlanguageSet
    let config = modules::load_app_config().unwrap_or_default();
    let texts = modules::i18n::get_tray_texts(&config.language);
    
    // 2. LoadIcon（macOS Using Template Image）
    let icon_bytes = include_bytes!("../../icons/tray-icon.png");
    let img = image::load_from_memory(icon_bytes)
        .map_err(|e| tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        .to_rgba8();
    let (width, height) = img.dimensions();
    let icon = Image::new_owned(img.into_raw(), width, height);

    // 3. definitionMenu项（UsingTranslate text）
    // Status区
    let loading_text = format!("{}: ...", texts.current);
    let quota_text = format!("{}: --", texts.quota);
    let info_user = MenuItem::with_id(app, "info_user", &loading_text, false, None::<&str>)?;
    let info_quota = MenuItem::with_id(app, "info_quota", &quota_text, false, None::<&str>)?;

    // Quick operation area
    let switch_next = MenuItem::with_id(app, "switch_next", &texts.switch_next, true, None::<&str>)?;
    let refresh_curr = MenuItem::with_id(app, "refresh_curr", &texts.refresh_current, true, None::<&str>)?;
    
    // SystemFunction
    let show_i = MenuItem::with_id(app, "show", &texts.show_window, true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", &texts.quit, true, None::<&str>)?;
    
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;

    // 4. buildMenu
    let menu = Menu::with_items(app, &[
        &info_user,
        &info_quota,
        &sep1,
        &switch_next,
        &refresh_curr,
        &sep2,
        &show_i,
        &sep3,
        &quit_i,
    ])?;

    // 4. build pallet
    let _ = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(icon)
        .on_menu_event(move |app, event| {
            let app_handle = app.clone();
            match event.id().as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        #[cfg(target_os = "macos")]
                        app.set_activation_policy(tauri::ActivationPolicy::Regular).unwrap_or(());
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                "refresh_curr" => {
                    // AsyncExecuteRefresh
                    tauri::async_runtime::spawn(async move {
                        if let Ok(Some(account_id)) = modules::get_current_account_id() {
                             // Notificationfront endBegin
                             let _ = app_handle.emit("tray://refresh-current", ());
                             
                             // ExecuteRefreshlogic
                             if let Ok(mut account) = modules::load_account(&account_id) {
                                 // Using modules::account shared logic in
                                 match modules::account::fetch_quota_with_retry(&mut account).await {
                                     Ok(quota) => {
                                         // Save
                                         let _ = modules::update_account_quota(&account.id, quota);
                                         // UpdatePallet display
                                         update_tray_menus(&app_handle);
                                     },
                                     Err(e) => {
                                         // Error handling，May只RecordLog
                                          modules::logger::log_error(&format!("trayRefreshFailed: {}", e));
                                     }
                                 }
                             }
                        }
                    });
                }
                "switch_next" => {
                    tauri::async_runtime::spawn(async move {
                         // 1. GetAllAccount
                         if let Ok(accounts) = modules::list_accounts() {
                             if accounts.is_empty() { return; }
                             
                             let current_id = modules::get_current_account_id().unwrap_or(None);
                             let next_account = if let Some(curr) = current_id {
                                 let idx = accounts.iter().position(|a| a.id == curr).unwrap_or(0);
                                 let next_idx = (idx + 1) % accounts.len();
                                 &accounts[next_idx]
                             } else {
                                 &accounts[0]
                             };
                             
                             // 2. switch
                             if let Ok(_) = modules::switch_account(&next_account.id).await {
                                 // 3. Notificationfront end
                                 let _ = app_handle.emit("tray://account-switched", next_account.id.clone());
                                 // 4. Updatetray
                                 update_tray_menus(&app_handle);
                             }
                         }
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
               let app = tray.app_handle();
               if let Some(window) = app.get_webview_window("main") {
                   let _ = window.show();
                   let _ = window.set_focus();
                   #[cfg(target_os = "macos")]
                   app.set_activation_policy(tauri::ActivationPolicy::Regular).unwrap_or(());
               }
            }
        })
        .build(app)?;

    // Initialize时UpdateonceStatus
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        update_tray_menus(&handle);
    });

    // ListenConfigchangeEvent
    let handle = app.clone();
    app.listen("config://updated", move |_event| {
        modules::logger::log_info("Config已Update，RefreshtrayMenu");
        update_tray_menus(&handle);
    });

    Ok(())
}

/// UpdatetrayMenu的AuxiliaryFunction
pub fn update_tray_menus<R: Runtime>(app: &tauri::AppHandle<R>) {
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
         // ReadConfigGetlanguage
         let config = modules::load_app_config().unwrap_or_default();
         let texts = modules::i18n::get_tray_texts(&config.language);
         
         // GetCurrentAccountInfo
         let current = modules::get_current_account_id().unwrap_or(None);
         
         let mut menu_lines = Vec::new();
         let mut user_text = format!("{}: {}", texts.current, texts.no_account);

         if let Some(id) = current {
             if let Ok(account) = modules::load_account(&id) {
                 user_text = format!("{}: {}", texts.current, account.email);
                 
                 if let Some(q) = account.quota {
                     if q.is_forbidden {
                         menu_lines.push(format!("🚫 {}", texts.forbidden));
                     } else {
                         // extract 3 specifiedModel
                         let mut gemini_high = 0;
                         let mut gemini_image = 0;
                         let mut claude = 0;
                         
                         // Usingstrict match，Consistent with front-end
                         for m in q.models {
                             let name = m.name.to_lowercase();
                             if name == "gemini-3-pro-high" { gemini_high = m.percentage; }
                             if name == "gemini-3-pro-image" { gemini_image = m.percentage; }
                             if name == "claude-sonnet-4-5" { claude = m.percentage; }
                         }
                         
                         menu_lines.push(format!("Gemini High: {}%", gemini_high));
                         menu_lines.push(format!("Gemini Image: {}%", gemini_image));
                         menu_lines.push(format!("Claude 4.5: {}%", claude));
                     }
                 } else {
                     menu_lines.push(texts.unknown_quota.clone());
                 }
             } else {
                 user_text = format!("{}: Error", texts.current);
                 menu_lines.push(format!("{}: --", texts.quota));
             }
         } else {
             menu_lines.push(texts.unknown_quota.clone());
         };

         // RebuildMenu项
         let info_user = MenuItem::with_id(&app_clone, "info_user", &user_text, false, None::<&str>);
         
         // DynamicCreateQuota item
         let mut quota_items = Vec::new();
         for (i, line) in menu_lines.iter().enumerate() {
             let item = MenuItem::with_id(&app_clone, format!("info_quota_{}", i), line, false, None::<&str>);
             if let Ok(item) = item {
                 quota_items.push(item);
             }
         }
         
         let switch_next = MenuItem::with_id(&app_clone, "switch_next", &texts.switch_next, true, None::<&str>);
         let refresh_curr = MenuItem::with_id(&app_clone, "refresh_curr", &texts.refresh_current, true, None::<&str>);
         
         let show_i = MenuItem::with_id(&app_clone, "show", &texts.show_window, true, None::<&str>);
         let quit_i = MenuItem::with_id(&app_clone, "quit", &texts.quit, true, None::<&str>);
         
         if let (Ok(i_u), Ok(s_n), Ok(r_c), Ok(s), Ok(q)) = (info_user, switch_next, refresh_curr, show_i, quit_i) {
             let sep1 = PredefinedMenuItem::separator(&app_clone).ok();
             let sep2 = PredefinedMenuItem::separator(&app_clone).ok();
             let sep3 = PredefinedMenuItem::separator(&app_clone).ok();
             
             let mut items: Vec<&dyn tauri::menu::IsMenuItem<R>> = vec![&i_u];
             // AddDynamicamount item
             for item in &quota_items {
                 items.push(item);
             }
             
             if let Some(ref s) = sep1 { items.push(s); }
             items.push(&s_n);
             items.push(&r_c);
             if let Some(ref s) = sep2 { items.push(s); }
             items.push(&s);
             if let Some(ref s) = sep3 { items.push(s); }
             items.push(&q);
             
             if let Ok(menu) = Menu::with_items(&app_clone, &items) {
                 if let Some(tray) = app_clone.tray_by_id("main") {
                     let _ = tray.set_menu(Some(menu));
                 }
             }
         }
    });
}
