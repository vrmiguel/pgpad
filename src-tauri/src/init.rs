use tauri::WebviewWindowBuilder;

#[cfg(target_os = "macos")]
use tauri::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID},
    AppHandle, Emitter,
};

fn init_script() -> String {
    format!(
        r#"
        console.log(`I am in the init script, window.location.origin: ${{window.location.origin}}`);
        if (window.location.origin === '{}') {{
            window.__PGPAD_INTERNAL__ = {{ platform: "{}" }};
            console.log("window.__PGPAD_INTERNAL__: ", window.__PGPAD_INTERNAL__);
            (function() {{
              function ensureBanner() {{
                var id = 'pgpad-conn-banner';
                var el = document.getElementById(id);
                if (!el) {{
                  el = document.createElement('div');
                  el.id = id;
                  el.style.position = 'fixed';
                  el.style.top = '0';
                  el.style.left = '50%';
                  el.style.transform = 'translateX(-50%)';
                  el.style.zIndex = '9999';
                  el.style.padding = '8px 12px';
                  el.style.borderRadius = '6px';
                  el.style.fontFamily = 'system-ui, sans-serif';
                  el.style.fontSize = '12px';
                  el.style.color = '#fff';
                  el.style.background = '#444';
                  el.style.boxShadow = '0 2px 8px rgba(0,0,0,0.15)';
                  el.style.display = 'none';
                  document.body.appendChild(el);
                }}
                return el;
              }}
              function showBanner(text, color) {{
                var el = ensureBanner();
                el.textContent = text;
                el.style.background = color || '#444';
                el.style.display = 'block';
                clearTimeout(window.__PGPAD_BANNER_HIDE__);
                window.__PGPAD_BANNER_HIDE__ = setTimeout(function() {{ el.style.display = 'none'; }}, 3000);
              }}
              function listen(name, handler) {{
                if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {{
                  window.__TAURI__.event.listen(name, handler);
                }}
              }}
              document.addEventListener('DOMContentLoaded', function() {{ ensureBanner(); }});
              window.pgpadCancelAndReconnect = function(connectionId) {{
                try {{
                  if (window.__TAURI__ && window.__TAURI__.invoke) {{
                    window.__TAURI__.invoke('cancel_and_reconnect_mssql', {{ connection_id: connectionId }});
                  }}
                }} catch (e) {{ console.error('cancel_and_reconnect_mssql failed', e); }}
              }};
              // Optional inline prompt hook for quick testing
              var btnId = 'pgpad-conn-banner-btn';
              function ensureButton() {{
                var banner = document.getElementById('pgpad-conn-banner');
                if (!banner) return;
                if (document.getElementById(btnId)) return;
                var btn = document.createElement('button');
                btn.id = btnId;
                btn.textContent = 'Cancel+Reconnect';
                btn.style.marginLeft = '8px';
                btn.style.padding = '4px 8px';
                btn.style.border = 'none';
                btn.style.borderRadius = '4px';
                btn.style.cursor = 'pointer';
                btn.style.background = '#222';
                btn.style.color = '#fff';
                btn.addEventListener('click', function() {{
                  var id = prompt('Enter connection UUID to cancel & reconnect');
                  if (id) window.pgpadCancelAndReconnect(id);
                }});
                banner.appendChild(btn);
              }}
              document.addEventListener('DOMContentLoaded', function() {{ ensureBanner(); ensureButton(); }});
              (function(){{
                var id = 'pgpad-schema-panel';
                if (!document.getElementById(id)){{
                  var el = document.createElement('div');
                  el.id = id;
                  el.style.position = 'fixed';
                  el.style.bottom = '12px';
                  el.style.right = '12px';
                  el.style.zIndex = '9999';
                  el.style.padding = '10px';
                  el.style.borderRadius = '8px';
                  el.style.fontFamily = 'system-ui, sans-serif';
                  el.style.fontSize = '12px';
                  el.style.color = '#fff';
                  el.style.background = 'rgba(0,0,0,0.6)';
                  el.style.boxShadow = '0 2px 12px rgba(0,0,0,0.25)';
                  el.innerHTML = '<div style="display:flex;gap:6px;align-items:center;margin-bottom:6px">'+
                                 '<input id="pgpad-conn-id" placeholder="connection UUID" style="padding:4px;border-radius:4px;border:1px solid #333;background:#222;color:#fff" />'+
                                 '<input id="pgpad-page" type="number" value="0" min="0" style="width:56px;padding:4px;border-radius:4px;border:1px solid #333;background:#222;color:#fff" />'+
                                 '<input id="pgpad-size" type="number" value="20" min="1" style="width:56px;padding:4px;border-radius:4px;border:1px solid #333;background:#222;color:#fff" />'+
                                 '</div>'+
                                 '<div style="display:flex;flex-wrap:wrap;gap:6px;margin-bottom:6px">'+
                                  '<button data-ep="get_mssql_indexes" class="pgpad-ep">Indexes</button>'+
                                  '<button data-ep="get_mssql_constraints" class="pgpad-ep">Constraints</button>'+
                                  '<button data-ep="get_mssql_triggers" class="pgpad-ep">Triggers</button>'+
                                  '<button data-ep="get_mssql_routines" class="pgpad-ep">Routines</button>'+
                                  '<button data-ep="get_mssql_views" class="pgpad-ep">Views</button>'+
                                  '<button data-ep="get_mssql_index_columns" class="pgpad-ep">Index Columns</button>'+
                                  '<button data-ep="get_mssql_trigger_events" class="pgpad-ep">Trigger Events</button>'+
                                  '<button data-ep="get_mssql_routine_parameters" class="pgpad-ep">Routine Params</button>'+
                                  '<button data-ep="get_mssql_foreign_keys" class="pgpad-ep">Foreign Keys</button>'+
                                  '<button data-ep="get_mssql_view_definitions" class="pgpad-ep">View Defs</button>'+
                                 '</div>'+
                                  '<div style="display:flex;gap:6px;align-items:center;margin-bottom:6px">'+
                                   '<input id="pgpad-retry" type="number" value="3" min="0" style="width:72px;padding:4px;border-radius:4px;border:1px solid #333;background:#222;color:#fff" />'+
                                   '<input id="pgpad-backoff" type="number" value="1000" min="100" style="width:92px;padding:4px;border-radius:4px;border:1px solid #333;background:#222;color:#fff" />'+
                                   '<button id="pgpad-save-reconnect" class="pgpad-ep">Save Reconnect</button>'+
                                   '<label style="display:flex;align-items:center;gap:6px;color:#fff"><input id="pgpad-variant-basetype" type="checkbox" /> Variant base type</label>'+
                                   '<button id="pgpad-save-variant" class="pgpad-ep">Save Variant Pref</button>'+
                                  '</div>'+
                                  '<pre id="pgpad-ep-output" style="max-height:220px;overflow:auto;background:#111;padding:8px;border-radius:6px"></pre>';
                  document.body.appendChild(el);
                  var styleButtons = el.querySelectorAll('.pgpad-ep');
                  styleButtons.forEach(function(b){{ b.style.padding='4px 8px'; b.style.border='none'; b.style.borderRadius='4px'; b.style.background='#444'; b.style.color='#fff'; }});
                  el.addEventListener('click', async function(ev){{
                    var t = ev.target;
                    if (t.classList && t.classList.contains('pgpad-ep')){{
                      if (t.id === 'pgpad-save-reconnect'){{
                        var id = document.getElementById('pgpad-conn-id').value || null;
                        var retries = parseInt(document.getElementById('pgpad-retry').value||'0',10);
                        var backoff = parseInt(document.getElementById('pgpad-backoff').value||'1000',10);
                        var payload = id ? {{ connection_id:id, max_retries:retries, backoff_ms:backoff }} : {{ max_retries:retries, backoff_ms:backoff }};
                        window.__TAURI__.invoke('set_reconnect_settings', payload)
                          .then(function(){{ showBanner('Reconnect settings saved', '#2e7d32'); }})
                          .catch(function(e){{ showBanner('Save failed: '+String(e), '#c62828'); }});
                        return;
                      }}
                      if (t.id === 'pgpad-save-variant'){{
                        var idv = document.getElementById('pgpad-conn-id').value || null;
                        if (!idv) {{ showBanner('Provide connection UUID', '#cc8800'); return; }}
                        var enable = !!document.getElementById('pgpad-variant-basetype').checked;
                        window.__TAURI__.invoke('set_variant_settings', {{ connection_id:idv, enrich_base_type: enable }})
                          .then(function(){{ showBanner('Variant preference saved', '#2e7d32'); }})
                          .catch(function(e){{ showBanner('Save failed: '+String(e), '#c62828'); }});
                        return;
                      }}
                      var id = document.getElementById('pgpad-conn-id').value;
                      if (id) {{
                        try {{
                          var vset = await window.__TAURI__.invoke('get_variant_settings', {{ connection_id:id }});
                          if (vset) {{
                            try {{ var parsed = JSON.parse(vset); if (parsed && typeof parsed.enrich_base_type === 'boolean') document.getElementById('pgpad-variant-basetype').checked = parsed.enrich_base_type; }} catch(e){{}}
                          }}
                        }} catch(e){{}}
                      }}
                      var page = parseInt(document.getElementById('pgpad-page').value||'0',10);
                      var size = parseInt(document.getElementById('pgpad-size').value||'20',10);
                      if (!id || !window.__TAURI__ || !window.__TAURI__.invoke) return;
                      window.__TAURI__.invoke(t.dataset.ep, {{ connection_id:id, page:page, page_size:size }})
                        .then(async function(data){{
                          try {{
                            var variantToggle = document.getElementById('pgpad-variant-basetype');
                            var obj = JSON.parse(data);
                            if (variantToggle && variantToggle.checked) {{
                              obj = await window.pgpadEnrichVariantBaseTypes(id, obj);
                            }}
                            document.getElementById('pgpad-ep-output').textContent = JSON.stringify(obj,null,2);
                          }} catch(e) {{ document.getElementById('pgpad-ep-output').textContent = String(e); }}
                        }})
                        .catch(function(e){{ document.getElementById('pgpad-ep-output').textContent = String(e); }});
                    }}
                  }});
                  // Global helper to enrich variant base types in any result structure
                  window.pgpadEnrichVariantBaseTypes = async function(connectionId, data){{
                    async function enrichValue(v){{
                      if (v && typeof v === 'object'){{
                        if (Array.isArray(v)){{
                          for (var i=0;i<v.length;i++) v[i] = await enrichValue(v[i]);
                          return v;
                        }} else {{
                          if (v.type === 'sql_variant' && (v.base_type===null || v.base_type===undefined) && typeof v.value === 'string'){{
                            try {{
                              var bt = await window.__TAURI__.invoke('get_mssql_variant_base_type', {{ connection_id: connectionId, value: v.value }});
                              v.base_type = bt || null;
                            }} catch(e) {{ /* ignore */ }}
                          }}
                          // recurse into object fields
                          for (var k in v) if (Object.prototype.hasOwnProperty.call(v,k)) v[k] = await enrichValue(v[k]);
                          return v;
                        }}
                      }}
                      return v;
                    }}
                    return enrichValue(data);
                  }};
                }}
              }})();
              listen('mssql-cancel-start', function(ev) {{ showBanner('Cancelling MSSQL…', '#cc8800'); }});
              listen('mssql-cancelled', function(ev) {{ showBanner('MSSQL cancelled', '#888'); }});
              listen('mssql-reconnecting', function(ev) {{ showBanner('Reconnecting MSSQL…', '#0077cc'); }});
              listen('mssql-reconnected', function(ev) {{ showBanner('MSSQL reconnected', '#2e7d32'); setDisabled(false); }});
              function setDisabled(disabled){{
                var id = 'pgpad-disable-overlay';
                var el = document.getElementById(id);
                if (disabled){{
                  if (!el){{ el = document.createElement('div'); el.id = id; el.style.position='fixed'; el.style.inset='0'; el.style.background='rgba(0,0,0,0.08)'; el.style.backdropFilter='blur(2px)'; el.style.zIndex='9998'; document.body.appendChild(el); }}
                }} else {{ if (el) el.remove(); }}
              }}
              listen('mssql-reconnecting', function(ev){{ setDisabled(true); }});
              listen('mssql-reconnect-failed', function(ev) {{
                var payload = ev && ev.payload ? ev.payload : null;
                var msg = 'MSSQL reconnect failed';
                if (payload && payload.error) msg += ': ' + payload.error;
                showBanner(msg, '#c62828');
                setDisabled(false);
                // Attach quick retry
                var id = (payload && payload.connection_id) ? payload.connection_id : null;
                if (id) {{ window.pgpadCancelAndReconnect(id); }}
              }});
              listen('end-of-connection', function(ev) {{ showBanner('Connection ended', '#c62828'); }});
              listen('connection-reconnected', function(ev) {{ showBanner('Connection reconnected', '#2e7d32'); }});
            }})();
        }}
    "#,
        if cfg!(debug_assertions) {
            "http://localhost:1420"
        } else {
            "tauri://localhost"
        },
        std::env::consts::OS
    )
}

pub fn build_window(app: &tauri::App) -> tauri::Result<()> {
    let cfg = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .expect("main window config missing");

    let window_builder = WebviewWindowBuilder::from_config(app.handle(), cfg)?
        .initialization_script(init_script())
        .prevent_overflow();

    #[cfg(target_os = "macos")]
    let window_builder = {
        use tauri::{utils::config::WindowEffectsConfig, window::Effect, LogicalPosition};

        window_builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .decorations(true)
            .effects(WindowEffectsConfig {
                effects: vec![Effect::WindowBackground],
                state: None,
                radius: Some(12.0),
                color: None,
            })
            .traffic_light_position(tauri::Position::Logical(LogicalPosition::new(16.0, 18.5)))
            .hidden_title(true)
    };

    window_builder.build()?;

    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn build_menu(_app: &tauri::App) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn build_menu(app: &tauri::App) -> anyhow::Result<()> {
    let app_handle = app.handle();
    let pkg_info = app_handle.package_info();

    let about_metadata = {
        let config = app_handle.config();
        AboutMetadata {
            name: Some(pkg_info.name.clone()),
            version: Some(pkg_info.version.to_string()),
            copyright: config.bundle.copyright.clone(),
            authors: config.bundle.publisher.clone().map(|p| vec![p]),
            ..Default::default()
        }
    };

    let window_menu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;

    let menu = Menu::with_items(
        app_handle,
        &[
            &Submenu::with_items(
                app_handle,
                pkg_info.name.clone(),
                true,
                &[
                    &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::services(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::hide(app_handle, None)?,
                    &PredefinedMenuItem::hide_others(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::quit(app_handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "File",
                true,
                &[
                    &MenuItem::with_id(
                        app_handle,
                        "new_tab",
                        "New Tab",
                        true,
                        Some("CmdOrControl+N"),
                    )?,
                    &MenuItem::with_id(
                        app_handle,
                        "close_tab",
                        "Close Tab",
                        true,
                        Some("CmdOrControl+W"),
                    )?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(app_handle, None)?,
                    &PredefinedMenuItem::redo(app_handle, None)?,
                    &PredefinedMenuItem::separator(app_handle)?,
                    &PredefinedMenuItem::cut(app_handle, None)?,
                    &PredefinedMenuItem::copy(app_handle, None)?,
                    &PredefinedMenuItem::paste(app_handle, None)?,
                    &PredefinedMenuItem::select_all(app_handle, None)?,
                ],
            )?,
            &Submenu::with_items(
                app_handle,
                "View",
                true,
                &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
            )?,
            &window_menu,
        ],
    )?;

    app.set_menu(menu)?;
    app.on_menu_event(move |handle: &tauri::AppHandle, event| {
        let event = event.id().0.as_str();

        log::debug!("[on_menu_event][{event}] Event triggered");

        if let Err(err) = menu_event_handler(event, handle) {
            log::error!("[on_menu_event] [{event}] {:?}", err);
        }
    });

    Ok(())
}

#[cfg(target_os = "macos")]
fn menu_event_handler(event: &str, handle: &AppHandle) -> anyhow::Result<()> {
    match event {
        "new_tab" => {
            handle.emit("new_tab", ())?;
        }
        "close_tab" => {
            handle.emit("close_tab", ())?;
        }
        _ => {
            log::info!("Unexpected menu event: {}", event);
        }
    }
    Ok(())
}
